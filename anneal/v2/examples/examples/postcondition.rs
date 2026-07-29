use anneal_macros::anneal;

#[anneal(spec(ensures(name = "identity_result", prop = ret == x)))]
pub fn identity(x: u32) -> u32 {
    x
}

fn main() {
    let _ = identity(7);
}
