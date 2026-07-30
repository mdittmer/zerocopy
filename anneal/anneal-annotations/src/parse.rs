// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens as _;
use syn::{
    Meta, Path, Token, parenthesized,
    parse::{Parse, ParseStream, Parser as _},
};

use crate::{
    error::{AnnotationError, Result},
    schema::{ANNEAL_SCHEMA, AnnotationTarget, SchemaNode},
};

#[derive(Clone, Debug)]
pub(super) struct MatchedEntry<'a> {
    pub(super) node: &'a SchemaNode,
    pub(super) path: Vec<&'static str>,
    pub(super) payload: MatchedPayload<'a>,
}

#[derive(Clone, Debug)]
pub(super) enum MatchedPayload<'a> {
    Flag,
    Value(TokenStream),
    List(Vec<MatchedEntry<'a>>),
}

#[derive(Clone, Debug)]
struct RawEntry {
    path: Vec<String>,
    kind: RawEntryKind,
}

#[derive(Clone, Debug)]
enum RawEntryKind {
    Flag,
    Value(TokenStream),
    List(TokenStream),
}

struct RawEntryList(Vec<RawEntry>);

impl Parse for RawEntryList {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            entries.push(parse_raw_entry(input)?);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self(entries))
    }
}

fn parse_raw_entry(input: ParseStream<'_>) -> syn::Result<RawEntry> {
    let path: Path = input.parse()?;
    let path =
        path_segments(&path).map_err(|err| syn::Error::new_spanned(&path, err.to_string()))?;

    let kind = if input.peek(Token![=]) {
        input.parse::<Token![=]>()?;
        RawEntryKind::Value(parse_value_tokens(input, &path)?)
    } else if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        RawEntryKind::List(content.parse::<TokenStream>()?)
    } else {
        RawEntryKind::Flag
    };

    Ok(RawEntry { path, kind })
}

fn parse_value_tokens(input: ParseStream<'_>, path: &[String]) -> syn::Result<TokenStream> {
    if path.last().is_some_and(|segment| {
        matches!(segment.as_str(), "spec" | "requires" | "ensures" | "invariant" | "prop")
    }) {
        return input.parse::<syn::Expr>().map(|expr| expr.into_token_stream());
    }

    let mut tokens = TokenStream::new();
    while !input.is_empty() && !input.peek(Token![,]) {
        tokens.extend(std::iter::once(input.parse::<TokenTree>()?));
    }
    if tokens.is_empty() { Err(input.error("expected value after `=`")) } else { Ok(tokens) }
}

fn parse_entries(tokens: TokenStream) -> Result<Vec<RawEntry>> {
    syn::parse2::<RawEntryList>(tokens)
        .map(|entries| entries.0)
        .map_err(|err| AnnotationError::Parse(err.to_string()))
}

pub(super) fn parse_matches(
    attrs: &[syn::Attribute],
    target: AnnotationTarget,
) -> Result<Vec<MatchedEntry<'static>>> {
    let mut out = Vec::new();

    for attr in attrs {
        if is_anneal_attribute(&attr.meta)? {
            out.extend(parse_anneal_attribute(&attr.meta, target)?);
        } else if cfg_attr_wraps_anneal(&attr.meta)? {
            return Err(AnnotationError::CfgAttrWrapperUnsupported);
        }
    }

    Ok(out)
}

pub(crate) fn is_anneal_attribute(meta: &Meta) -> Result<bool> {
    let path = path_segments(meta.path())?;
    Ok(path.first().is_some_and(|segment| segment == "anneal")
        || matches!(path.as_slice(), [package, macro_name, ..]
            if package == "anneal_macros" && macro_name == "anneal"))
}

fn anneal_path_prefix_len(path: &[String]) -> Option<usize> {
    if matches!(path, [anneal] if anneal == "anneal") {
        Some(1)
    } else if matches!(path, [package, macro_name]
        if package == "anneal_macros" && macro_name == "anneal")
    {
        Some(2)
    } else {
        None
    }
}

