#![feature(register_tool)]
#![register_tool(aeneas)]

#[anneal::type_invariant(is_valid = "self.val.val > 0")]
pub struct Positive {
    pub val: u32,
}

#[anneal::trait_invariant(is_safe = "True")]
pub trait MyTrait {
    fn is_even(&self) -> bool;
}

pub struct MyStruct {
    pub val: u32,
}

impl MyStruct {
    #[anneal::spec(
        requires(h_overflow = "self.val.val < 4294967295"),
        ensures(result = "ret.val = self.val.val + 1")
    )]
    pub unsafe fn incr(&self) -> u32 {
        self.val + 1
    }

    #[anneal::spec(
        requires(h_x = "x.val > 0"),
        requires(h_overflow_y = "old_y.val + x.val < 4294967296"),
        requires(h_overflow_ret = "self.val.val + old_y.val + x.val < 4294967296"),
        ensures(h_y = "y.val = old_y.val + x.val")
    )]
    pub unsafe fn add_val(&self, x: u32, y: &mut u32) -> u32 {
        *y += x;
        self.val + *y
    }
}
