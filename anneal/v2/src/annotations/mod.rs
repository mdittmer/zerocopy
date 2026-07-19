// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Parsing for Anneal v2 Rust attributes.
//!
//! The parser is split into three small stages:
//! - [`schema`] defines legal paths and default children.
//! - [`parse`] matches raw Rust attribute syntax against that schema.
//! - [`decode`] converts matched entries into Anneal data structures.
//!
//! This module is intentionally landed before the generation pipeline calls it.
//! Keep unused warnings quiet until the interpretation stage is wired in.
#![allow(dead_code, unused_imports)]

mod decode;
mod error;
mod parse;
mod schema;

pub use error::{AnnotationError, Result};
pub use schema::AnnotationTarget;

/// All Anneal attributes attached to one Rust item.
pub type Attributes = Vec<Attribute>;

/// One decoded `#[anneal(...)]` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    /// Semantic content of the attribute.
    pub data: AttributeData,
    /// Proof domains to which this attribute applies.
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
    /// Auxiliary theorem declaration.
    Theorem(Theorem),
    /// Proof fragment.
    Proof(Proof),
}

impl Default for AttributeData {
    fn default() -> Self {
        Self::Spec(Spec::default())
    }
}

/// Function or type specifications.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spec {
    /// Preconditions.
    pub requires: Vec<Prop>,
    /// Postconditions.
    pub ensures: Vec<Prop>,
    /// Invariants.
    pub invariants: Vec<Prop>,
}

/// A proposition. The initial v2 implementation supports only Lean source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prop {
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

/// A Lean proposition and optional Lean attributes attached to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeanProp {
    /// Optional stable name used when generating a Lean definition for this condition.
    pub name: Option<String>,
    /// Lean attributes attached to the generated declaration for this condition.
    pub attrs: LeanAttrs,
    /// Lean source for the proposition.
    pub prop: String,
}

impl From<String> for LeanProp {
    fn from(value: String) -> Self {
        Self { name: None, attrs: Default::default(), prop: value }
    }
}

impl From<&str> for LeanProp {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

/// An auxiliary theorem that may be referenced by generated proofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theorem {
    /// Lean attributes attached to the theorem declaration.
    pub attrs: LeanAttrs,
    /// The Lean theorem name.
    pub name: String,
    /// The theorem definition.
    pub definition: TheoremDefinition,
}

/// Theorem definition language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TheoremDefinition {
    /// A theorem definition written in Lean.
    Lean(LeanTheoremDefinition),
}

/// A theorem definition written in Lean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeanTheoremDefinition {
    /// Arguments passed to the theorem.
    pub args: Vec<LeanArgs>,
    /// The theorem type.
    pub ty: LeanType,
    /// The proof.
    pub proof: LeanProof,
}

/// Arguments passed to a Lean function, theorem, or definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanArgs {
    /// Structured arguments expressed using slice-like syntax.
    Args(Vec<LeanArg>),
    /// Full argument declarations as an inline string.
    ArgsString(String),
}

impl From<String> for LeanArgs {
    fn from(value: String) -> Self {
        Self::ArgsString(value)
    }
}

/// One Lean argument declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanArg {
    /// Structured argument written as `"name" : "Type"`.
    Arg { name: String, ty: LeanType },
    /// Inline argument declaration.
    ArgString(String),
}

impl From<String> for LeanArg {
    fn from(value: String) -> Self {
        Self::ArgString(value)
    }
}

/// Reference to a Lean type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeanType(pub String);

/// Lean attributes attached to a definition, theorem, proposition, etc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanAttrs {
    /// Structured attributes.
    Attrs(Vec<LeanAttr>),
    /// Inline attribute declaration string.
    AttrsString(String),
}

impl Default for LeanAttrs {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<String> for LeanAttrs {
    fn from(value: String) -> Self {
        Self::AttrsString(value)
    }
}

impl From<&str> for LeanAttrs {
    fn from(value: &str) -> Self {
        Self::from(value.to_string())
    }
}

impl LeanAttrs {
    pub fn empty() -> Self {
        Self::Attrs(Vec::new())
    }
}

/// One Lean attribute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeanAttr {
    /// Inline Lean attribute text.
    AttrString(String),
}

impl From<String> for LeanAttr {
    fn from(value: String) -> Self {
        Self::AttrString(value)
    }
}

impl From<&str> for LeanAttr {
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

impl Default for Proof {
    fn default() -> Self {
        Self::Lean(LeanProof::TacticProof(String::from("sorry")))
    }
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
    use super::*;
    use syn::parse_quote;

