#![feature(register_tool)]
#![register_tool(aeneas)]

pub struct MyStruct {
    pub val: u32,
}

impl MyStruct {
    #[anneal::spec(
        ensures(result = "ret.val = self.val.val + 1")
    )]
    pub fn incr(&self) -> u32 {
        self.val + 1
    }

    #[anneal::spec(
        requires(h_x = "x.val > 0"),
        ensures(h_y = "y.val = old_y.val + x.val")
    )]
    pub unsafe fn add_val(&self, x: u32, y: &mut u32) -> u32 {
        *y += x;
        self.val + *y
    }
}
