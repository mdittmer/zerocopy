#[anneal(spec(invariant = "self.value.val <= 10"))]
pub struct Counter {
    value: u32,
}

#[anneal(spec(requires = "value.val <= 10"))]
pub fn make(value: u32) -> Counter {
    Counter { value }
}

pub fn read(counter: Counter) -> u32 {
    counter.value
}

pub fn touch(_counter: &mut Counter) {}

pub fn call_read(counter: Counter) -> u32 {
    read(counter)
}

fn main() {
    let mut counter = make(4);
    touch(&mut counter);
    let _ = call_read(counter);
}
