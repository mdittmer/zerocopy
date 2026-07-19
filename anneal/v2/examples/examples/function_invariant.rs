#[anneal(spec(invariant(name = "bounded", prop = "x.val <= 10")))]
pub fn bounded_identity(x: u32) -> u32 {
    x
}

#[anneal(spec(requires(name = "bounded", prop = "x.val <= 10")))]
pub fn forwards_bounded(x: u32) -> u32 {
    bounded_identity(x)
}

fn main() {
    let _ = forwards_bounded(3);
}