fn cfg_attr_wraps_anneal(meta: &Meta) -> Result<bool> {
    if path_segments(meta.path())?.first().map(String::as_str) != Some("cfg_attr") {
        return Ok(false);
    }
    let Meta::List(list) = meta else {
        return Ok(false);
    };

    let parser = syn::punctuated::Punctuated::<Meta, Token![,]>::parse_terminated;
    let entries = parser
        .parse2(list.tokens.clone())
        .map_err(|err| AnnotationError::Parse(err.to_string()))?;
    for entry in entries.into_iter().skip(1) {
        if is_anneal_attribute(&entry)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_anneal_attribute(
    meta: &Meta,
    target: AnnotationTarget,
) -> Result<Vec<MatchedEntry<'static>>> {
    let path = path_segments(meta.path())?;
    if anneal_path_prefix_len(&path).is_none() {
        return Err(AnnotationError::UnknownPath(path.join("::")));
    }
    let Meta::List(list) = meta else {
        return Err(AnnotationError::NonLeafPath("anneal".into()));
    };
    let entries = parse_entries(list.tokens.clone())?;
    if entries.is_empty() {
        return Err(AnnotationError::NonLeafPath("anneal".into()));
    }
    entries
        .into_iter()
        .map(|entry| match_entry(&ANNEAL_SCHEMA, Vec::new(), entry, target))
        .collect()
}

fn match_entry<'a>(
    context: &'a SchemaNode,
    prefix: Vec<&'static str>,
    entry: RawEntry,
    target: AnnotationTarget,
) -> Result<MatchedEntry<'a>> {
    match entry.kind {
        RawEntryKind::Flag => {
            let (node, path) =
                resolve_for_capability(context, prefix, &entry.path, Capability::Flag, target)?;
            Ok(MatchedEntry { node, path, payload: MatchedPayload::Flag })
        }
        RawEntryKind::Value(tokens) => {
            let (node, path) =
                resolve_for_capability(context, prefix, &entry.path, Capability::Value, target)?;
            Ok(MatchedEntry { node, path, payload: MatchedPayload::Value(tokens) })
        }
        RawEntryKind::List(tokens) => {
            let (node, path) =
                resolve_for_capability(context, prefix, &entry.path, Capability::List, target)?;
            let children = parse_entries(tokens)?
                .into_iter()
                .map(|entry| match_entry(node, path.clone(), entry, target))
                .collect::<Result<Vec<_>>>()?;
            Ok(MatchedEntry { node, path, payload: MatchedPayload::List(children) })
        }
    }
}

#[derive(Clone, Copy)]
enum Capability {
    Flag,
    Value,
    List,
}

