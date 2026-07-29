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
/// A free function gets a sibling `anneal__<function>` module. Trait conditions
/// become default associated helper methods, and struct invariants become
/// inherent helper methods. Aeneas then lowers these helpers alongside the
/// user's code, allowing generated Lean theorems to refer to the lowered
/// definitions directly.
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
    if let Ok(mut item) = syn::parse2::<syn::ItemStruct>(item.clone()) {
        return expand_struct(attr, &mut item);
    }

    // A required trait method is the only supported annotated item whose
    // syntax ends in `;`. Methods with default bodies are syntactically
    // indistinguishable from private free functions to an attribute macro.
    if let Ok(mut item) = syn::parse2::<syn::TraitItemFn>(item.clone())
        && item.default.is_none()
    {
        return expand_trait_method(attr, &mut item);
    }

    if let Ok(mut item) = syn::parse2::<syn::ItemFn>(item.clone()) {
        return expand_free_function(attr, &mut item);
    }

    Err(syn::Error::new_spanned(
        item,
        "Rust-authored Anneal conditions are supported on free functions, required trait methods, \
         and structs; bare `theorem` markers are supported only on public free functions",
    ))
}

fn expand_free_function(attr: TokenStream, item: &mut syn::ItemFn) -> syn::Result<TokenStream> {
    let attrs = take_anneal_attrs(attr, &mut item.attrs)?;
    install_expansion_markers(&attrs, &mut item.attrs)?;
    let decoded =
        anneal_annotations::parse_attributes_for_target(&attrs, AnnotationTarget::Function)
            .map_err(|err| syn::Error::new(item.sig.ident.span(), err.to_string()))?;
    let RustSoundnessAttrs { spec, rust_theorem } = rust_soundness_attrs(decoded);
    if rust_theorem {
        validate_rust_theorem_function(item)?;
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
    if let Some(receiver) = item.sig.receiver() {
        return Err(syn::Error::new(
            receiver.span(),
            "Rust-authored Anneal conditions on impl methods and default trait methods are not \
             supported yet; an attribute macro cannot distinguish their surrounding context",
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

fn expand_trait_method(attr: TokenStream, item: &mut syn::TraitItemFn) -> syn::Result<TokenStream> {
    let attrs = take_anneal_attrs(attr, &mut item.attrs)?;
    install_expansion_markers(&attrs, &mut item.attrs)?;
    let decoded =
        anneal_annotations::parse_attributes_for_target(&attrs, AnnotationTarget::TraitFunction)
            .map_err(|err| syn::Error::new(item.sig.ident.span(), err.to_string()))?;
    let RustSoundnessAttrs { spec, rust_theorem } = rust_soundness_attrs(decoded);
    if rust_theorem {
        return Err(syn::Error::new(
            item.sig.ident.span(),
            "bare `theorem` markers are supported only on public free functions",
        ));
    }
    if spec.is_empty() {
        return Ok(item.to_token_stream());
    }
    validate_function_signature(&item.sig, "trait methods")?;

    let helpers = trait_helpers(&item.sig, &spec)?;
    Ok(quote! {
        #item
        #(#helpers)*
    })
}

fn expand_struct(attr: TokenStream, item: &mut syn::ItemStruct) -> syn::Result<TokenStream> {
    let attrs = take_anneal_attrs(attr, &mut item.attrs)?;
    install_expansion_markers(&attrs, &mut item.attrs)?;
    let decoded = anneal_annotations::parse_attributes_for_target(&attrs, AnnotationTarget::Struct)
        .map_err(|err| syn::Error::new(item.ident.span(), err.to_string()))?;
    let RustSoundnessAttrs { spec, rust_theorem } = rust_soundness_attrs(decoded);
    if rust_theorem {
        return Err(syn::Error::new(
            item.ident.span(),
            "bare `theorem` markers are supported only on public free functions",
        ));
    }
    if spec.is_empty() {
        return Ok(item.to_token_stream());
    }
    if !spec.requires.is_empty() || !spec.ensures.is_empty() {
        return Err(syn::Error::new(
            item.ident.span(),
            "struct Anneal specifications may contain only invariants",
        ));
    }
    if !item.generics.params.is_empty() || item.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            item.generics.span(),
            "Rust-authored Anneal invariants on generic structs are not supported yet",
        ));
    }

    let struct_name = &item.ident;
    let helpers = struct_helpers(&spec)?;
    Ok(quote! {
        #item

        #[doc(hidden)]
        impl #struct_name {
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

#[derive(Default)]
struct RustSoundnessAttrs {
    spec: RustSpec,
    rust_theorem: bool,
}

impl RustSpec {
    fn is_empty(&self) -> bool {
        self.requires.is_empty() && self.ensures.is_empty() && self.invariants.is_empty()
    }
}

fn rust_soundness_attrs(attrs: Vec<anneal_annotations::Attribute>) -> RustSoundnessAttrs {
    let mut out = RustSoundnessAttrs::default();
    for attr in attrs {
        if !attr.domains.iter().any(|domain| domain == "soundness") {
            continue;
        }
        match attr.data {
            AttributeData::Spec(spec) => {
                out.spec.requires.extend(rust_props(spec.requires));
                out.spec.ensures.extend(rust_props(spec.ensures));
                out.spec.invariants.extend(rust_props(spec.invariants));
            }
            AttributeData::RustTheorem => out.rust_theorem = true,
            AttributeData::Theorem(_) | AttributeData::Proof(_) => {}
        }
    }
    out
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

/// Validates theorem syntax visible to an item attribute macro.
///
/// A public static inherent method has exactly the same input token shape as a
/// public free function. The source scanner, which can see the containing impl,
/// performs the definitive context check for `cargo anneal`.
fn validate_rust_theorem_function(item: &syn::ItemFn) -> syn::Result<()> {
    let sig = &item.sig;
    if sig.receiver().is_some() {
        return Err(syn::Error::new(
            sig.ident.span(),
            "bare `theorem` markers are supported only on public free functions",
        ));
    }
    if !matches!(item.vis, syn::Visibility::Public(_)) {
        return Err(syn::Error::new(
            sig.ident.span(),
            "Rust theorem functions must use unrestricted `pub` visibility",
        ));
    }
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new(
            sig.generics.span(),
            "Rust theorem functions must be concrete and cannot have generic parameters",
        ));
    }
    if let Some(span) = impl_trait_span(sig) {
        return Err(syn::Error::new(
            span,
            "Rust theorem functions must be concrete and cannot use `impl Trait`",
        ));
    }
    if sig.asyncness.is_some()
        || sig.constness.is_some()
        || sig.abi.is_some()
        || sig.variadic.is_some()
    {
        return Err(syn::Error::new(
            sig.span(),
            "Rust theorem functions cannot be async, const, extern, or variadic",
        ));
    }
    if !returns_plain_bool(sig) {
        return Err(syn::Error::new(
            sig.output.span(),
            "Rust theorem functions must return exactly `bool`",
        ));
    }
    if let Some(span) = mutable_reference_span(sig) {
        return Err(syn::Error::new(
            span,
            "Rust theorem functions cannot take or return mutable references",
        ));
    }
    Ok(())
}

fn returns_plain_bool(sig: &syn::Signature) -> bool {
    let syn::ReturnType::Type(_, ty) = &sig.output else {
        return false;
    };
    let syn::Type::Path(path) = &**ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == "bool"
        && matches!(path.path.segments[0].arguments, syn::PathArguments::None)
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
        let name = helper_ident(prop, RustConditionRole::Pre, index, None)?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(free_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.invariants.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Invariant, index, None)?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(free_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.ensures.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Post, index, None)?;
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

fn trait_helpers(sig: &syn::Signature, spec: &RustSpec) -> syn::Result<Vec<TokenStream>> {
    let mut names = std::collections::BTreeSet::new();
    let mut helpers = Vec::new();
    for (index, prop) in spec.requires.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Pre, index, Some(&sig.ident))?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(trait_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.invariants.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Invariant, index, Some(&sig.ident))?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(trait_helper(sig, name, prop, false)?);
    }
    for (index, prop) in spec.ensures.iter().enumerate() {
        let name = helper_ident(prop, RustConditionRole::Post, index, Some(&sig.ident))?;
        ensure_unique_helper(&mut names, &name)?;
        helpers.push(trait_helper(sig, name, prop, true)?);
    }
    Ok(helpers)
}

fn trait_helper(
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

fn struct_helpers(spec: &RustSpec) -> syn::Result<Vec<TokenStream>> {
    let mut names = std::collections::BTreeSet::new();
    spec.invariants
        .iter()
        .enumerate()
        .map(|(index, prop)| {
            let suffix = helper_ident(prop, RustConditionRole::Invariant, index, None)?;
            let name = format_ident!("anneal__invariant__{}", suffix, span = suffix.span());
            ensure_unique_helper(&mut names, &name)?;
            let expr = &prop.expr;
            Ok(quote! {
                #[doc(hidden)]
                #[allow(dead_code, non_snake_case, unused_mut, unused_variables)]
                fn #name(&self) -> bool {
                    #expr
                }
            })
        })
        .collect()
}

fn helper_ident(
    prop: &RustProp,
    role: RustConditionRole,
    index: usize,
    method: Option<&Ident>,
) -> syn::Result<Ident> {
    let helper = prop.helper_name(role, index);
    let helper =
        if let Some(method) = method { format!("anneal__{method}__{helper}") } else { helper };
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
    fn trait_method_emits_default_associated_helper() {
        let expanded = expand_item(
            quote!(spec(requires(name = "bounded", prop = x <= 10))),
            quote!(
                fn bounded(x: u32) -> u32;
            ),
        );

        assert!(expanded.contains("fn anneal__bounded__bounded"));
        assert!(expanded.contains("x <= 10"));
    }

    #[test]
    fn struct_emits_inherent_invariant_helper() {
        let expanded = expand_item(
            quote!(spec(invariant(name = "bounded", prop = self.value <= 10))),
            quote!(
                pub struct Counter {
                    value: u32,
                }
            ),
        );

        assert!(expanded.contains("impl Counter"));
        assert!(expanded.contains("fn anneal__invariant__bounded (& self) -> bool"));
        assert!(expanded.contains("self . value <= 10"));
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

        assert!(error.to_string().contains("surrounding context"));
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
    fn rust_trait_conditions_reject_mutable_receivers() {
        let error = expand(
            quote!(spec(requires(prop = true))),
            quote!(
                fn update(&mut self) -> bool;
            ),
        )
        .unwrap_err();

        assert!(error.to_string().contains("mutable-reference inputs or outputs"));
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
    fn theorem_only_marker_is_validated_and_stripped() {
        let expanded = expand_item(
            quote!(theorem),
            quote!(
                pub fn truth() -> bool {
                    true
                }
            ),
        );

        assert!(expanded.contains("pub fn truth"));
        assert!(!expanded.contains("# [anneal"));
        assert!(!expanded.contains("mod anneal__truth"));
    }

    #[test]
    fn rust_theorems_require_unrestricted_public_free_functions() {
        let private = expand(
            quote!(theorem),
            quote!(
                fn private_truth() -> bool {
                    true
                }
            ),
        )
        .unwrap_err();
        assert!(private.to_string().contains("unrestricted `pub` visibility"));

        let restricted = expand(
            quote!(theorem),
            quote!(
                pub(crate) fn restricted_truth() -> bool {
                    true
                }
            ),
        )
        .unwrap_err();
        assert!(restricted.to_string().contains("unrestricted `pub` visibility"));

        let method = expand(
            quote!(theorem),
            quote!(
                pub fn method(&self) -> bool {
                    true
                }
            ),
        )
        .unwrap_err();
        assert!(method.to_string().contains("only on public free functions"));
    }

    #[test]
    fn rust_theorems_reject_non_concrete_or_non_bool_signatures() {
        let generic = expand(
            quote!(theorem),
            quote!(
                pub fn generic<T>() -> bool {
                    true
                }
            ),
        )
        .unwrap_err();
        assert!(generic.to_string().contains("cannot have generic parameters"));

        let impl_trait = expand(
            quote!(theorem),
            quote!(
                pub fn opaque(value: impl Copy) -> bool {
                    let _ = value;
                    true
                }
            ),
        )
        .unwrap_err();
        assert!(impl_trait.to_string().contains("cannot use `impl Trait`"));

        let wrong_return = expand(
            quote!(theorem),
            quote!(
                pub fn number() -> u32 {
                    0
                }
            ),
        )
        .unwrap_err();
        assert!(wrong_return.to_string().contains("return exactly `bool`"));
    }

    #[test]
    fn rust_theorems_reject_mutable_references_and_special_function_kinds() {
        let mutable = expand(
            quote!(theorem),
            quote!(
                pub fn reads_mut(value: &mut bool) -> bool {
                    *value
                }
            ),
        )
        .unwrap_err();
        assert!(mutable.to_string().contains("cannot take or return mutable references"));

        for item in [
            quote!(
                pub async fn asynchronous() -> bool {
                    true
                }
            ),
            quote!(
                pub const fn constant() -> bool {
                    true
                }
            ),
            quote!(
                pub extern "C" fn external() -> bool {
                    true
                }
            ),
        ] {
            let error = expand(quote!(theorem), item).unwrap_err();
            assert!(error.to_string().contains("cannot be async, const, extern, or variadic"));
        }
    }

    #[test]
    fn rust_theorem_marker_is_rejected_on_required_trait_methods_and_structs() {
        let trait_method = expand(
            quote!(theorem),
            quote!(
                fn truth() -> bool;
            ),
        )
        .unwrap_err();
        assert!(trait_method.to_string().contains("only on public free functions"));

        let strukt = expand(
            quote!(theorem),
            quote!(
                pub struct NotATheorem;
            ),
        )
        .unwrap_err();
        assert!(strukt.to_string().contains("only on public free functions"));
    }

    #[test]
    fn rust_theorem_marker_is_stripped_while_specs_still_expand() {
        let expanded = expand_item(
            quote!(
                theorem,
                spec(
                    requires(name = "flag_true", prop = flag),
                    ensures(name = "pass_thru", prop = ret == flag),
                ),
                proof(auto),
            ),
            quote!(
                pub fn boolean_theorem(flag: bool) -> bool {
                    let witness = flag;
                    witness
                }
            ),
        );

        assert!(!expanded.contains("# [anneal"));
        assert!(expanded.contains("let witness = flag"));
        assert!(expanded.contains("witness"));
        assert!(expanded.contains("mod anneal__boolean_theorem"));
        assert!(expanded.contains("fn flag_true"));
        assert!(expanded.contains("fn pass_thru"));
    }

    #[test]
    fn lean_theorem_declaration_is_not_a_rust_theorem_marker() {
        let expanded = expand_item(
            quote!(
                theorem(name = "auxiliary_fact", definition(ty = "True"),),
                proof(lean::tactic = "trivial"),
            ),
            quote!(
                pub fn ordinary_function(flag: bool) -> bool {
                    flag
                }
            ),
        );

        assert!(!expanded.contains("# [anneal"));
        assert!(!expanded.contains("mod anneal__ordinary_function"));
        assert!(expanded.contains("pub fn ordinary_function"));
        assert!(expanded.contains("{ flag }"));
    }

    #[test]
    fn every_expansion_marks_the_original_item_without_executable_code() {
        let expanded = expand_item(
            quote!(theorem),
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