    fn parse_one(attr: syn::Attribute) -> Attributes {
        parse_attributes(&[attr]).unwrap()
    }

    fn lean_prop(prop: &str) -> Prop {
        Prop::Lean(LeanProp::from(prop))
    }

    fn lean_prop_with_attrs(attrs: &[&str], prop: &str) -> Prop {
        Prop::Lean(LeanProp { name: None, attrs: lean_attrs(attrs), prop: prop.into() })
    }

    fn lean_prop_with_name(name: &str, prop: &str) -> Prop {
        Prop::Lean(LeanProp {
            name: Some(name.into()),
            attrs: LeanAttrs::default(),
            prop: prop.into(),
        })
    }

    fn lean_prop_with_name_and_attrs(name: &str, attrs: &[&str], prop: &str) -> Prop {
        Prop::Lean(LeanProp {
            name: Some(name.into()),
            attrs: lean_attrs(attrs),
            prop: prop.into(),
        })
    }

    fn lean_attrs(attrs: &[&str]) -> LeanAttrs {
        LeanAttrs::Attrs(attrs.iter().copied().map(LeanAttr::from).collect())
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
    fn spec_rejects_rust_language_prop_for_now() {
        let err =
            parse_attributes(&[parse_quote!(#[anneal(spec(requires = x != 42))])]).unwrap_err();

        assert_eq!(
            err,
            AnnotationError::UnsupportedValue(
                "Rust-language propositions are not implemented yet; write Lean propositions as string literals, got `x != 42`".into()
            )
        );
    }

    #[test]
    fn spec_nested_fields_accumulate_and_domains_replace_default() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires = "x > 42#U32",
                requires(
                    attrs = ["step", "grind"],
                    prop = "z < 42#U32"
                ),
                ensures = "True",
                invariant = "loop_inv",
                domain = "fast",
                domains = ["soundness", "custom"],
                domain = "local"
            ))]
        ));

