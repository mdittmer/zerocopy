// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Rust expansion for Anneal specifications.

use anneal_annotations::{AnnotationTarget, AttributeData, Prop, RustConditionRole, RustProp};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{ToTokens as _, format_ident, quote};
use syn::spanned::Spanned as _;

/// Expands Rust-authored Anneal conditions into ordinary Rust helper functions.
///
/// A free function gets a sibling `anneal__<function>` module. Aeneas then
/// lowers these helpers alongside the user's code, allowing generated Lean
/// theorems to refer to the lowered definitions directly.
#[proc_macro_attribute]
pub fn anneal(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = TokenStream::from(attr);
    let item = TokenStream::from(item);
    expand(attr, item).unwrap_or_else(syn::Error::into_compile_error).into()
}

fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    if let Ok(mut item) = syn::parse2::<syn::ItemFn>(item.clone()) {
        return expand_free_function(attr, &mut item);
    }

    Err(syn::Error::new_spanned(item, "Anneal annotations are supported only on free functions"))
}

fn expand_free_function(attr: TokenStream, item: &mut syn::ItemFn) -> syn::Result<TokenStream> {
    let attrs = take_anneal_attrs(attr, &mut item.attrs)?;
    install_expansion_markers(&attrs, &mut item.attrs)?;
    let decoded =
        anneal_annotations::parse_attributes_for_target(&attrs, AnnotationTarget::Function)
            .map_err(|err| syn::Error::new(item.sig.ident.span(), err.to_string()))?;
    let spec = rust_spec(decoded);
    if let Some(receiver) = item.sig.receiver() {
        return Err(syn::Error::new(
            receiver.span(),
            "Anneal annotations on methods are not supported; attach the annotation to a free \
             function",
        ));
    }
    if spec.is_empty() {
        return Ok(item.to_token_stream());
    }
    if matches!(item.vis, syn::Visibility::Inherited) {
        return Err(syn::Error::new(
            item.sig.ident.span(),
            "Rust-authored Anneal conditions on private free functions, static impl methods, and \
             default trait methods are not supported yet; make a free function visible or move \
             the condition to a supported item",
        ));
    }
    validate_function_signature(&item.sig, "free functions")?;

    let module = format_ident!("anneal__{}", item.sig.ident, span = item.sig.ident.span());
    let helpers = function_helpers(&item.sig, &spec)?;
    Ok(quote! {
        #item

        #[doc(hidden)]
        #[allow(dead_code, non_snake_case, unused_imports)]
        mod #module {
            use super::*;
            #(#helpers)*
        }
    })
}

