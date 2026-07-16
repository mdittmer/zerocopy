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
    const STRUCT: Self = Self(AnnotationTarget::Struct.bit());
    const TRAIT_FUNCTION: Self = Self(AnnotationTarget::TraitFunction.bit());
    const IMPL_FUNCTION: Self = Self(AnnotationTarget::ImplFunction.bit());
    const ALL: Self = Self(
        AnnotationTarget::Function.bit()
            | AnnotationTarget::Struct.bit()
            | AnnotationTarget::Trait.bit()
            | AnnotationTarget::TraitFunction.bit()
            | AnnotationTarget::Impl.bit()
            | AnnotationTarget::ImplFunction.bit()
            | AnnotationTarget::Other.bit(),
    );
    const FUNCTION_LIKE: Self =
        Self::FUNCTION.union(Self::TRAIT_FUNCTION).union(Self::IMPL_FUNCTION);
    const FUNCTION_OR_STRUCT: Self = Self::FUNCTION_LIKE.union(Self::STRUCT);

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(super) const fn contains(self, target: AnnotationTarget) -> bool {
        matches!(target, AnnotationTarget::Any) || self.0 & target.bit() != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SchemaNode {
    pub(super) name: &'static str,
    pub(super) default_child: Option<&'static str>,
    pub(super) value: Option<ValueSpec>,
    pub(super) flag: Option<FlagSpec>,
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

#[derive(Clone, Copy, Debug)]
pub(super) struct FlagSpec {
    pub(super) kind: FlagKind,
    pub(super) targets: TargetSet,
}

impl FlagSpec {
    const fn new(kind: FlagKind, targets: TargetSet) -> Self {
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
    LeanAttr,
    LeanAttrs,
    String,
    LeanArg,
    LeanArgs,
    LeanProofTactic,
    LeanProofTerm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FlagKind {
    LeanProofTacticSorry,
}

const PROP_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "attr",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanAttr, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "attrs",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanAttrs, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "prop",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION_OR_STRUCT)),
        flag: None,
        children: &[],
    },
];

const SPEC_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "requires",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION_LIKE)),
        flag: None,
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "ensures",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION_LIKE)),
        flag: None,
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "invariant",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Prop, TargetSet::FUNCTION_OR_STRUCT)),
        flag: None,
        children: PROP_CHILDREN,
    },
    SchemaNode {
        name: "domain",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domain, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "domains",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domains, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
];

const THEOREM_LEAN_PROOF_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "tactic",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTactic, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "term",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTerm, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
];

const THEOREM_LEAN_DEFINITION_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "arg",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanArg, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "args",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanArgs, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "ty",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::String, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "proof",
        default_child: Some("tactic"),
        value: None,
        flag: None,
        children: THEOREM_LEAN_PROOF_CHILDREN,
    },
];

const THEOREM_DEFINITION_CHILDREN: &[SchemaNode] = &[SchemaNode {
    name: "lean",
    default_child: None,
    value: None,
    flag: None,
    children: THEOREM_LEAN_DEFINITION_CHILDREN,
}];

const THEOREM_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "attr",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanAttr, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "attrs",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanAttrs, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "name",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::String, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "definition",
        default_child: Some("lean"),
        value: None,
        flag: None,
        children: THEOREM_DEFINITION_CHILDREN,
    },
    SchemaNode {
        name: "domain",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domain, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "domains",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domains, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
];

const PROOF_LEAN_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "tactic",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTactic, TargetSet::ALL)),
        flag: Some(FlagSpec::new(FlagKind::LeanProofTacticSorry, TargetSet::ALL)),
        children: &[],
    },
    SchemaNode {
        name: "term",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::LeanProofTerm, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
];

const PROOF_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "lean",
        default_child: Some("tactic"),
        value: None,
        flag: None,
        children: PROOF_LEAN_CHILDREN,
    },
    SchemaNode {
        name: "domain",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domain, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
    SchemaNode {
        name: "domains",
        default_child: None,
        value: Some(ValueSpec::new(ValueKind::Domains, TargetSet::ALL)),
        flag: None,
        children: &[],
    },
];

const ANNEAL_CHILDREN: &[SchemaNode] = &[
    SchemaNode {
        name: "spec",
        default_child: Some("requires"),
        value: None,
        flag: None,
        children: SPEC_CHILDREN,
    },
    SchemaNode {
        name: "theorem",
        default_child: None,
        value: None,
        flag: None,
        children: THEOREM_CHILDREN,
    },
    SchemaNode {
        name: "proof",
        default_child: Some("lean"),
        value: None,
        flag: None,
        children: PROOF_CHILDREN,
    },
];

pub(super) const ANNEAL_SCHEMA: SchemaNode = SchemaNode {
    name: "anneal",
    default_child: None,
    value: None,
    flag: None,
    children: ANNEAL_CHILDREN,
};