        let [attr] = attrs.as_slice() else {
            panic!("expected one attr");
        };
        assert_eq!(attr.domains, vec!["fast", "soundness", "custom", "local"]);
        let AttributeData::Spec(spec) = &attr.data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires.len(), 2);
        assert_eq!(spec.requires[0], lean_prop("x > 42#U32"));
        assert_eq!(spec.requires[1], lean_prop_with_attrs(&["step", "grind"], "z < 42#U32"));
        assert_eq!(spec.ensures, vec![lean_prop("True")]);
        assert_eq!(spec.invariants, vec![lean_prop("loop_inv")]);
    }

    #[test]
    fn spec_structured_prop_accepts_inline_attrs_string() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                ensures(
                    attrs = "simp",
                    prop = "True"
                )
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(
            spec.ensures,
            vec![Prop::Lean(LeanProp {
                name: None,
                attrs: LeanAttrs::AttrsString("simp".into()),
                prop: "True".into(),
            })]
        );
    }

    #[test]
    fn spec_structured_prop_accepts_singular_attr_repeated() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                ensures(
                    attr = "simp",
                    attr = "grind",
                    prop = "True"
                )
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(
            spec.ensures,
            vec![Prop::Lean(LeanProp {
                name: None,
                attrs: LeanAttrs::Attrs(vec![
                    LeanAttr::AttrString("simp".into()),
                    LeanAttr::AttrString("grind".into()),
                ]),
                prop: "True".into(),
            })]
        );
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
    fn proof_flag_defaults_to_lean_tactic_sorry() {
        let attrs = parse_one(parse_quote!(#[anneal(proof)]));

        assert_eq!(
            attrs,
            vec![Attribute {
                data: AttributeData::Proof(Proof::Lean(LeanProof::TacticProof("sorry".into()))),
                domains: vec!["soundness".into()],
            }]
        );
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
    fn theorem_definition_defaults_to_lean_and_proof_defaults_to_sorry() {
        let attrs = parse_one(parse_quote!(
            #[anneal(theorem(
                name = "a_sufficiently_large",
                definition(
                    args = [["a" : "U32"]],
                    ty = "a > 42#U32"
                )
            ))]
        ));

        assert_eq!(
            attrs,
            vec![Attribute {
                data: AttributeData::Theorem(Theorem {
                    attrs: LeanAttrs::empty(),
                    name: "a_sufficiently_large".into(),
                    definition: TheoremDefinition::Lean(LeanTheoremDefinition {
                        args: vec![LeanArgs::Args(vec![LeanArg::Arg {
                            name: "a".into(),
                            ty: LeanType("U32".into()),
                        }])],
                        ty: LeanType("a > 42#U32".into()),
                        proof: LeanProof::TacticProof("sorry".into()),
                    }),
                }),
                domains: vec!["soundness".into()],
            }]
        );
    }

    #[test]
    fn theorem_accepts_singular_and_plural_lean_attrs() {
        let attrs = parse_one(parse_quote!(
            #[anneal(theorem(
                attr = "grind",
                attrs = ["scalar_tac"],
                attr = "local",
                name = "processed_a_sufficiently_large",
                definition::lean(
                    args = "(a : U32) (b : ProcessedA)",
                    ty = "a > 42#U32"
                )
            ))]
        ));

        assert_eq!(
            attrs[0].data,
            AttributeData::Theorem(Theorem {
                attrs: lean_attrs(&["grind", "scalar_tac", "local"]),
                name: "processed_a_sufficiently_large".into(),
                definition: TheoremDefinition::Lean(LeanTheoremDefinition {
                    args: vec![LeanArgs::ArgsString("(a : U32) (b : ProcessedA)".into())],
                    ty: LeanType("a > 42#U32".into()),
                    proof: LeanProof::TacticProof("sorry".into()),
                }),
            })
        );
    }

    #[test]
    fn theorem_accepts_singular_arg_repeated() {
        let attrs = parse_one(parse_quote!(
            #[anneal(theorem(
                name = "singular_args",
                definition(
                    arg = "a : U32",
                    arg = ["b" : "U32"],
                    ty = "True"
                )
            ))]
        ));

        assert_eq!(
            attrs[0].data,
            AttributeData::Theorem(Theorem {
                attrs: LeanAttrs::empty(),
                name: "singular_args".into(),
                definition: TheoremDefinition::Lean(LeanTheoremDefinition {
                    args: vec![
                        LeanArgs::ArgsString("a : U32".into()),
                        LeanArgs::Args(vec![LeanArg::Arg {
                            name: "b".into(),
                            ty: LeanType("U32".into()),
                        }]),
                    ],
                    ty: LeanType("True".into()),
                    proof: LeanProof::TacticProof("sorry".into()),
                }),
            })
        );
    }

    #[test]
    fn nested_lists_accept_trailing_commas() {
        let spec_attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires(
                    attrs = ["step", "grind",],
                    prop = "x > 42#U32",
                ),
            ))]
        ));
        let AttributeData::Spec(spec) = &spec_attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop_with_attrs(&["step", "grind"], "x > 42#U32")]);
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
    fn spec_path_composed_prop_accepts_name_and_prop() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires::name = "positive",
                requires::prop = "x.val > 0",
                ensures::name = "same_value",
                ensures = "result.val = x.val",
                invariant::name = "bounded",
                invariant::attr = "simp",
                invariant::prop = "x.val <= 10"
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.requires, vec![lean_prop_with_name("positive", "x.val > 0")]);
        assert_eq!(spec.ensures, vec![lean_prop_with_name("same_value", "result.val = x.val")]);
        assert_eq!(
            spec.invariants,
            vec![lean_prop_with_name_and_attrs("bounded", &["simp"], "x.val <= 10")]
        );
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
    fn rejects_duplicate_single_field_after_default_insertion() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(theorem(
                name = "a",
                definition(
                    ty = "True",
                    proof = "one",
                    proof::tactic = "two"
                )
            ))]
        )])
        .unwrap_err();

        assert_eq!(
            err,
            AnnotationError::Duplicate("theorem::definition::lean::proof::tactic".into())
        );
    }

    #[test]
    fn rejects_missing_theorem_name() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(theorem(
                definition(ty = "True")
            ))]
        )])
        .unwrap_err();

        assert_eq!(err, AnnotationError::MissingRequired("theorem::name".into()));
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
    fn struct_items_accept_invariants_but_reject_preconditions() {
        let item: syn::ItemStruct = parse_quote!(
            #[anneal(spec(invariant = "Counter.Valid self"))]
            pub struct Counter {
                value: u32,
            }
        );
        let attrs = parse_attributes_for_target(&item.attrs, AnnotationTarget::Struct).unwrap();
        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(spec.invariants, vec![lean_prop("Counter.Valid self")]);

        let item: syn::ItemStruct = parse_quote!(
            #[anneal(spec(requires = "x > 0"))]
            pub struct Bad {
                value: u32,
            }
        );
        let err = parse_attributes_for_target(&item.attrs, AnnotationTarget::Struct).unwrap_err();
        assert_eq!(
            err,
            AnnotationError::UnsupportedTarget {
                path: "spec::requires".into(),
                target: AnnotationTarget::Struct,
            }
        );
    }
}
