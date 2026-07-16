// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use crate::annotations::schema::AnnotationTarget;

/// Errors produced while parsing Anneal attributes.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnnotationError {
    #[error("failed to parse Anneal attribute: {0}")]
    Parse(String),
    #[error("unsupported Anneal flag at `{0}`")]
    UnsupportedFlag(String),
    #[error("unsupported Anneal value: {0}")]
    UnsupportedValue(String),
    #[error("unknown Anneal path `{0}`")]
    UnknownPath(String),
    #[error("Anneal path `{0}` is not a leaf")]
    NonLeafPath(String),
    #[error("duplicate Anneal value at `{0}`")]
    Duplicate(String),
    #[error("missing required Anneal value `{0}`")]
    MissingRequired(String),
    #[error("Anneal path `{path}` is not supported on {target:?}")]
    UnsupportedTarget { path: String, target: AnnotationTarget },
}

pub type Result<T> = std::result::Result<T, AnnotationError>;
