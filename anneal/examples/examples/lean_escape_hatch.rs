// Copyright 2026 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://opensource.org/licenses/MIT>, or the MIT
// license <LICENSE-MIT>, at your option.

// Quoted propositions retain an explicit Lean escape hatch for conditions
// which cannot yet be expressed in supported Rust.
#[anneal_macros::anneal(spec(ensures(
    name = "lean_identity",
    prop = "result.val = value.val",
)))]
pub fn identity(value: u32) -> u32 {
    value
}

fn main() {
    let _ = identity(7);
}
