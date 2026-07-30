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
use syn::{Expr, ExprLit, Lit};

use crate::annotations::{
    Attribute, AttributeData, Attributes, LeanProof, LeanProp, Proof, Prop, Spec,
    error::{AnnotationError, Result},
    parse::{MatchedEntry, MatchedPayload, path_string},
    schema::{AnnotationTarget, ValueKind},
};

pub(crate) fn parse_attributes(
    attrs: &[syn::Attribute],
    target: AnnotationTarget,
) -> Result<Attributes> {
    crate::annotations::parse::parse_matches(attrs, target)?
        .into_iter()
        .map(attribute_from_match)
        .collect()
}

fn attribute_from_match(matched: MatchedEntry<'_>) -> Result<Attribute> {
    let Some(kind) = matched.path.first().copied() else {
        return Err(AnnotationError::UnknownPath(String::new()));
    };

    let mut builder = match kind {
        "spec" => AttributeBuilder::Spec { spec: Spec::default(), domains: Domains::default() },
        "proof" => AttributeBuilder::Proof { proof: None, domains: Domains::default() },
        _ => return Err(AnnotationError::UnknownPath(kind.into())),
    };

    apply_match(&mut builder, matched)?;
    builder.finish()
}

fn apply_match(builder: &mut AttributeBuilder, matched: MatchedEntry<'_>) -> Result<()> {
    match matched.payload {
        MatchedPayload::Value(tokens) => {
            apply_value(builder, &matched.path, matched.node.value.map(|value| value.kind), tokens)
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
    name: Option<String>,
    prop: Option<Prop>,
}

impl PropParts {
    fn is_empty(&self) -> bool {
        self.name.is_none() && self.prop.is_none()
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
    match (*leaf, child.node.value.map(|value| value.kind), child.payload) {
        ("name", Some(ValueKind::String), MatchedPayload::Value(tokens)) => {
            set_once(&mut parts.name, string_value(tokens)?, &child.path)
        }
        ("prop", Some(ValueKind::Prop), MatchedPayload::Value(tokens)) => {
            set_once(&mut parts.prop, prop_scalar_value(tokens)?, &child.path)
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
    let Prop::Lean(mut lean) = required(parts.prop, &prop_path)?;
    if let Some(name) = parts.name {
        lean.name = Some(name);
    }
    push_spec_prop(builder, &["spec", field], Prop::Lean(lean))
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
    fn make_explicit(&mut self) {
        if !self.explicit {
            self.values.clear();
            self.explicit = true;
        }
    }

    fn push(&mut self, domain: String) {
        self.make_explicit();
        self.values.push(domain);
    }

    fn extend(&mut self, domains: impl IntoIterator<Item = String>) {
        self.make_explicit();
        self.values.extend(domains);
    }
}

enum AttributeBuilder {
    Spec { spec: Spec, domains: Domains },
    Proof { proof: Option<LeanProof>, domains: Domains },
}

impl AttributeBuilder {
    fn finish(self) -> Result<Attribute> {
        match self {
            AttributeBuilder::Spec { spec, domains } => {
                Ok(Attribute { data: AttributeData::Spec(spec), domains: domains.values })
            }
            AttributeBuilder::Proof { proof, domains } => Ok(Attribute {
                data: AttributeData::Proof(Proof::Lean(required(
                    proof,
                    "proof::lean::tactic or proof::lean::term",
                )?)),
                domains: domains.values,
            }),
        }
    }
}

fn required<T>(value: Option<T>, path: &str) -> Result<T> {
    value.ok_or_else(|| AnnotationError::MissingRequired(path.into()))
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
            domains_builder(builder).extend(domain_values(kind, tokens)?);
            Ok(())
        }
        ([proof, lean, mode], Some(ValueKind::LeanProofTactic))
            if *proof == "proof" && *lean == "lean" && *mode == "tactic" =>
        {
            set_proof(builder, LeanProof::TacticProof(string_value(tokens)?), path)
        }
        ([proof, lean, mode], Some(ValueKind::LeanProofTerm))
            if *proof == "proof" && *lean == "lean" && *mode == "term" =>
        {
            set_proof(builder, LeanProof::TermProof(string_value(tokens)?), path)
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

fn domains_builder(builder: &mut AttributeBuilder) -> &mut Domains {
    match builder {
        AttributeBuilder::Spec { domains, .. } | AttributeBuilder::Proof { domains, .. } => domains,
    }
}

fn set_proof(
    builder: &mut AttributeBuilder,
    value: LeanProof,
    path: &[&'static str],
) -> Result<()> {
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
                && matches!(*leaf, "name" | "prop")
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
    let mut name = None;
    let mut prop = None;

    for child in children {
        match (child.path.as_slice(), child.node.value.map(|value| value.kind), child.payload) {
            ([.., "name"], Some(ValueKind::String), MatchedPayload::Value(tokens)) => {
                set_once(&mut name, string_value(tokens)?, &child.path)?;
            }
            ([.., "prop"], Some(ValueKind::Prop), MatchedPayload::Value(tokens)) => {
                set_once(&mut prop, prop_scalar_value(tokens)?, &child.path)?;
            }
            (_, _, MatchedPayload::List(_)) => {
                return Err(AnnotationError::NonLeafPath(path_string(&child.path)));
            }
            _ => return Err(AnnotationError::UnknownPath(path_string(&child.path))),
        }
    }

    let prop_path = format!("{}::prop", path_string(path));
    let Prop::Lean(mut lean) = required(prop, &prop_path)?;
    if let Some(name) = name {
        lean.name = Some(name);
    }
    Ok(Prop::Lean(lean))
}

fn set_once<T>(slot: &mut Option<T>, value: T, path: &[&'static str]) -> Result<()> {
    if slot.replace(value).is_some() {
        Err(AnnotationError::Duplicate(path_string(path)))
    } else {
        Ok(())
    }
}

fn prop_scalar_value(tokens: TokenStream) -> Result<Prop> {
    let expr = expr_value(tokens)?;
    let Expr::Lit(ExprLit { lit: Lit::Str(value), .. }) = &expr else {
        return Err(AnnotationError::UnsupportedValue(format!(
            "Rust-language propositions are not implemented yet; write Lean propositions as string literals, got `{}`",
            quote_expr(&expr)
        )));
    };
    Ok(Prop::Lean(LeanProp::from(value.value())))
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
