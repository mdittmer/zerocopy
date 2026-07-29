// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use anneal_macros::anneal;

#[anneal(theorem, proof(auto))]
pub fn composed(value: u32) -> bool {
    pass_through(value) == value && later_reflexive(value)
}

#[anneal(spec(ensures(
    name = "pass_through_result",
    prop = ret == value,
)))]
pub fn pass_through(value: u32) -> u32 {
    value
}

#[anneal(
    theorem,
    spec(
        requires(name = "self_equal", prop = value == value),
        ensures(name = "returns_true", prop = ret == true),
    ),
)]
pub fn later_reflexive(value: u32) -> bool {
    value == value
}

#[anneal(spec(ensures(
    name = "identity_result",
    prop = composed(ret),
)))]
pub fn identity(value: u32) -> u32 {
    value
}

fn main() {
    let _ = identity(7);
}
