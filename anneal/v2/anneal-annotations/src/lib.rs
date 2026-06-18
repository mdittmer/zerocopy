// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Expr, ExprLit, FnArg, Lit, LitStr, Meta, MetaList, MetaNameValue, Pat, PatIdent, Signature,
    Token, parse::Parser as _, punctuated::Punctuated,
};

const METADATA_PREFIX: &str = "anneal:v2:";

#[proc_macro_attribute]
pub fn spec(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(AnnotationKind::Spec, attr, item)
}

#[proc_macro_attribute]
pub fn unsafe_axiom(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(AnnotationKind::UnsafeAxiom, attr, item)
}

#[proc_macro_attribute]
pub fn type_invariant(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(AnnotationKind::TypeInvariant, attr, item)
}

#[proc_macro_attribute]
pub fn trait_invariant(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(AnnotationKind::TraitInvariant, attr, item)
}

#[proc_macro_attribute]
pub fn impl_invariant(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand(AnnotationKind::ImplInvariant, attr, item)
}

#[derive(Clone, Copy)]
enum AnnotationKind {
    Spec,
    UnsafeAxiom,
    TypeInvariant,
    TraitInvariant,
    ImplInvariant,
}

impl AnnotationKind {
    fn as_str(self) -> &'static str {
        match self {
            AnnotationKind::Spec => "spec",
            AnnotationKind::UnsafeAxiom => "unsafe_axiom",
            AnnotationKind::TypeInvariant => "type_invariant",
            AnnotationKind::TraitInvariant => "trait_invariant",
            AnnotationKind::ImplInvariant => "impl_invariant",
        }
    }

    fn allows(self, section: SectionKind) -> bool {
        use AnnotationKind::*;
        use SectionKind::*;
        matches!(
            (self, section),
            (Spec, Context | Requires | Ensures | ProofContext | Proof)
                | (UnsafeAxiom, Context | Requires | Ensures)
                | (TypeInvariant, Context | IsValid)
                | (TraitInvariant, Context | IsSafe)
                | (ImplInvariant, Context)
        )
    }

    fn injects_opacity(self) -> bool {
        matches!(self, AnnotationKind::UnsafeAxiom)
    }
}

#[derive(Clone, Copy, Debug)]
enum SectionKind {
    Context,
    Requires,
    Ensures,
    ProofContext,
    Proof,
    IsValid,
    IsSafe,
}

impl SectionKind {
    fn parse(path: &syn::Path) -> syn::Result<Self> {
        let Some(ident) = path.get_ident() else {
            return Err(syn::Error::new_spanned(path, "expected an Anneal section name"));
        };
        match ident.to_string().as_str() {
            "context" => Ok(Self::Context),
            "requires" => Ok(Self::Requires),
            "ensures" => Ok(Self::Ensures),
            "proof_context" => Ok(Self::ProofContext),
            "proof" => Ok(Self::Proof),
            "is_valid" => Ok(Self::IsValid),
            "is_safe" => Ok(Self::IsSafe),
            _ => Err(syn::Error::new_spanned(
                ident,
                "unknown Anneal section; expected one of context, requires, ensures, \
                 proof_context, proof, is_valid, or is_safe",
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Requires => "requires",
            Self::Ensures => "ensures",
            Self::ProofContext => "proof_context",
            Self::Proof => "proof",
            Self::IsValid => "is_valid",
            Self::IsSafe => "is_safe",
        }
    }
}

#[derive(Debug)]
struct Section {
    kind: SectionKind,
    name: Option<String>,
    lean: String,
}

fn expand(kind: AnnotationKind, attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_ts: proc_macro2::TokenStream = attr.into();
    let item_ts: proc_macro2::TokenStream = item.into();
    match parse_sections(kind, attr_ts) {
        Ok(sections) => expand_item(kind, sections, item_ts).into(),
        Err(err) => {
            let compile_error = err.to_compile_error();
            quote! {
                #compile_error
                #item_ts
            }
            .into()
        }
    }
}

fn parse_sections(
    kind: AnnotationKind,
    attr: proc_macro2::TokenStream,
) -> syn::Result<Vec<Section>> {
    let metas =
        Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr).map_err(|mut err| {
            err.combine(syn::Error::new(
                err.span(),
                "expected structured Anneal arguments, e.g. requires(name = \"Lean\")",
            ));
            err
        })?;
    let mut sections = Vec::new();
    for meta in metas {
        parse_meta(kind, meta, &mut sections)?;
    }
    Ok(sections)
}

fn parse_meta(kind: AnnotationKind, meta: Meta, sections: &mut Vec<Section>) -> syn::Result<()> {
    match meta {
        Meta::NameValue(name_value) => {
            let section = SectionKind::parse(&name_value.path)?;
            ensure_allowed(kind, section, &name_value)?;
            sections.push(Section {
                kind: section,
                name: None,
                lean: string_value(&name_value.value)?,
            });
        }
        Meta::List(list) => {
            let section = SectionKind::parse(&list.path)?;
            ensure_allowed(kind, section, &list)?;
            parse_named_sections(section, list, sections)?;
        }
        Meta::Path(path) => {
            return Err(syn::Error::new_spanned(
                path,
                "Anneal sections require a Lean string literal",
            ));
        }
    }
    Ok(())
}

fn ensure_allowed(
    kind: AnnotationKind,
    section: SectionKind,
    tokens: &impl quote::ToTokens,
) -> syn::Result<()> {
    if kind.allows(section) {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            tokens,
            format!(
                "`{}` sections are not valid on `#[anneal::{}]`",
                section.as_str(),
                kind.as_str()
            ),
        ))
    }
}

