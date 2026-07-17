#[anneal(spec(ensures = "result.val = x.val"))]
pub fn identity(x: u32) -> u32 {
    x
}

fn main() {
    let _ = identity(7);
}
