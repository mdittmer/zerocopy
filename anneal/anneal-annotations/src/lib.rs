// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Shared parsing and semantic data for Anneal v2 Rust attributes.
//!
//! The parser is split into three small stages:
//! - [`schema`] defines legal paths and default children.
//! - [`parse`] matches raw Rust attribute syntax against that schema.
//! - [`decode`] converts matched entries into Anneal data structures.
//!
mod decode;
mod error;
mod parse;
mod schema;

pub use error::{AnnotationError, Result};
pub use schema::AnnotationTarget;

/// Prefix used for invisible rustdoc comments which prove that rustc expanded
/// an Anneal attribute observed by the source scanner.
pub const EXPANSION_MARKER_PREFIX: &str = "<!--anneal-expanded-v1:";
const EXPANSION_MARKER_SUFFIX: &str = "-->";

/// Builds the expansion marker for a directly spelled Anneal attribute.
///
/// The proc macro attaches this as an HTML comment to the expanded item.
/// `cargo anneal` reconciles those comments against its source scan after
/// Charon runs, failing closed if an alias or another macro hid an annotation
/// from the unexpanded syntax tree.
pub fn expansion_marker_for_attribute(attr: &syn::Attribute) -> Result<Option<String>> {
    use quote::ToTokens as _;

    let path =
        attr.path().segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>();
    let is_macro = matches!(path.as_slice(), [anneal] if anneal == "anneal")
        || matches!(path.as_slice(), [crate_name, anneal]
            if crate_name == "anneal_macros" && anneal == "anneal");
    if !is_macro {
        return Ok(None);
    }
    let tokens = match &attr.meta {
        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::NameValue(value) => value.value.to_token_stream(),
    };
    // `proc_macro` and proc_macro2's string parser preserve different spacing
    // metadata specifically for the punctuation in Rust path separators.
    // Normalize only `:` (a separated `: :` is not valid in Anneal's parsed
    // expression/path positions) and retain jointness for every other
    // punctuation token so distinct Rust operators cannot collide.
    fn canonical_tokens(tokens: proc_macro2::TokenStream, out: &mut String) {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Group(group) => {
                    let delimiter = match group.delimiter() {
                        proc_macro2::Delimiter::Parenthesis => "()",
                        proc_macro2::Delimiter::Brace => "{}",
                        proc_macro2::Delimiter::Bracket => "[]",
                        proc_macro2::Delimiter::None => "--",
                    };
                    out.push('g');
                    out.push_str(delimiter);
                    out.push('[');
                    canonical_tokens(group.stream(), out);
                    out.push(']');
                }
                proc_macro2::TokenTree::Ident(identifier) => {
                    let identifier = identifier.to_string();
                    out.push('i');
                    out.push_str(&identifier.len().to_string());
                    out.push(':');
                    out.push_str(&identifier);
                    out.push(';');
                }
                proc_macro2::TokenTree::Punct(punctuation) => {
                    out.push('p');
                    out.push(punctuation.as_char());
                    if punctuation.as_char() != ':' {
                        out.push(match punctuation.spacing() {
                            proc_macro2::Spacing::Alone => 'a',
                            proc_macro2::Spacing::Joint => 'j',
                        });
                    }
                    out.push(';');
                }
                proc_macro2::TokenTree::Literal(literal) => {
                    let literal = literal.to_string();
                    out.push('l');
                    out.push_str(&literal.len().to_string());
                    out.push(':');
                    out.push_str(&literal);
                    out.push(';');
                }
            }
        }
    }
    let mut payload = String::new();
    canonical_tokens(tokens, &mut payload);
    let encoded = payload.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    Ok(Some(format!("{EXPANSION_MARKER_PREFIX}{encoded}{EXPANSION_MARKER_SUFFIX}")))
}