fn resolve_for_capability<'a>(
    context: &'a SchemaNode,
    prefix: Vec<&'static str>,
    path: &[String],
    capability: Capability,
    target: AnnotationTarget,
) -> Result<(&'a SchemaNode, Vec<&'static str>)> {
    let mut matches = Vec::new();
    for (node, matched_path) in resolve_segments(context, prefix.clone(), path) {
        finish_for_capability(node, matched_path, capability, target, &mut matches);
    }

    dedup_matches(&mut matches);
    match matches.as_slice() {
        [matched] => Ok((matched.0, matched.1.clone())),
        [] => resolve_for_capability_greedy(context, prefix, path, capability, target),
        _ => Err(AnnotationError::UnsupportedValue(format!(
            "ambiguous path `{}` matched {}",
            path_with_prefix(&prefix, path),
            matches.iter().map(|(_, path)| path_string(path)).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn resolve_segments<'a>(
    node: &'a SchemaNode,
    path: Vec<&'static str>,
    segments: &[String],
) -> Vec<(&'a SchemaNode, Vec<&'static str>)> {
    let Some((segment, rest)) = segments.split_first() else {
        return vec![(node, path)];
    };

    let mut out = Vec::new();
    if let Some(child) = child_named(node, segment) {
        let mut child_path = path.clone();
        child_path.push(child.name);
        out.extend(resolve_segments(child, child_path, rest));
    }

    if let Some(default_child) = default_child(node) {
        if default_child.name != segment.as_str() {
            let mut default_path = path;
            default_path.push(default_child.name);
            out.extend(resolve_segments(default_child, default_path, segments));
        }
    }

    out
}

fn finish_for_capability<'a>(
    node: &'a SchemaNode,
    path: Vec<&'static str>,
    capability: Capability,
    target: AnnotationTarget,
    out: &mut Vec<(&'a SchemaNode, Vec<&'static str>)>,
) {
    if supports(node, capability, target) {
        out.push((node, path));
        return;
    }

    if let Some(default_child) = default_child(node) {
        let mut default_path = path;
        default_path.push(default_child.name);
        finish_for_capability(default_child, default_path, capability, target, out);
    }
}

fn dedup_matches<'a>(matches: &mut Vec<(&'a SchemaNode, Vec<&'static str>)>) {
    let mut out: Vec<(&'a SchemaNode, Vec<&'static str>)> = Vec::new();
    for matched in matches.drain(..) {
        let key = path_string(&matched.1);
        if !out.iter().any(|(_, path)| path_string(path) == key) {
            out.push(matched);
        }
    }
    *matches = out;
}

fn resolve_for_capability_greedy<'a>(
    context: &'a SchemaNode,
    prefix: Vec<&'static str>,
    path: &[String],
    capability: Capability,
    target: AnnotationTarget,
) -> Result<(&'a SchemaNode, Vec<&'static str>)> {
    let original = path_with_prefix(&prefix, path);
    let (mut node, mut matched_path) = resolve_segments_greedy(context, prefix, path, &original)?;

    loop {
        if supports(node, capability, target) {
            return Ok((node, matched_path));
        }
        if supports_wrong_target(node, capability) {
            return Err(AnnotationError::UnsupportedTarget {
                path: path_string(&matched_path),
                target,
            });
        }

        let Some(child) = default_child(node) else {
            return Err(AnnotationError::NonLeafPath(path_string(&matched_path)));
        };
        matched_path.push(child.name);
        node = child;
    }
}

fn resolve_segments_greedy<'a>(
    mut node: &'a SchemaNode,
    mut matched_path: Vec<&'static str>,
    segments: &[String],
    original: &str,
) -> Result<(&'a SchemaNode, Vec<&'static str>)> {
    for segment in segments {
        loop {
            if let Some(child) = child_named(node, segment) {
                matched_path.push(child.name);
                node = child;
                break;
            }

            let Some(child) = default_child(node) else {
                return Err(AnnotationError::UnknownPath(original.into()));
            };
            matched_path.push(child.name);
            node = child;
        }
    }
    Ok((node, matched_path))
}

fn child_named<'a>(node: &'a SchemaNode, name: &str) -> Option<&'a SchemaNode> {
    node.children.iter().find(|child| child.name == name)
}

fn default_child(node: &SchemaNode) -> Option<&SchemaNode> {
    node.default_child.and_then(|name| child_named(node, name))
}

fn supports(node: &SchemaNode, capability: Capability, target: AnnotationTarget) -> bool {
    match capability {
        Capability::Flag => false,
        Capability::Value => node.value.is_some_and(|value| value.supports(target)),
        Capability::List => !node.children.is_empty() && node_supports_target(node, target),
    }
}

fn supports_wrong_target(node: &SchemaNode, capability: Capability) -> bool {
    match capability {
        Capability::Flag => false,
        Capability::Value => node.value.is_some(),
        Capability::List => !node.children.is_empty(),
    }
}

fn node_supports_target(node: &SchemaNode, target: AnnotationTarget) -> bool {
    node.value.is_some_and(|value| value.supports(target))
        || node.children.iter().any(|child| node_supports_target(child, target))
}

fn path_segments(path: &Path) -> Result<Vec<String>> {
    if path.leading_colon.is_some() {
        return Err(AnnotationError::Parse("absolute paths are not supported".into()));
    }

    let mut segments = Vec::new();
    for segment in &path.segments {
        if !matches!(segment.arguments, syn::PathArguments::None) {
            return Err(AnnotationError::Parse("generic path arguments are not supported".into()));
        }
        segments.push(segment.ident.to_string());
    }
    Ok(segments)
}

fn path_with_prefix(prefix: &[&'static str], path: &[String]) -> String {
    prefix.iter().copied().chain(path.iter().map(String::as_str)).collect::<Vec<_>>().join("::")
}

pub(super) fn path_string(path: &[&'static str]) -> String {
    path.join("::")
}