fn parse_named_sections(
    section: SectionKind,
    list: MetaList,
    sections: &mut Vec<Section>,
) -> syn::Result<()> {
    let entries = Punctuated::<MetaNameValue, Token![,]>::parse_terminated.parse2(list.tokens)?;
    if entries.is_empty() {
        return Err(syn::Error::new_spanned(
            list.path,
            "expected at least one named Lean string literal",
        ));
    }
    for entry in entries {
        let Some(ident) = entry.path.get_ident() else {
            return Err(syn::Error::new_spanned(entry.path, "expected a bound name"));
        };
        sections.push(Section {
            kind: section,
            name: Some(ident.to_string()),
            lean: string_value(&entry.value)?,
        });
    }
    Ok(())
}

fn string_value(expr: &Expr) -> syn::Result<String> {
    if let Expr::Lit(ExprLit { lit: Lit::Str(value), .. }) = expr {
        Ok(value.value())
    } else {
        Err(syn::Error::new_spanned(expr, "expected a string literal containing pure Lean code"))
    }
}

fn expand_item(
    kind: AnnotationKind,
    sections: Vec<Section>,
    item: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let argument_names = match argument_names_for_item(kind, item.clone()) {
        Ok(argument_names) => argument_names,
        Err(err) => {
            let compile_error = err.to_compile_error();
            return quote! {
                #compile_error
                #item
            };
        }
    };
    let metadata = LitStr::new(
        &metadata_json(kind, &sections, argument_names.as_deref()),
        proc_macro2::Span::call_site(),
    );
    let opaque = kind.injects_opacity().then(|| quote!(#[aeneas::opaque]));
    quote! {
        #opaque
        #[doc = #metadata]
        #item
    }
}

fn argument_names_for_item(
    kind: AnnotationKind,
    item: proc_macro2::TokenStream,
) -> syn::Result<Option<Vec<String>>> {
    if !matches!(kind, AnnotationKind::Spec | AnnotationKind::UnsafeAxiom) {
        return Ok(None);
    }

    if let Ok(item_fn) = syn::parse2::<syn::ItemFn>(item.clone()) {
        return argument_names_for_signature(&item_fn.sig).map(Some);
    }
    if let Ok(impl_fn) = syn::parse2::<syn::ImplItemFn>(item.clone()) {
        return argument_names_for_signature(&impl_fn.sig).map(Some);
    }
    if let Ok(trait_fn) = syn::parse2::<syn::TraitItemFn>(item.clone()) {
        return argument_names_for_signature(&trait_fn.sig).map(Some);
    }
    if let Ok(foreign_fn) = syn::parse2::<syn::ForeignItemFn>(item) {
        return argument_names_for_signature(&foreign_fn.sig).map(Some);
    }

    Ok(None)
}

fn argument_names_for_signature(sig: &Signature) -> syn::Result<Vec<String>> {
    sig.inputs.iter().map(argument_name).collect()
}

fn argument_name(arg: &FnArg) -> syn::Result<String> {
    match arg {
        FnArg::Receiver(_) => Ok("self".to_string()),
        FnArg::Typed(typed) => match typed.pat.as_ref() {
            Pat::Ident(PatIdent { ident, .. }) => Ok(ident.to_string()),
            pat => Err(syn::Error::new_spanned(
                pat,
                "Anneal requires annotated function parameters to be named identifiers",
            )),
        },
    }
}

fn metadata_json(
    kind: AnnotationKind,
    sections: &[Section],
    argument_names: Option<&[String]>,
) -> String {
    let sections = sections
        .iter()
        .map(|section| {
            serde_json::json!({
                "kind": section.kind.as_str(),
                "name": section.name,
                "lean": section.lean,
            })
        })
        .collect::<Vec<_>>();
    format!(
        "{}{}",
        METADATA_PREFIX,
        serde_json::json!({
            "version": 1,
            "kind": kind.as_str(),
            "sections": sections,
            "argument_names": argument_names,
        })
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse(kind: AnnotationKind, attr: proc_macro2::TokenStream) -> Vec<Section> {
        parse_sections(kind, attr).unwrap()
    }

    #[test]
    fn spec_sections_are_structured_and_lean_only() {
        let sections = parse(
            AnnotationKind::Spec,
            quote! {
                context = "open Aeneas",
                requires(nonzero = "b.val > 0"),
                ensures(result = "ret.val = a.val / b.val"),
                proof(result = "simp_all [safe_div]")
            },
        );

        assert_eq!(sections.len(), 4);
        assert!(matches!(sections[0].kind, SectionKind::Context));
        assert_eq!(sections[1].name.as_deref(), Some("nonzero"));
        assert_eq!(sections[1].lean, "b.val > 0");
        assert_eq!(sections[3].lean, "simp_all [safe_div]");
    }

    #[test]
    fn unsafe_axiom_rejects_proof_sections() {
        let err = parse_sections(
            AnnotationKind::UnsafeAxiom,
            quote! {
                proof(result = "simp")
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("not valid"));
    }

    #[test]
    fn metadata_is_json_with_stable_prefix() {
        let sections = parse(
            AnnotationKind::TypeInvariant,
            quote! {
                is_valid = "self.val > 0"
            },
        );
        let metadata = metadata_json(AnnotationKind::TypeInvariant, &sections, None);
        let payload = metadata.strip_prefix(METADATA_PREFIX).unwrap();
        let value: serde_json::Value = serde_json::from_str(payload).unwrap();

        assert_eq!(value["version"], 1);
        assert_eq!(value["kind"], "type_invariant");
        assert_eq!(value["argument_names"], serde_json::Value::Null);
        assert_eq!(value["sections"][0]["kind"], "is_valid");
        assert_eq!(value["sections"][0]["lean"], "self.val > 0");
    }

    #[test]
    fn unsafe_axiom_injects_aeneas_opacity() {
        let expanded = expand_item(
            AnnotationKind::UnsafeAxiom,
            parse(
                AnnotationKind::UnsafeAxiom,
                quote! {
                    ensures(result = "ret = 0")
                },
            ),
            quote! {
                pub unsafe fn read() -> u32 { 0 }
            },
        );
        let text = expanded.to_string();

        assert!(text.contains("aeneas :: opaque"));
        assert!(text.contains("anneal:v2:"));
        assert!(text.contains("pub unsafe fn read"));
    }

    #[test]
    fn records_free_function_argument_names() {
        let names = argument_names_for_item(
            AnnotationKind::Spec,
            quote! {
                pub unsafe fn read(ptr: *const u8, len: usize) -> u8 { 0 }
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(names, ["ptr", "len"]);
    }

    #[test]
    fn records_impl_method_receiver_argument_name() {
        let names = argument_names_for_item(
            AnnotationKind::Spec,
            quote! {
                pub fn add(&mut self, x: u32) -> u32 { self.val + x }
            },
        )
        .unwrap()
        .unwrap();
        let metadata = metadata_json(AnnotationKind::Spec, &[], Some(&names));
        let payload = metadata.strip_prefix(METADATA_PREFIX).unwrap();
        let value: serde_json::Value = serde_json::from_str(payload).unwrap();

        assert_eq!(names, ["self", "x"]);
        assert_eq!(value["argument_names"], serde_json::json!(["self", "x"]));
    }

    #[test]
    fn string_literals_must_be_direct_lean_snippets() {
        let err = parse_sections(
            AnnotationKind::Spec,
            quote! {
                ensures(result = format!("ret = {}", 0))
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("pure Lean code"));
    }
}
