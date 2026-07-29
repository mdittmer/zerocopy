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
    theorem,
    spec(
        requires(name = "flag_true", prop = flag),
        ensures(name = "pass_thru", prop = ret == flag),
    ),
    proof(auto),
)]
pub fn boolean_theorem(flag: bool) -> bool {
    flag
}

#[anneal(theorem)]
pub fn theorem_only() -> bool {
    true
}

#[anneal(spec(ensures(prop = ret)))]
pub fn returns_true(mut value: u32) -> bool {
    value += 1;
    value > 0
}

#[anneal(spec(invariant(name = "bounded", prop = self.value <= 10)))]
pub struct Counter {
    value: u32,
}

pub trait Bounded {
    #[anneal(spec(requires(name = "bounded", prop = value <= 10)))]
    fn bounded(value: u32) -> u32;
}

pub struct Identity;

impl Bounded for Identity {
    fn bounded(value: u32) -> u32 {
        value
    }
}

#[test]
fn rust_conditions_expand_to_compilable_helpers() {
    assert!(needs_true(true));
    assert!(boolean_theorem(true));
    assert!(theorem_only());
    assert!(returns_true(0));

    let counter = Counter { value: 10 };
    assert!(counter.anneal__invariant__bounded());

    assert!(<Identity as Bounded>::anneal__bounded__bounded(10));
}