fn take_anneal_attrs(
    current: TokenStream,
    attrs: &mut Vec<syn::Attribute>,
) -> syn::Result<Vec<syn::Attribute>> {
    let current: syn::Attribute = syn::parse_quote!(#[anneal(#current)]);
    let mut anneal_attrs = vec![current];
    let mut retained = Vec::with_capacity(attrs.len());
    for attr in attrs.drain(..) {
        if is_anneal_attr(&attr) {
            anneal_attrs.push(attr);
        } else {
            retained.push(attr);
        }
    }
    *attrs = retained;
    Ok(anneal_attrs)
}

fn is_anneal_attr(attr: &syn::Attribute) -> bool {
    let segments = attr.path().segments.iter().map(|segment| &segment.ident).collect::<Vec<_>>();
    matches!(segments.as_slice(), [anneal] if *anneal == "anneal")
        || matches!(
            segments.as_slice(),
            [crate_name, anneal] if *crate_name == "anneal_macros" && *anneal == "anneal"
        )
}

fn install_expansion_markers(
    anneal_attrs: &[syn::Attribute],
    target_attrs: &mut Vec<syn::Attribute>,
) -> syn::Result<()> {
    for attr in anneal_attrs {
        let Some(marker) = anneal_annotations::expansion_marker_for_attribute(attr)
            .map_err(|err| syn::Error::new_spanned(attr, err.to_string()))?
        else {
            continue;
        };
        // HTML comments are invisible in rendered rustdoc, while Charon keeps
        // doc comments in ItemMeta.  This adds metadata, not executable helper
        // code, and preserves the original item's macro-expansion provenance.
        target_attrs.push(syn::parse_quote!(#[doc = #marker]));
    }
    Ok(())
}

#[derive(Default)]
struct RustSpec {
    requires: Vec<RustProp>,
    ensures: Vec<RustProp>,
    invariants: Vec<RustProp>,
}

impl RustSpec {
    fn is_empty(&self) -> bool {
        self.requires.is_empty() && self.ensures.is_empty() && self.invariants.is_empty()
    }
}

fn rust_spec(attrs: Vec<anneal_annotations::Attribute>) -> RustSpec {
    let mut spec = RustSpec::default();
    for attr in attrs {
        if !attr.domains.iter().any(|domain| domain == "soundness") {
            continue;
        }
        match attr.data {
            AttributeData::Spec(partial) => {
                spec.requires.extend(rust_props(partial.requires));
                spec.ensures.extend(rust_props(partial.ensures));
                spec.invariants.extend(rust_props(partial.invariants));
            }
            AttributeData::Proof(_) => {}
        }
    }
    spec
}

fn rust_props(props: Vec<Prop>) -> impl Iterator<Item = RustProp> {
    props.into_iter().filter_map(|prop| match prop {
        Prop::Rust(prop) => Some(prop),
        Prop::Lean(_) => None,
    })
}

fn validate_function_signature(sig: &syn::Signature, target: &str) -> syn::Result<()> {
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            sig.generics.span(),
            format!("Rust-authored Anneal conditions on generic {target} are not supported yet"),
        ));
    }
    if let Some(span) = impl_trait_span(sig) {
        return Err(syn::Error::new(
            span,
            format!(
                "Rust-authored Anneal conditions on {target} using `impl Trait` are not supported; \
                 use a concrete type"
            ),
        ));
    }
    if sig.asyncness.is_some()
        || sig.constness.is_some()
        || sig.abi.is_some()
        || sig.variadic.is_some()
    {
        return Err(syn::Error::new(
            sig.span(),
            format!(
                "Rust-authored Anneal conditions on async, const, extern, or variadic {target} are not supported yet"
            ),
        ));
    }
    if let Some(span) = mutable_reference_span(sig) {
        return Err(syn::Error::new(
            span,
            format!(
                "Rust-authored Anneal conditions on {target} with mutable-reference inputs or \
                 outputs are not supported yet"
            ),
        ));
    }
    Ok(())
}

fn mutable_reference_span(sig: &syn::Signature) -> Option<Span> {
    struct MutableReference(Option<Span>);

    impl<'ast> syn::visit::Visit<'ast> for MutableReference {
        fn visit_type_reference(&mut self, reference: &'ast syn::TypeReference) {
            if self.0.is_none() && reference.mutability.is_some() {
                self.0 = Some(reference.span());
            }
            syn::visit::visit_type_reference(self, reference);
        }
    }

    let mut visitor = MutableReference(None);
    syn::visit::Visit::visit_signature(&mut visitor, sig);
    visitor.0
}

fn impl_trait_span(sig: &syn::Signature) -> Option<Span> {
    struct ImplTrait(Option<Span>);

    impl<'ast> syn::visit::Visit<'ast> for ImplTrait {
        fn visit_type_impl_trait(&mut self, ty: &'ast syn::TypeImplTrait) {
            if self.0.is_none() {
                self.0 = Some(ty.span());
            }
            syn::visit::visit_type_impl_trait(self, ty);
        }
    }

    let mut visitor = ImplTrait(None);
    syn::visit::Visit::visit_signature(&mut visitor, sig);
    visitor.0
}

fn function_helpers(sig: &syn::Signature, spec: &RustSpec) -> syn::Result<Vec<TokenStream>> {
    let mut names = std::collections::BTreeSet::new();
    let mut helpers = Vec::new();
    for (index, prop) in spec.requires.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Pre, index)?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(free_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.invariants.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Invariant, index)?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(free_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.ensures.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Post, index)?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(free_helper(sig, name, prop, true)?);
    }
    Ok(helpers)
}

fn free_helper(
    sig: &syn::Signature,
    name: Ident,
    prop: &RustProp,
    binds_ret: bool,
) -> syn::Result<TokenStream> {
    let inputs = &sig.inputs;
    let ret = binds_ret.then(|| return_input(sig)).transpose()?;
    if binds_ret {
        reject_ret_binding(inputs)?;
    }
    let ret = optional_trailing_input(inputs.is_empty(), ret);
    let expr = &prop.expr;
    Ok(quote! {
        #[doc(hidden)]
        #[allow(dead_code, non_snake_case, unused_mut, unused_variables)]
        fn #name(#inputs #ret) -> bool {
            #expr
        }
    })
}

