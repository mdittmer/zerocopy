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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Spec {
    /// Preconditions.
    pub requires: Vec<Prop>,
    /// Postconditions.
    pub ensures: Vec<Prop>,
    /// Invariants.
    pub invariants: Vec<Prop>,
}

/// A proposition. The initial v2 implementation supports only Lean source.
#[derive(Debug, PartialEq, Eq)]
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

/// A Lean proposition.
#[derive(Debug, PartialEq, Eq)]
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

/// A proof fragment.
#[derive(Debug, PartialEq, Eq)]
pub enum Proof {
    /// A proof implemented in Lean.
    Lean(LeanProof),
}

/// A Lean proof fragment.
#[derive(Debug, PartialEq, Eq)]
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

    fn named_lean_prop(name: &str, prop: &str) -> Prop {
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
    fn spec_fields_accumulate_and_explicit_domains_replace_default() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(
                requires = "x > 42#U32",
                requires(name = "small", prop = "z < 42#U32"),
                ensures = "True",
                invariant = "loop_inv",
                domain = "fast",
                domains = ["soundness", "custom"],
                domain = "local"
            ))]
        ));

        let AttributeData::Spec(spec) = &attrs[0].data else {
            panic!("expected spec");
        };
        assert_eq!(attrs[0].domains, vec!["fast", "soundness", "custom", "local"]);
        assert_eq!(
            spec.requires,
            vec![lean_prop("x > 42#U32"), named_lean_prop("small", "z < 42#U32")]
        );
        assert_eq!(spec.ensures, vec![lean_prop("True")]);
        assert_eq!(spec.invariants, vec![lean_prop("loop_inv")]);
    }

    #[test]
    fn proof_domains_replace_default_and_accumulate() {
        let attrs = parse_one(parse_quote!(
            #[anneal(proof(
                lean::tactic = "trivial",
                domain = "fast",
                domains = ["soundness", "custom"]
            ))]
        ));

        assert_eq!(
            attrs,
            vec![Attribute {
                data: AttributeData::Proof(Proof::Lean(LeanProof::TacticProof("trivial".into()))),
                domains: vec!["fast".into(), "soundness".into(), "custom".into()],
            }]
        );
    }

    #[test]
    fn explicitly_empty_domains_select_nothing() {
        let attrs = parse_one(parse_quote!(
            #[anneal(spec(requires = "True", domains = []))]
        ));

        assert!(attrs[0].domains.is_empty());
    }

    #[test]
    fn explicit_lean_tactic_forms_are_equivalent() {
        let path = parse_one(parse_quote!(
            #[anneal(proof::lean::tactic = "trivial")]
        ));
        let nested_path = parse_one(parse_quote!(
            #[anneal(proof(lean::tactic = "trivial"))]
        ));
        let nested_list = parse_one(parse_quote!(
            #[anneal(proof(lean(tactic = "trivial")))]
        ));

        assert_eq!(path, nested_path);
        assert_eq!(path, nested_list);
    }

    #[test]
    fn explicit_lean_term_forms_are_equivalent() {
        let path = parse_one(parse_quote!(
            #[anneal(proof::lean::term = "exact h")]
        ));
        let nested_path = parse_one(parse_quote!(
            #[anneal(proof(lean::term = "exact h"))]
        ));
        let nested_list = parse_one(parse_quote!(
            #[anneal(proof(lean(term = "exact h")))]
        ));

        assert_eq!(path, nested_path);
        assert_eq!(path, nested_list);
        assert_eq!(
            path[0].data,
            AttributeData::Proof(Proof::Lean(LeanProof::TermProof("exact h".into())))
        );
    }

    #[test]
    fn wrapper_can_contain_repeated_attributes() {
        let attrs = parse_one(parse_quote!(
            #[anneal(
                spec = "x != 42#U32",
                proof::lean::term = "fun h => h"
            )]
        ));

        assert_eq!(attrs.len(), 2);
        assert!(matches!(attrs[0].data, AttributeData::Spec(_)));
        assert!(matches!(
            attrs[1].data,
            AttributeData::Proof(Proof::Lean(LeanProof::TermProof(_)))
        ));
    }

    #[test]
    fn rejects_rust_language_prop_for_now() {
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
    fn rejects_condition_attrs() {
        for attr in [
            parse_quote!(#[anneal(spec(requires(attr = "simp", prop = "True")))]),
            parse_quote!(#[anneal(spec(requires(attrs = ["simp"], prop = "True")))]),
        ] {
            assert!(matches!(parse_attributes(&[attr]), Err(AnnotationError::UnknownPath(_))));
        }
    }

    #[test]
    fn rejects_theorem_syntax() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(theorem(name = "helper"))]
        )])
        .unwrap_err();

        assert_eq!(err, AnnotationError::UnknownPath("theorem".into()));
    }

    #[test]
    fn rejects_automatic_and_implicit_proof_syntax() {
        for attr in [
            parse_quote!(#[anneal(proof(auto))]),
            parse_quote!(#[anneal(proof = "trivial")]),
            parse_quote!(#[anneal(proof::tactic = "trivial")]),
        ] {
            assert!(parse_attributes(&[attr]).is_err());
        }
    }

    #[test]
    fn rejects_bare_and_empty_lists() {
        for attr in [
            parse_quote!(#[anneal()]),
            parse_quote!(#[anneal(spec)]),
            parse_quote!(#[anneal(spec())]),
            parse_quote!(#[anneal(proof)]),
            parse_quote!(#[anneal(proof())]),
            parse_quote!(#[anneal(proof(lean))]),
            parse_quote!(#[anneal(proof(lean()))]),
        ] {
            assert!(parse_attributes(&[attr]).is_err());
        }
    }

    #[test]
    fn rejects_parser_only_outer_qualified_paths() {
        for attr in [
            parse_quote!(#[anneal::spec(requires = "True")]),
            parse_quote!(#[anneal::proof::lean::tactic = "trivial"]),
        ] {
            assert!(matches!(parse_attributes(&[attr]), Err(AnnotationError::UnknownPath(_))));
        }
    }

    #[test]
    fn cfg_attr_anneal_wrapper_is_rejected() {
        let err =
            parse_attributes(&[parse_quote!(#[cfg_attr(anneal, anneal(spec(requires = "True")))])])
                .unwrap_err();

        assert_eq!(err, AnnotationError::CfgAttrWrapperUnsupported);
    }

    #[test]
    fn named_conditions_require_a_proposition() {
        let err = parse_attributes(&[parse_quote!(
            #[anneal(spec(requires::name = "positive"))]
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
    fn free_functions_accept_all_spec_condition_kinds() {
        let item: syn::Item = parse_quote!(
            #[anneal(spec(
                requires = "x.val > 0",
                ensures = "result.val >= x.val",
                invariant = "True"
            ))]
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
        assert_eq!(spec.invariants, vec![lean_prop("True")]);
    }

    #[test]
    fn non_function_items_reject_specs() {
        let item: syn::ItemStruct = parse_quote!(
            #[anneal(spec(invariant = "True"))]
            pub struct Counter {
                value: u32,
            }
        );

        let err = parse_attributes_for_target(&item.attrs, AnnotationTarget::Struct).unwrap_err();
        assert_eq!(
            err,
            AnnotationError::UnsupportedTarget {
                path: "spec::invariant".into(),
                target: AnnotationTarget::Struct,
            }
        );
    }
}
