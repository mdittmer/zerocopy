// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use proc_macro2::TokenStream;
use quote::ToTokens as _;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, ExprLit, Lit, LitStr, Token, bracketed};

use crate::error::{AnnotationError, Result};
use crate::parse::{MatchedEntry, MatchedPayload, path_string};
use crate::schema::{AnnotationTarget, FlagKind, ValueKind};
use crate::{
    Attribute, AttributeData, Attributes, LeanArg, LeanArgs, LeanAttr, LeanAttrs, LeanProof,
    LeanProp, LeanTheoremDefinition, LeanType, Proof, Prop, RustProp, Spec, Theorem,
    TheoremDefinition,
};

pub(crate) fn parse_attributes(
    attrs: &[syn::Attribute],
    target: AnnotationTarget,
) -> Result<Attributes> {
    crate::parse::parse_matches(attrs, target)?.into_iter().map(attribute_from_match).collect()
}

fn attribute_from_match(matched: MatchedEntry<'_>) -> Result<Attribute> {
    let Some(kind) = matched.path.first().copied() else {
        return Err(AnnotationError::UnknownPath(String::new()));
    };

    if matches!(matched.path.as_slice(), ["theorem"])
        && matches!(&matched.payload, MatchedPayload::Flag)
    {
        return Ok(Attribute {
            data: AttributeData::RustTheorem,
            domains: Attribute::default_domains(),
        });
    }

    let mut builder = match kind {
        "spec" => AttributeBuilder::Spec { spec: Spec::default(), domains: Domains::default() },
        "theorem" => AttributeBuilder::Theorem {
            theorem: TheoremBuilder::default(),
            domains: Domains::default(),
        },
        "proof" => AttributeBuilder::Proof { proof: None, domains: Domains::default() },
        _ => return Err(AnnotationError::UnknownPath(kind.into())),
    };

    apply_match(&mut builder, matched)?;
    builder.finish()
}

fn apply_match(builder: &mut AttributeBuilder, matched: MatchedEntry<'_>) -> Result<()> {
    match matched.payload {
        MatchedPayload::Flag => {
            apply_flag(builder, &matched.path, matched.node.flag.map(|f| f.kind))
        }
        MatchedPayload::Value(tokens) => {
            apply_value(builder, &matched.path, matched.node.value.map(|v| v.kind), tokens)
        }
        MatchedPayload::List(children) => {
            if matches!(matched.path.as_slice(), ["spec"]) {
                return apply_spec_list(builder, children);
            }
            if is_spec_prop_path(&matched.path) {
                return push_spec_prop(
                    builder,
                    &matched.path,
                    prop_from_children(&matched.path, children)?,
                );
            }

            for child in children {
                apply_match(builder, child)?;
            }
            Ok(())
        }
    }
}

#[derive(Default)]
struct PropParts {
    attrs: Option<LeanAttrs>,
    name: Option<String>,
    prop: Option<Prop>,
}

impl PropParts {
    fn is_empty(&self) -> bool {
        self.attrs.is_none() && self.name.is_none() && self.prop.is_none()
    }
}

fn apply_spec_list(builder: &mut AttributeBuilder, children: Vec<MatchedEntry<'_>>) -> Result<()> {
    let mut pending: std::collections::BTreeMap<&'static str, PropParts> =
        std::collections::BTreeMap::new();

    for child in children {
        if is_spec_prop_child_path(&child.path) {
            apply_spec_prop_child(builder, child, &mut pending)?;
        } else if is_spec_prop_path(&child.path)
            && matches!(&child.payload, MatchedPayload::Value(_))
        {
            apply_spec_prop_value(builder, child, &mut pending)?;
        } else {
            flush_pending_props(builder, &mut pending)?;
            apply_match(builder, child)?;
        }
    }

    flush_pending_props(builder, &mut pending)
}