fn helper_ident(prop: &RustProp, role: RustConditionRole, index: usize) -> syn::Result<Ident> {
    let helper = prop.helper_name(role, index);
    syn::parse_str::<Ident>(&helper).map_err(|_| {
        syn::Error::new_spanned(
            &prop.expr,
            format!("Anneal condition name `{helper}` is not a valid Rust identifier"),
        )
    })
}

fn ensure_unique_helper(
    names: &mut std::collections::BTreeSet<String>,
    name: &Ident,
) -> syn::Result<()> {
    if names.insert(name.to_string()) {
        Ok(())
    } else {
        Err(syn::Error::new(name.span(), format!("duplicate generated Anneal helper `{name}`")))
    }
}

fn return_input(sig: &syn::Signature) -> syn::Result<TokenStream> {
    let ty: TokenStream = match &sig.output {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };
    Ok(quote!(ret: #ty))
}

fn optional_trailing_input(inputs_are_empty: bool, input: Option<TokenStream>) -> TokenStream {
    match (inputs_are_empty, input) {
        (_, None) => TokenStream::new(),
        (true, Some(input)) => input,
        (false, Some(input)) => quote!(, #input),
    }
}

fn reject_ret_binding(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>,
) -> syn::Result<()> {
    struct RetBinding(Option<Span>);
    impl<'ast> syn::visit::Visit<'ast> for RetBinding {
        fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
            if pat.ident == "ret" {
                self.0 = Some(pat.ident.span());
            }
            syn::visit::visit_pat_ident(self, pat);
        }
    }

    let mut visitor = RetBinding(None);
    for input in inputs {
        if let syn::FnArg::Typed(input) = input {
            syn::visit::Visit::visit_pat(&mut visitor, &input.pat);
        }
    }
    if let Some(span) = visitor.0 {
        Err(syn::Error::new(span, "`ret` is reserved for Rust-authored Anneal postconditions"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_item(attr: TokenStream, item: TokenStream) -> String {
        expand(attr, item).unwrap().to_string()
    }

    #[test]
    fn free_function_emits_named_pre_and_post_helpers() {
        let expanded = expand_item(
            quote!(spec(
                requires(name = "flag_true", prop = flag == true),
                ensures(name = "pass_thru", prop = ret == flag),
            )),
            quote!(
                pub fn needs_true(flag: bool) -> bool {
                    flag
                }
            ),
        );

        assert!(expanded.contains("mod anneal__needs_true"));
        assert!(expanded.contains("fn flag_true (flag : bool) -> bool"));
        assert!(expanded.contains("fn pass_thru (flag : bool , ret : bool) -> bool"));
        assert!(expanded.contains("flag == true"));
        assert!(expanded.contains("ret == flag"));
    }

    #[test]
    fn free_function_emits_invariant_helper() {
        let expanded = expand_item(
            quote!(spec(invariant(name = "bounded", prop = value <= 10))),
            quote!(
                pub fn bounded_identity(value: u32) -> u32 {
                    value
                }
            ),
        );

        assert!(expanded.contains("mod anneal__bounded_identity"));
        assert!(expanded.contains("fn bounded (value : u32) -> bool"));
        assert!(expanded.contains("value <= 10"));
    }

    #[test]
    fn repeated_attrs_are_consumed_once() {
        let expanded = expand_item(
            quote!(spec(requires(name = "positive", prop = x > 0))),
            quote!(
                #[anneal(spec(ensures(name = "same", prop = ret == x)))]
                pub fn identity(x: u32) -> u32 {
                    x
                }
            ),
        );

        assert!(!expanded.contains("# [anneal"));
        assert!(expanded.contains("fn positive"));
        assert!(expanded.contains("fn same"));
    }

    #[test]
    fn qualified_repeated_attrs_are_consumed_once() {
        let expanded = expand_item(
            quote!(spec(requires(name = "positive", prop = x > 0))),
            quote!(
                #[anneal_macros::anneal(spec(ensures(name = "same", prop = ret == x)))]
                pub fn identity(x: u32) -> u32 {
                    x
                }
            ),
        );

        assert!(!expanded.contains("anneal_macros"));
        assert!(expanded.contains("fn positive"));
        assert!(expanded.contains("fn same"));
    }

    #[test]
    fn non_function_items_are_rejected() {
        for item in [
            quote!(
                fn bounded(x: u32) -> u32;
            ),
            quote!(
                pub struct Counter {
                    value: u32,
                }
            ),
        ] {
            let error = expand(quote!(spec(requires(prop = true))), item).unwrap_err();
            assert!(error.to_string().contains("only on free functions"));
        }
    }

    #[test]
    fn receiver_method_has_a_context_error() {
        let error = expand(
            quote!(spec(requires(prop = self.ready))),
            quote!(
                pub fn ready(&self) -> bool {
                    true
                }
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("annotations on methods are not supported"));
    }

    #[test]
    fn private_body_item_has_a_context_error() {
        let error = expand(
            quote!(spec(requires(prop = ready))),
            quote!(
                fn ready(ready: bool) -> bool {
                    ready
                }
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("private free functions"));
    }

    #[test]
    fn rust_conditions_reject_mutable_reference_inputs_and_outputs() {
        let input_error = expand(
            quote!(spec(requires(prop = *value > 0))),
            quote!(
                pub fn read_mut(value: &mut u32) -> u32 {
                    *value
                }
            ),
        )
        .unwrap_err();
        assert!(input_error.to_string().contains("mutable-reference inputs or outputs"));

        let output_error = expand(
            quote!(spec(ensures(prop = true))),
            quote!(
                pub fn borrow_mut() -> &'static mut u32 {
                    unreachable!()
                }
            ),
        )
        .unwrap_err();
        assert!(output_error.to_string().contains("mutable-reference inputs or outputs"));
    }

    #[test]
    fn rust_conditions_reject_impl_trait_signatures() {
        let error = expand(
            quote!(spec(requires(prop = true))),
            quote!(
                pub fn inspect(value: impl Copy) -> bool {
                    let _ = value;
                    true
                }
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("using `impl Trait`"));
    }

    #[test]
    fn lean_only_specs_do_not_apply_rust_helper_signature_restrictions() {
        let expanded = expand_item(
            quote!(spec(requires = "True")),
            quote!(
                pub fn read_mut(value: &mut u32) -> u32 {
                    *value
                }
            ),
        );

        assert!(expanded.contains("pub fn read_mut"));
        assert!(!expanded.contains("mod anneal__read_mut"));
    }

    #[test]
    fn rust_helpers_are_emitted_only_for_the_soundness_domain() {
        let expanded = expand_item(
            quote!(spec(requires(name = "ready", prop = flag), domain = "documentation",)),
            quote!(
                pub fn ready(flag: bool) -> bool {
                    flag
                }
            ),
        );

        assert!(expanded.contains("pub fn ready"));
        assert!(!expanded.contains("mod anneal__ready"));
    }

    #[test]
    fn bare_theorem_is_rejected() {
        let error = expand(
            quote!(theorem),
            quote!(
                pub fn truth() -> bool {
                    true
                }
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown Anneal path `theorem`"));
    }

    #[test]
    fn structured_auxiliary_theorem_is_rejected() {
        let error = expand(
            quote!(theorem(name = "auxiliary_fact", definition(ty = "True", proof = "trivial"),)),
            quote!(
                pub fn ordinary_function(flag: bool) -> bool {
                    flag
                }
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("unknown Anneal path `theorem`"));
    }

    #[test]
    fn implicit_proof_spellings_are_rejected() {
        for proof in [
            quote!(proof),
            quote!(proof()),
            quote!(proof(lean)),
            quote!(proof(lean::tactic)),
            quote!(proof(auto)),
        ] {
            let error = expand(
                proof,
                quote!(
                    pub fn truth() -> bool {
                        true
                    }
                ),
            )
            .unwrap_err();
            assert!(error.to_string().contains("Anneal"), "unexpected error: {error}");
        }
    }

    #[test]
    fn every_expansion_marks_the_original_item_without_executable_code() {
        let expanded = expand_item(
            quote!(proof = "trivial"),
            quote!(
                pub fn truth() -> bool {
                    true
                }
            ),
        );

        assert!(expanded.contains("doc"));
        assert!(expanded.contains("anneal-expanded-v1"));
        assert!(!expanded.contains("anneal__truth"));
    }
}