/// Returns whether a Charon-retained doc comment is an Anneal expansion marker.
pub fn is_expansion_marker(comment: &str) -> bool {
    comment
        .strip_prefix(EXPANSION_MARKER_PREFIX)
        .and_then(|payload| payload.strip_suffix(EXPANSION_MARKER_SUFFIX))
        .is_some_and(|payload| {
            payload.len() % 2 == 0 && payload.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

/// All Anneal attributes attached to one Rust item.
pub type Attributes = Vec<Attribute>;

/// One decoded `#[anneal(...)]` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    /// Semantic content of the attribute.
    pub data: AttributeData,
    /// Verification domains to which this attribute applies.
    pub domains: Vec<String>,
}

impl Default for Attribute {
    fn default() -> Self {
        Self { data: AttributeData::default(), domains: Self::default_domains() }
    }
}

impl Attribute {
    fn default_domains() -> Vec<String> {
        vec![String::from("soundness")]
    }
}

/// Decoded Anneal attribute content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttributeData {
    /// Preconditions, postconditions, and invariants.
    Spec(Spec),
    /// Proof fragment.
    Proof(Proof),
}

impl Default for AttributeData {
    fn default() -> Self {
        Self::Spec(Spec::default())
    }
}

/// Function specifications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spec {
    /// Preconditions.
    pub requires: Vec<Prop>,
    /// Postconditions.
    pub ensures: Vec<Prop>,
    /// Invariants.
    pub invariants: Vec<Prop>,
}

/// A proposition used by an Anneal specification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prop {
    /// A proposition expressed as a Rust boolean expression.
    Rust(RustProp),
    /// A proposition expressed as Lean source code.
    Lean(LeanProp),
}

impl From<String> for Prop {
    fn from(value: String) -> Self {
        Self::Lean(LeanProp::from(value))
    }
}

impl From<&str> for Prop {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

/// A Rust boolean expression and its optional stable helper name.
///
/// The token stream is retained, rather than stringified and reparsed, so an
/// attribute proc macro can interpolate the original tokens into generated
/// helper functions without losing their source spans.
#[derive(Clone, Debug)]
pub struct RustProp {
    /// Optional stable name used for the generated Rust helper function.
    pub name: Option<String>,
    /// The original Rust expression tokens.
    pub expr: proc_macro2::TokenStream,
    /// The Rust item that owns the generated condition helper.
    ///
    /// This is filled in by source scanning. The proc macro derives the same
    /// owner directly from the item it expands.
    pub owner: Option<RustConditionOwner>,
}

impl PartialEq for RustProp {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.expr.to_string() == other.expr.to_string()
            && self.owner == other.owner
    }
}

impl Eq for RustProp {}

impl RustProp {
    /// Constructs an unnamed Rust proposition from expression tokens.
    pub fn new(expr: proc_macro2::TokenStream) -> Self {
        Self { name: None, expr, owner: None }
    }

    /// Returns the generated Rust helper name for this condition.
    pub fn helper_name(&self, role: RustConditionRole, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| format!("{}{}", role.stem(), index + 1))
    }
}

/// Role of a Rust-authored condition in a function contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RustConditionRole {
    /// Function precondition.
    Pre,
    /// Function invariant.
    Invariant,
    /// Function postcondition.
    Post,
}

impl RustConditionRole {
    /// Stable prefix used for automatically named helpers.
    pub const fn stem(self) -> &'static str {
        match self {
            Self::Pre => "pre",
            Self::Invariant => "inv",
            Self::Post => "post",
        }
    }
}

/// Rust source item that owns a generated condition helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RustConditionOwner {
    /// A helper in `anneal__<function>`.
    Function { module_path: Vec<String>, function: String },
}

/// A Lean proposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeanProp {
    /// Optional stable name used when generating a Lean definition for this condition.
    pub name: Option<String>,
    /// Lean source for the proposition.
    pub prop: String,
}

impl From<String> for LeanProp {
    fn from(value: String) -> Self {
        Self { name: None, prop: value }
    }
}