fn apply_spec_prop_child(
    builder: &mut AttributeBuilder,
    child: MatchedEntry<'_>,
    pending: &mut std::collections::BTreeMap<&'static str, PropParts>,
) -> Result<()> {
    let [_, field, leaf] = child.path.as_slice() else {
        return Err(AnnotationError::UnknownPath(path_string(&child.path)));
    };
    if matches!(leaf, &"prop") && pending.get(*field).is_some_and(|parts| parts.prop.is_some()) {
        flush_pending_prop(builder, pending, *field)?;
    }

    let parts = pending.entry(*field).or_default();
    match (*leaf, child.node.value.map(|v| v.kind), child.payload) {
        ("name", Some(ValueKind::String), MatchedPayload::Value(tokens)) => {
            set_once(&mut parts.name, string_value(tokens)?, &child.path)
        }
        (
            "attr" | "attrs",
            Some(kind @ (ValueKind::LeanAttr | ValueKind::LeanAttrs)),
            MatchedPayload::Value(tokens),
        ) => merge_lean_attrs(&mut parts.attrs, lean_attrs_fragment(kind, tokens)?, &child.path),
        ("prop", Some(ValueKind::Prop), MatchedPayload::Value(tokens)) => {
            set_once(&mut parts.prop, prop_scalar_value(tokens)?, &child.path)
        }
        (_, _, MatchedPayload::Flag) => {
            Err(AnnotationError::UnsupportedFlag(path_string(&child.path)))
        }
        (_, _, MatchedPayload::List(_)) => {
            Err(AnnotationError::NonLeafPath(path_string(&child.path)))
        }
        _ => Err(AnnotationError::UnknownPath(path_string(&child.path))),
    }
}

fn apply_spec_prop_value(
    builder: &mut AttributeBuilder,
    child: MatchedEntry<'_>,
    pending: &mut std::collections::BTreeMap<&'static str, PropParts>,
) -> Result<()> {
    let [_, field] = child.path.as_slice() else {
        return Err(AnnotationError::UnknownPath(path_string(&child.path)));
    };
    if !pending.get(*field).is_some_and(|parts| !parts.is_empty()) {
        return apply_match(builder, child);
    }
    if pending.get(*field).is_some_and(|parts| parts.prop.is_some()) {
        flush_pending_prop(builder, pending, *field)?;
    }

    let MatchedPayload::Value(tokens) = child.payload else {
        return Err(AnnotationError::UnknownPath(path_string(&child.path)));
    };
    let parts = pending.entry(*field).or_default();
    set_once(&mut parts.prop, prop_scalar_value(tokens)?, &child.path)?;
    flush_pending_prop(builder, pending, *field)
}

fn flush_pending_props(
    builder: &mut AttributeBuilder,
    pending: &mut std::collections::BTreeMap<&'static str, PropParts>,
) -> Result<()> {
    let fields = pending.keys().copied().collect::<Vec<_>>();
    for field in fields {
        flush_pending_prop(builder, pending, field)?;
    }
    Ok(())
}

fn flush_pending_prop(
    builder: &mut AttributeBuilder,
    pending: &mut std::collections::BTreeMap<&'static str, PropParts>,
    field: &'static str,
) -> Result<()> {
    let Some(parts) = pending.remove(field) else {
        return Ok(());
    };
    if parts.is_empty() {
        return Ok(());
    }

    let prop_path = format!("spec::{field}::prop");
    let mut prop = required(parts.prop, &prop_path)?;
    set_prop_name(&mut prop, parts.name);
    if let Some(attrs) = parts.attrs {
        match &mut prop {
            Prop::Lean(lean) => lean.attrs = attrs,
            Prop::Rust(_) => {
                return Err(AnnotationError::UnsupportedValue(format!(
                    "Lean attributes are not supported on Rust proposition `{prop_path}`"
                )));
            }
        }
    }
    push_spec_prop(builder, &["spec", field], prop)
}

struct Domains {
    values: Vec<String>,
    explicit: bool,
}

impl Default for Domains {
    fn default() -> Self {
        Self { values: Attribute::default_domains(), explicit: false }
    }
}

impl Domains {
    fn push(&mut self, domain: String) {
        if !self.explicit {
            self.values.clear();
            self.explicit = true;
        }
        self.values.push(domain);
    }

    fn extend(&mut self, domains: impl IntoIterator<Item = String>) {
        for domain in domains {
            self.push(domain);
        }
    }
}

enum AttributeBuilder {
    Spec { spec: Spec, domains: Domains },
    Theorem { theorem: TheoremBuilder, domains: Domains },
    Proof { proof: Option<Proof>, domains: Domains },
}

