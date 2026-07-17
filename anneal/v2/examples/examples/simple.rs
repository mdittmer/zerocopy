pub fn unannotated_identity(flag: bool) -> bool {
    flag
}

#[anneal(spec(requires = "flag = true"))]
pub fn needs_true(flag: bool) -> bool {
    flag
}

pub fn forwards_checked(flag: bool) -> bool {
    if flag {
        needs_true(flag)
    } else {
        false
    }
}

fn main() {
    let _ = forwards_checked(true);
}