impl From<&str> for LeanProp {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

/// A Lean proof fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    /// A proof implemented in Lean.
    Lean(LeanProof),
}

/// A Lean proof fragment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanProof {
    /// Tactic-style proof inserted after `:= by`.
    TacticProof(String),
    /// Term proof inserted after `:=`.
    TermProof(String),
}

impl From<String> for LeanProof {
    fn from(value: String) -> Self {
        Self::TacticProof(value)
    }
}

/// Parse all Anneal attributes from a Rust attribute slice.
pub fn parse_attributes(attrs: &[syn::Attribute]) -> Result<Attributes> {
    decode::parse_attributes(attrs, AnnotationTarget::Any)
}

/// Parse Anneal attributes with item-kind-aware restrictions enabled.
pub fn parse_attributes_for_target(
    attrs: &[syn::Attribute],
    target: AnnotationTarget,
) -> Result<Attributes> {
    decode::parse_attributes(attrs, target)
}

/// Parse Anneal attributes attached directly to a Rust item.
pub fn parse_item_attributes(item: &syn::Item) -> Result<Attributes> {
    match item {
        syn::Item::Fn(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Function),
        syn::Item::Struct(item) => {
            parse_attributes_for_target(&item.attrs, AnnotationTarget::Struct)
        }
        syn::Item::Trait(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Trait),
        syn::Item::Impl(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Impl),
        syn::Item::Enum(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Other),
        syn::Item::Mod(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Other),
        syn::Item::Type(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Other),
        syn::Item::Const(item) => parse_attributes_for_target(&item.attrs, AnnotationTarget::Other),
        syn::Item::Static(item) => {
            parse_attributes_for_target(&item.attrs, AnnotationTarget::Other)
        }
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn parse_one(attr: syn::Attribute) -> Attributes {
        parse_attributes(&[attr]).unwrap()
    }

    fn lean_prop(prop: &str) -> Prop {
        Prop::Lean(LeanProp::from(prop))
    }

    fn rust_prop(prop: proc_macro2::TokenStream) -> Prop {
        Prop::Rust(RustProp::new(prop))
    }

    fn lean_prop_with_name(name: &str, prop: &str) -> Prop {
        Prop::Lean(LeanProp { name: Some(name.into()), prop: prop.into() })
    }

    #[test]
    fn spec_value_defaults_to_requires_for_lean_prop() {
        let attrs = parse_one(parse_quote!(#[anneal(spec = "x != 42#U32")]));

        assert_eq!(
            attrs,
            vec![Attribute {
                data: AttributeData::Spec(Spec {
                    requires: vec![lean_prop("x != 42#U32")],
                    ensures: vec![],
                    invariants: vec![],
                }),
                domains: vec!["soundness".into()],
            }]
        );
    }

    #[test]
    fn spec_requires_forms_are_equivalent_inside_anneal_list() {
        let default = parse_one(parse_quote!(#[anneal(spec = "x != 42#U32")]));
        let explicit_path = parse_one(parse_quote!(#[anneal(spec::requires = "x != 42#U32")]));
        let nested = parse_one(parse_quote!(#[anneal(spec(requires = "x != 42#U32"))]));

        assert_eq!(default, explicit_path);
        assert_eq!(default, nested);
    }

    #[test]
    fn cfg_attr_anneal_wrapper_is_rejected() {
        let err = parse_attributes(&[
            parse_quote!(#[cfg_attr(anneal, anneal(spec(requires = "x != 42#U32")))]),
        ])
        .unwrap_err();

        assert_eq!(err, AnnotationError::CfgAttrWrapperUnsupported);
    }

    #[test]
    fn canonical_qualified_macro_path_matches_bare_path() {
        let bare = parse_one(parse_quote!(#[anneal(spec(requires = x != 42))]));
        let qualified = parse_one(parse_quote!(#[anneal_macros::anneal(spec(requires = x != 42))]));

        assert_eq!(qualified, bare);
    }

    #[test]
    fn cfg_attr_qualified_anneal_wrapper_is_rejected() {
        let err = parse_attributes(&[parse_quote!(
            #[cfg_attr(anneal, anneal_macros::anneal(spec(requires = x != 42)))]
        )])
        .unwrap_err();

        assert_eq!(err, AnnotationError::CfgAttrWrapperUnsupported);
    }

    #[test]
    fn spec_accepts_rust_language_prop() {
        let attrs = parse_attributes(&[parse_quote!(#[anneal(spec(requires = x != 42))])]).unwrap();

        assert_eq!(
            attrs[0].data,
            AttributeData::Spec(Spec {
                requires: vec![rust_prop(quote::quote!(x != 42))],
                ensures: vec![],
                invariants: vec![],
            })
        );
    }

    #[test]
    fn rust_prop_accepts_commas_inside_turbofish_arguments() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires(
                    name = "generic_predicate",
                    prop = predicate::<u8, u16>(x),
                ),
                ensures = ret == x,
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        let mut predicate = RustProp::new(quote::quote!(predicate::<u8, u16>(x)));
        predicate.name = Some("generic_predicate".into());
        assert_eq!(spec.requires, vec![Prop::Rust(predicate)]);
        assert_eq!(spec.ensures, vec![rust_prop(quote::quote!(ret == x))]);
    }

    #[test]
    fn spec_nested_fields_accumulate() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires = "x > 42#U32",
                requires(
                    name = "bounded",
                    prop = "z < 42#U32"
                ),
                ensures = "True",
                invariant = "loop_inv"
            ))]
        ));

        let [attr] = attrs.as_slice() else {
            panic!("expected one attr");
        };
        let AttributeData::Spec(spec) = &attr.data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires.len(), 2);
        assert_eq!(spec.requires[0], lean_prop("x > 42#U32"));
        assert_eq!(spec.requires[1], lean_prop_with_name("bounded", "z < 42#U32"));
        assert_eq!(spec.ensures, vec![lean_prop("True")]);
        assert_eq!(spec.invariants, vec![lean_prop("loop_inv")]);
    }

    #[test]
    fn removed_prop_attrs_are_rejected() {
        for field in [quote::quote!(attr = "simp"), quote::quote!(attrs = ["simp", "grind"])] {
            let attr: syn::Attribute =
                parse_quote!(#[anneal(spec(requires(#field, prop = "True")))]);
            let err = parse_attributes(&[attr]).unwrap_err();
            assert!(matches!(err, AnnotationError::UnknownPath(_)));
        }
    }

    #[test]
    fn anneal_wrapper_can_contain_repeated_attributes() {
        let attrs = parse_one(parse_quote!(
            #[anneal(
                spec = "x != 42#U32",
                proof::term = "fun h => h"
            )]
        ));

        assert_eq!(
            attrs,
            vec![
                Attribute {
                    data: AttributeData::Spec(Spec {
                        requires: vec![lean_prop("x != 42#U32")],
                        ensures: vec![],
                        invariants: vec![],
                    }),
                    domains: vec!["soundness".into()],
                },
                Attribute {
                    data: AttributeData::Proof(Proof::Lean(LeanProof::TermProof(
                        "fun h => h".into()
                    ))),
                    domains: vec!["soundness".into()],
                }
            ]
        );
    }

    #[test]
    fn implicit_proof_forms_are_rejected() {
        for attr in [
            parse_quote!(#[anneal(proof)]),
            parse_quote!(#[anneal(proof())]),
            parse_quote!(#[anneal(proof(lean))]),
            parse_quote!(#[anneal(proof(lean::tactic))]),
            parse_quote!(#[anneal(proof(auto))]),
        ] {
            assert!(parse_attributes(&[attr]).is_err());
        }
    }

    #[test]
    fn proof_tactic_forms_are_equivalent_inside_anneal_list() {
        let default = parse_one(parse_quote!(#[anneal(proof = "trivial")]));
        let explicit_path = parse_one(parse_quote!(#[anneal(proof::lean::tactic = "trivial")]));
        let nested = parse_one(parse_quote!(#[anneal(proof(lean::tactic = "trivial"))]));

        assert_eq!(default, explicit_path);
        assert_eq!(default, nested);
    }

    #[test]
    fn proof_term_inserts_lean_default_node() {
        let direct = parse_one(parse_quote!(#[anneal(proof::term = "exact h")]));
        let explicit = parse_one(parse_quote!(#[anneal(proof::lean::term = "exact h")]));
        let nested = parse_one(parse_quote!(#[anneal(proof(lean::term = "exact h"))]));

        assert_eq!(direct, explicit);
        assert_eq!(direct, nested);
        assert_eq!(
            direct[0].data,
            AttributeData::Proof(Proof::Lean(LeanProof::TermProof("exact h".into())))
        );
    }

    #[test]
    fn nested_lists_accept_trailing_commas() {
        let spec_attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires(
                    name = "positive",
                    prop = "x > 42#U32",
                ),
            ))]
        ));
        let AttributeData::Spec(spec) = &spec_attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop_with_name("positive", "x > 42#U32")]);
    }

    #[test]
    fn spec_structured_prop_accepts_name() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires(
                    name = "positive",
                    prop = "x.val > 0"
                )
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop_with_name("positive", "x.val > 0")]);
    }

    #[test]
    fn spec_structured_rust_prop_accepts_name() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires(
                    name = "positive",
                    prop = x > 0,
                ),
                ensures(
                    name = "pass_thru",
                    prop = ret == x,
                ),
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        let mut positive = RustProp::new(quote::quote!(x > 0));
        positive.name = Some("positive".into());
        let mut pass_thru = RustProp::new(quote::quote!(ret == x));
        pass_thru.name = Some("pass_thru".into());
        assert_eq!(spec.requires, vec![Prop::Rust(positive)]);
        assert_eq!(spec.ensures, vec![Prop::Rust(pass_thru)]);
    }

    #[test]
    fn spec_path_composed_prop_accepts_name_and_prop() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires::name = "positive",
                requires::prop = "x.val > 0",
                ensures::name = "same_value",
                ensures = "result.val = x.val",
                invariant::name = "bounded",
                invariant::prop = "x.val <= 10"
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop_with_name("positive", "x.val > 0")]);
        assert_eq!(spec.ensures, vec![lean_prop_with_name("same_value", "result.val = x.val")]);
        assert_eq!(spec.invariants, vec![lean_prop_with_name("bounded", "x.val <= 10")]);
    }

    #[test]
    fn spec_path_composed_name_requires_prop() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(spec(
                requires::name = "positive"
            ))]
        )])
        .unwrap_err();

        assert_eq!(err, AnnotationError::MissingRequired("spec::requires::prop".into()));
    }

    #[test]
    fn non_anneal_attributes_are_ignored() {
        let attrs = parse_attributes(&[
            parse_quote!(#[inline]),
            parse_quote!(#[cfg_attr(test, allow(dead_code))]),
        ])
        .unwrap();

        assert!(attrs.is_empty());
    }

    #[test]
    fn rejects_unknown_non_default_child() {
        let err = parse_attributes(&[parse_quote!(#[anneal(spec::bogus = "x")])]).unwrap_err();

        assert_eq!(err, AnnotationError::UnknownPath("spec::bogus".into()));
    }

    #[test]
    fn removed_annotation_constructs_are_rejected() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(theorem(
                name = "a",
                definition(ty = "True", proof = "trivial"),
            ))]
        )])
        .unwrap_err();
        assert!(matches!(err, AnnotationError::UnknownPath(_)));
    }

    #[test]
    fn domains_default_to_soundness_and_explicit_domains_replace_the_default() {
        let default = parse_one(parse_quote!(#[anneal(spec(requires = "True"))]));
        assert_eq!(default[0].domains, vec!["soundness"]);

        let explicit = parse_one(parse_quote!(
            #[anneal(spec(
                requires = "True",
                domain = "documentation",
                domains = ["soundness", "custom"],
                domain = "local",
            ))]
        ));
        assert_eq!(explicit[0].domains, vec!["documentation", "soundness", "custom", "local"]);

        let empty = parse_one(parse_quote!(
            #[anneal(spec(requires = "True", domains = []))]
        ));
        assert!(empty[0].domains.is_empty());
    }

    #[test]
    fn rejects_duplicate_explicit_proof() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(proof(
                lean::tactic = "one",
                lean::tactic = "two"
            ))]
        )])
        .unwrap_err();

        assert_eq!(err, AnnotationError::Duplicate("proof::lean::tactic".into()));
    }

    #[test]
    fn function_items_accept_preconditions_and_postconditions() {
        let item: syn::Item = parse_quote!(
            #[anneal(spec(requires = "x.val > 0", ensures = "result.val >= x.val"))]
            pub fn increment(x: u32) -> u32 {
                x + 1
            }
        );

        let attrs = parse_item_attributes(&item).unwrap();
        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop("x.val > 0")]);
        assert_eq!(spec.ensures, vec![lean_prop("result.val >= x.val")]);
    }

    #[test]
    fn non_function_items_reject_specs() {
        for (attr, path) in [
            (parse_quote!(#[anneal(spec(invariant = "Counter.Valid self"))]), "spec"),
            (parse_quote!(#[anneal(proof = "trivial")]), "proof::lean::tactic"),
        ] {
            let err = parse_attributes_for_target(&[attr], AnnotationTarget::Struct).unwrap_err();
            assert_eq!(
                err,
                AnnotationError::UnsupportedTarget {
                    path: path.into(),
                    target: AnnotationTarget::Struct,
                }
            );
        }
    }

    #[test]
    fn empty_lists_are_rejected() {
        for attr in [
            parse_quote!(#[anneal()]),
            parse_quote!(#[anneal(spec())]),
            parse_quote!(#[anneal(proof())]),
            parse_quote!(#[anneal(proof(lean()))]),
        ] {
            assert!(matches!(
                parse_attributes(&[attr]).unwrap_err(),
                AnnotationError::NonLeafPath(_)
            ));
        }
    }

    #[test]
    fn legacy_outer_qualified_spelling_is_rejected() {
        let legacy: syn::Attribute = parse_quote!(#[anneal::spec::requires(prop = flag == true)]);

        assert_eq!(
            parse_attributes(&[legacy]).unwrap_err(),
            AnnotationError::UnknownPath("anneal::spec::requires".into())
        );
    }

    #[test]
    fn expansion_markers_are_stable_and_invisible_rustdoc_comments() {
        let bare: syn::Attribute = parse_quote!(#[anneal(spec(requires(prop = flag == true)))]);
        let qualified: syn::Attribute =
            parse_quote!(#[anneal_macros::anneal(spec(requires(prop = flag == true)))]);
        let unrelated: syn::Attribute = parse_quote!(#[allow(dead_code)]);
        let legacy_leaf: syn::Attribute =
            parse_quote!(#[anneal::spec::requires(prop = flag == true)]);

        let marker = expansion_marker_for_attribute(&bare).unwrap().unwrap();
        assert_eq!(
            expansion_marker_for_attribute(&qualified).unwrap().as_deref(),
            Some(marker.as_str())
        );
        assert!(marker.starts_with("<!--"));
        assert!(marker.ends_with("-->"));
        assert!(is_expansion_marker(&marker));
        assert_eq!(expansion_marker_for_attribute(&unrelated).unwrap(), None);
        assert_eq!(expansion_marker_for_attribute(&legacy_leaf).unwrap(), None);
        assert!(!is_expansion_marker("<!--anneal-expanded-v1:not-hex-->"));
    }
}