impl AttributeBuilder {
    fn finish(self) -> Result<Attribute> {
        match self {
            AttributeBuilder::Spec { spec, domains } => {
                Ok(Attribute { data: AttributeData::Spec(spec), domains: domains.values })
            }
            AttributeBuilder::Theorem { theorem, domains } => Ok(Attribute {
                data: AttributeData::Theorem(theorem.finish()?),
                domains: domains.values,
            }),
            AttributeBuilder::Proof { proof, domains } => Ok(Attribute {
                data: AttributeData::Proof(
                    proof.unwrap_or_else(|| Proof::Lean(LeanProof::TacticProof("sorry".into()))),
                ),
                domains: domains.values,
            }),
        }
    }
}

#[derive(Default)]
struct TheoremBuilder {
    attrs: Option<LeanAttrs>,
    name: Option<String>,
    definition: LeanTheoremDefinitionBuilder,
}

impl TheoremBuilder {
    fn finish(self) -> Result<Theorem> {
        Ok(Theorem {
            attrs: self.attrs.unwrap_or_else(LeanAttrs::empty),
            name: required(self.name, "theorem::name")?,
            definition: TheoremDefinition::Lean(self.definition.finish()?),
        })
    }
}

#[derive(Default)]
struct LeanTheoremDefinitionBuilder {
    args: Vec<LeanArgs>,
    ty: Option<LeanType>,
    proof: Option<LeanProof>,
}

impl LeanTheoremDefinitionBuilder {
    fn finish(self) -> Result<LeanTheoremDefinition> {
        Ok(LeanTheoremDefinition {
            args: self.args,
            ty: required(self.ty, "theorem::definition::lean::ty")?,
            proof: self.proof.unwrap_or_else(|| LeanProof::TacticProof("sorry".into())),
        })
    }
}

fn required<T>(value: Option<T>, path: &str) -> Result<T> {
    value.ok_or_else(|| AnnotationError::MissingRequired(path.into()))
}

