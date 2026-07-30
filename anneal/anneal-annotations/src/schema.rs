// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

/// Rust item kind used for item-aware Anneal attribute parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationTarget {
    /// Parse without item-kind restrictions.
    Any,
    /// Free function.
    Function,
    /// Struct item.
    Struct,
    /// Trait item.
    Trait,
    /// Trait associated function.
    TraitFunction,
    /// Impl block.
    Impl,
    /// Impl associated function.
    ImplFunction,
    /// Some other Rust item kind.
    Other,
}

impl AnnotationTarget {
    const fn bit(self) -> u16 {
        match self {
            Self::Any => 0,
            Self::Function => 1 << 0,
            Self::Struct => 1 << 1,
            Self::Trait => 1 << 2,
            Self::TraitFunction => 1 << 3,
            Self::Impl => 1 << 4,
            Self::ImplFunction => 1 << 5,
            Self::Other => 1 << 6,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TargetSet(u16);

impl TargetSet {
    const FUNCTION: Self = Self(AnnotationTarget::Function.bit());

    pub(super) const fn contains(self, target: AnnotationTarget) -> bool {
        matches!(target, AnnotationTarget::Any) || self.0 & target.bit() != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SchemaNode {
    pub(super) name: &'static str,
    pub(super) default_child: Option<&'static str>,
    pub(super) value: Option<ValueSpec>,
    pub(super) children: &'static [SchemaNode],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ValueSpec {
    pub(super) kind: ValueKind,
    pub(super) targets: TargetSet,
}

impl ValueSpec {
    const fn new(kind: ValueKind, targets: TargetSet) -> Self {
        Self { kind, targets }
    }

    pub(super) const fn supports(self, target: AnnotationTarget) -> bool {
        self.targets.contains(target)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ValueKind {
    Prop,
    Domain,
    Domains,
    String,
    LeanProofTactic,
    LeanProofTerm,
}

const PROP_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "name",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::String, TargetSet::FUNCTION)),
        children: &[],
    },
    SchemaNode {
        name: "prop",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION)),
        children: &[],
    },
];

const SPEC_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "requires",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION)),
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "ensures",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION)),
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "invariant",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION)),
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "domain",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domain, TargetSet::FUNCTION)),
        children: &[],
    },
    SchemaNode {
        name: "domains",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domains, TargetSet::FUNCTION)),
        children: &[],
    },
];

const PROOF_LEAN_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "tactic",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTactic, TargetSet::FUNCTION)),
        children: &[],
    },
    SchemaNode {
        name: "term",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTerm, TargetSet::FUNCTION)),
        children: &[],
    },
];

const PROOF_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "lean",
        default_child: Some("tactic"),
        value: None,
        children: PROOF_LEAN_CHILDREN,
    },
    SchemaNode {
        name: "domain",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domain, TargetSet::FUNCTION)),
        children: &[],
    },
    SchemaNode {
        name: "domains",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domains, TargetSet::FUNCTION)),
        children: &[],
    },
];

const ANNEAL_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "spec",
        default_child: Some("requires"),
        value: None,
        children: SPEC_CHILDREN,
    },
    SchemaNode {
        name: "proof",
        default_child: Some("lean"),
        value: None,
        children: PROOF_CHILDREN,
    },
];

pub(super) const ANNEAL_SCHEMA: SchemaNode =
    SchemaNode { name: "anneal", default_child: None, value: None, children: ANNEAL_CHILDREN };
