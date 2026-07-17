pub trait Bounded {
    #[anneal(spec(requires = "x.val <= 10"))]
    fn bounded(x: u32) -> u32;
}

pub struct Impl;

impl Bounded for Impl {
    fn bounded(x: u32) -> u32 {
        x
    }
}

#[anneal(spec(requires = "x.val <= 10"))]
pub fn call_impl(x: u32) -> u32 {
    Impl::bounded(x)
}

fn main() {
    let _ = call_impl(3);
}