fn apply_flag(
    builder: &mut AttributeBuilder,
    path: &[&'static str],
    flag: Option<FlagKind>,
) -> Result<()> {
    match (path, flag) {
        ([theorem], Some(FlagKind::RustTheorem)) if *theorem == "theorem" => Ok(()),
        ([proof, auto], Some(FlagKind::AutoProof)) if *proof == "proof" && *auto == "auto" => {
            set_proof(builder, Proof::Auto, path)
        }
        ([proof, lean, tactic], Some(FlagKind::LeanProofTacticSorry))
            if *proof == "proof" && *lean == "lean" && *tactic == "tactic" =>
        {
            set_proof(builder, Proof::Lean(LeanProof::TacticProof("sorry".into())), path)
        }
        _ => Err(AnnotationError::UnsupportedFlag(path_string(path))),
    }
}

fn apply_value(
    builder: &mut AttributeBuilder,
    path: &[&'static str],
    kind: Option<ValueKind>,
    tokens: TokenStream,
) -> Result<()> {
    match (path, kind) {
        ([spec, field], Some(ValueKind::Prop))
            if *spec == "spec" && matches!(*field, "requires" | "ensures" | "invariant") =>
        {
            push_spec_prop(builder, path, prop_scalar_value(tokens)?)
        }
        (_, Some(kind @ (ValueKind::Domain | ValueKind::Domains))) => {
            extend_domains(builder, domain_values(kind, tokens)?)
        }
        ([theorem, _], Some(kind @ (ValueKind::LeanAttr | ValueKind::LeanAttrs)))
            if *theorem == "theorem" =>
        {
            merge_theorem_attrs(builder, lean_attrs_fragment(kind, tokens)?, path)
        }
        ([theorem, field], Some(ValueKind::String))
            if *theorem == "theorem" && *field == "name" =>
        {
            set_once(&mut theorem_builder(builder)?.name, string_value(tokens)?, path)
        }
        (
            [theorem, definition, lean, _],
            Some(kind @ (ValueKind::LeanArg | ValueKind::LeanArgs)),
        ) if *theorem == "theorem" && *definition == "definition" && *lean == "lean" => {
            push_theorem_args(builder, lean_args_fragment(kind, tokens)?)
        }
        ([theorem, definition, lean, field], Some(ValueKind::String))
            if *theorem == "theorem"
                && *definition == "definition"
                && *lean == "lean"
                && *field == "ty" =>
        {
            set_once(
                &mut theorem_builder(builder)?.definition.ty,
                LeanType(string_value(tokens)?),
                path,
            )
        }
        ([theorem, definition, lean, proof, mode], Some(ValueKind::LeanProofTactic))
            if *theorem == "theorem"
                && *definition == "definition"
                && *lean == "lean"
                && *proof == "proof"
                && *mode == "tactic" =>
        {
            set_once(
                &mut theorem_builder(builder)?.definition.proof,
                LeanProof::TacticProof(string_value(tokens)?),
                path,
            )
        }
        ([theorem, definition, lean, proof, mode], Some(ValueKind::LeanProofTerm))
            if *theorem == "theorem"
                && *definition == "definition"
                && *lean == "lean"
                && *proof == "proof"
                && *mode == "term" =>
        {
            set_once(
                &mut theorem_builder(builder)?.definition.proof,
                LeanProof::TermProof(string_value(tokens)?),
                path,
            )
        }
        ([proof, lean, mode], Some(ValueKind::LeanProofTactic))
            if *proof == "proof" && *lean == "lean" && *mode == "tactic" =>
        {
            set_proof(builder, Proof::Lean(LeanProof::TacticProof(string_value(tokens)?)), path)
        }
        ([proof, lean, mode], Some(ValueKind::LeanProofTerm))
            if *proof == "proof" && *lean == "lean" && *mode == "term" =>
        {
            set_proof(builder, Proof::Lean(LeanProof::TermProof(string_value(tokens)?)), path)
        }
        _ => Err(AnnotationError::UnknownPath(path_string(path))),
    }
}

fn spec_builder(builder: &mut AttributeBuilder) -> Result<&mut Spec> {
    match builder {
        AttributeBuilder::Spec { spec, .. } => Ok(spec),
        _ => Err(AnnotationError::UnknownPath("spec".into())),
    }
}

fn theorem_builder(builder: &mut AttributeBuilder) -> Result<&mut TheoremBuilder> {
    match builder {
        AttributeBuilder::Theorem { theorem, .. } => Ok(theorem),
        _ => Err(AnnotationError::UnknownPath("theorem".into())),
    }
}

fn domains_builder(builder: &mut AttributeBuilder) -> &mut Domains {
    match builder {
        AttributeBuilder::Spec { domains, .. }
        | AttributeBuilder::Theorem { domains, .. }
        | AttributeBuilder::Proof { domains, .. } => domains,
    }
}

fn extend_domains(builder: &mut AttributeBuilder, domains: Vec<String>) -> Result<()> {
    domains_builder(builder).extend(domains);
    Ok(())
}

fn push_theorem_args(builder: &mut AttributeBuilder, args: Vec<LeanArgs>) -> Result<()> {
    theorem_builder(builder)?.definition.args.extend(args);
    Ok(())
}

fn merge_theorem_attrs(
    builder: &mut AttributeBuilder,
    attrs: LeanAttrs,
    path: &[&'static str],
) -> Result<()> {
    merge_lean_attrs(&mut theorem_builder(builder)?.attrs, attrs, path)
}

fn set_proof(builder: &mut AttributeBuilder, value: Proof, path: &[&'static str]) -> Result<()> {
    match builder {
        AttributeBuilder::Proof { proof, .. } => set_once(proof, value, path),
        _ => Err(AnnotationError::UnknownPath(path_string(path))),
    }
}

fn is_spec_prop_path(path: &[&'static str]) -> bool {
    matches!(
        path,
        [spec, field]
            if *spec == "spec" && matches!(*field, "requires" | "ensures" | "invariant")
    )
}

fn is_spec_prop_child_path(path: &[&'static str]) -> bool {
    matches!(
        path,
        [spec, field, leaf]
            if *spec == "spec"
                && matches!(*field, "requires" | "ensures" | "invariant")
                && matches!(*leaf, "name" | "attr" | "attrs" | "prop")
    )
}

fn push_spec_prop(builder: &mut AttributeBuilder, path: &[&'static str], prop: Prop) -> Result<()> {
    let [spec, field] = path else {
        return Err(AnnotationError::UnknownPath(path_string(path)));
    };
    if *spec != "spec" {
        return Err(AnnotationError::UnknownPath(path_string(path)));
    }

    match *field {
        "requires" => spec_builder(builder)?.requires.push(prop),
        "ensures" => spec_builder(builder)?.ensures.push(prop),
        "invariant" => spec_builder(builder)?.invariants.push(prop),
        _ => return Err(AnnotationError::UnknownPath(path_string(path))),
    }
    Ok(())
}

fn prop_from_children(path: &[&'static str], children: Vec<MatchedEntry<'_>>) -> Result<Prop> {
    let mut attrs = None;
    let mut name = None;
    let mut prop = None;

    for child in children {
        match (child.path.as_slice(), child.node.value.map(|v| v.kind), child.payload) {
            ([.., "name"], Some(ValueKind::String), MatchedPayload::Value(tokens)) => {
                set_once(&mut name, string_value(tokens)?, &child.path)?;
            }
            (
                _,
                Some(kind @ (ValueKind::LeanAttr | ValueKind::LeanAttrs)),
                MatchedPayload::Value(tokens),
            ) => {
                merge_lean_attrs(&mut attrs, lean_attrs_fragment(kind, tokens)?, &child.path)?;
            }
            ([.., "prop"], Some(ValueKind::Prop), MatchedPayload::Value(tokens)) => {
                set_once(&mut prop, prop_scalar_value(tokens)?, &child.path)?;
            }
            (_, _, MatchedPayload::Flag) => {
                return Err(AnnotationError::UnsupportedFlag(path_string(&child.path)));
            }
            (_, _, MatchedPayload::List(_)) => {
                return Err(AnnotationError::NonLeafPath(path_string(&child.path)));
            }
            _ => return Err(AnnotationError::UnknownPath(path_string(&child.path))),
        }
    }

    let prop_path = format!("{}::prop", path_string(path));
    let mut prop = required(prop, &prop_path)?;
    set_prop_name(&mut prop, name);
    if let Some(attrs) = attrs {
        match &mut prop {
            Prop::Lean(lean) => lean.attrs = attrs,
            Prop::Rust(_) => {
                return Err(AnnotationError::UnsupportedValue(format!(
                    "Lean attributes are not supported on Rust proposition `{prop_path}`"
                )));
            }
        }
    }
    Ok(prop)
}

fn set_prop_name(prop: &mut Prop, name: Option<String>) {
    match prop {
        Prop::Rust(rust) => rust.name = name,
        Prop::Lean(lean) => lean.name = name,
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T, path: &[&'static str]) -> Result<()> {
    if slot.replace(value).is_some() {
        Err(AnnotationError::Duplicate(path_string(path)))
    } else {
        Ok(())
    }
}

fn prop_scalar_value(tokens: TokenStream) -> Result<Prop> {
    let expr = expr_value(tokens.clone())?;
    match &expr {
        Expr::Lit(ExprLit { lit: Lit::Str(value), .. }) => {
            Ok(Prop::Lean(LeanProp::from(value.value())))
        }
        _ => Ok(Prop::Rust(RustProp::new(tokens))),
    }
}

fn domain_values(kind: ValueKind, tokens: TokenStream) -> Result<Vec<String>> {
    match kind {
        ValueKind::Domain => string_value(tokens).map(|domain| vec![domain]),
        ValueKind::Domains => string_values(tokens),
        _ => Err(AnnotationError::UnsupportedValue(format!(
            "unsupported domain value kind: {kind:?}"
        ))),
    }
}

fn lean_attrs_fragment(kind: ValueKind, tokens: TokenStream) -> Result<LeanAttrs> {
    match kind {
        ValueKind::LeanAttr => Ok(LeanAttrs::Attrs(vec![lean_attr_value(tokens)?])),
        ValueKind::LeanAttrs => lean_attrs_value(tokens),
        _ => Err(AnnotationError::UnsupportedValue(format!(
            "unsupported Lean attrs value kind: {kind:?}"
        ))),
    }
}

fn lean_attrs_value(tokens: TokenStream) -> Result<LeanAttrs> {
    let expr = expr_value(tokens)?;
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Str(value), .. }) => {
            Ok(LeanAttrs::AttrsString(value.value()))
        }
        Expr::Array(array) => Ok(LeanAttrs::Attrs(
            array
                .elems
                .iter()
                .map(string_value_expr)
                .map(|value| value.map(LeanAttr::from))
                .collect::<Result<Vec<_>>>()?,
        )),
        expr => Err(AnnotationError::UnsupportedValue(quote_expr(&expr))),
    }
}

fn lean_attr_value(tokens: TokenStream) -> Result<LeanAttr> {
    string_value(tokens).map(LeanAttr::from)
}

fn merge_lean_attrs(
    slot: &mut Option<LeanAttrs>,
    value: LeanAttrs,
    path: &[&'static str],
) -> Result<()> {
    match (slot.as_mut(), value) {
        (None, value) => {
            *slot = Some(value);
            Ok(())
        }
        (Some(LeanAttrs::Attrs(existing)), LeanAttrs::Attrs(mut incoming)) => {
            existing.append(&mut incoming);
            Ok(())
        }
        (Some(_), _) => Err(AnnotationError::Duplicate(path_string(path))),
    }
}

fn string_value(tokens: TokenStream) -> Result<String> {
    let expr = expr_value(tokens)?;
    string_value_expr(&expr)
}

fn string_values(tokens: TokenStream) -> Result<Vec<String>> {
    let expr = expr_value(tokens)?;
    match expr {
        Expr::Array(array) => array.elems.iter().map(string_value_expr).collect::<Result<Vec<_>>>(),
        expr => string_value_expr(&expr).map(|value| vec![value]),
    }
}

fn string_value_expr(expr: &Expr) -> Result<String> {
    let Expr::Lit(ExprLit { lit: Lit::Str(value), .. }) = expr else {
        return Err(AnnotationError::UnsupportedValue(quote_expr(expr)));
    };
    Ok(value.value())
}

fn expr_value(tokens: TokenStream) -> Result<Expr> {
    syn::parse2::<Expr>(tokens).map_err(|err| AnnotationError::Parse(err.to_string()))
}

fn lean_args_fragment(kind: ValueKind, tokens: TokenStream) -> Result<Vec<LeanArgs>> {
    match kind {
        ValueKind::LeanArg => lean_arg_value(tokens).map(|arg| vec![arg]),
        ValueKind::LeanArgs => lean_args_value(tokens),
        _ => Err(AnnotationError::UnsupportedValue(format!(
            "unsupported Lean args value kind: {kind:?}"
        ))),
    }
}

fn lean_args_value(tokens: TokenStream) -> Result<Vec<LeanArgs>> {
    if let Ok(value) = string_value(tokens.clone()) {
        return Ok(vec![LeanArgs::ArgsString(value)]);
    }

    syn::parse2::<LeanArgsValue>(tokens)
        .map(|value| value.0)
        .map_err(|err| AnnotationError::Parse(err.to_string()))
}

fn lean_arg_value(tokens: TokenStream) -> Result<LeanArgs> {
    if let Ok(value) = string_value(tokens.clone()) {
        return Ok(LeanArgs::ArgsString(value));
    }

    syn::parse2::<LeanArgValue>(tokens)
        .map(|value| LeanArgs::Args(value.0))
        .map_err(|err| AnnotationError::Parse(err.to_string()))
}

struct LeanArgValue(Vec<LeanArg>);

impl Parse for LeanArgValue {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        bracketed!(content in input);
        Ok(Self(parse_lean_arg_group(&content)?))
    }
}

struct LeanArgsValue(Vec<LeanArgs>);

impl Parse for LeanArgsValue {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let content;
        bracketed!(content in input);

        let mut args = Vec::new();
        if content.peek(syn::token::Bracket) {
            while !content.is_empty() {
                args.push(LeanArgs::Args(parse_bracketed_lean_arg_group(&content)?));
                if content.is_empty() {
                    break;
                }
                content.parse::<Token![,]>()?;
            }
        } else {
            args.push(LeanArgs::Args(parse_lean_arg_group(&content)?));
        }

        Ok(Self(args))
    }
}

fn parse_bracketed_lean_arg_group(input: ParseStream<'_>) -> syn::Result<Vec<LeanArg>> {
    let content;
    bracketed!(content in input);
    parse_lean_arg_group(&content)
}

fn parse_lean_arg_group(input: ParseStream<'_>) -> syn::Result<Vec<LeanArg>> {
    let mut args = Vec::new();
    while !input.is_empty() {
        let name: LitStr = input.parse()?;
        if input.peek(Token![:]) {
            input.parse::<Token![:]>()?;
            let ty: LitStr = input.parse()?;
            args.push(LeanArg::Arg { name: name.value(), ty: LeanType(ty.value()) });
        } else {
            args.push(LeanArg::ArgString(name.value()));
        }

        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(args)
}

fn quote_expr(expr: &Expr) -> String {
    match expr {
        Expr::Lit(ExprLit { lit, .. }) => match lit {
            Lit::Str(value) => value.value(),
            Lit::Int(value) => value.base10_digits().into(),
            Lit::Bool(value) => value.value.to_string(),
            _ => "unsupported literal".into(),
        },
        expr => expr.to_token_stream().to_string(),
    }
}
