// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

#![deny(warnings)]

use anneal_macros::anneal;

#[anneal(spec(
    requires(name = "flag_true", prop = flag == true),
    ensures(name = "pass_thru", prop = ret == flag),
))]
pub fn needs_true(flag: bool) -> bool {
    flag
}

#[anneal(
    spec(
        requires(name = "flag_true", prop = flag),
        ensures(name = "pass_thru", prop = ret == flag),
    )
)]
pub fn boolean_spec(flag: bool) -> bool {
    flag
}

#[anneal(spec(ensures(prop = ret)))]
pub fn returns_true(mut value: u32) -> bool {
    value += 1;
    value > 0
}

#[anneal(proof = "trivial")]
pub fn uses_explicit_tactic_proof(value: u32) -> u32 {
    value
}

#[anneal(proof(lean::term = "by trivial"))]
pub fn uses_explicit_term_proof(value: u32) -> u32 {
    value
}

#[test]
fn rust_conditions_expand_to_compilable_helpers() {
    assert!(needs_true(true));
    assert!(boolean_spec(true));
    assert!(returns_true(0));
    assert_eq!(uses_explicit_tactic_proof(1), 1);
    assert_eq!(uses_explicit_term_proof(2), 2);
}
