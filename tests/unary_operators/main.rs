#![allow(unused)]

// All the unary operators are:
// Neg => "-",
// Not => "!",
// Deref => "*",

#[ignore]
fn main() {
    negation(1, 2, 99);
    boolean_not(false, true, false);
    let mut x = 1;
    println!("{x:?}");
    dereference(&mut x, &2, &&99);
    println!("{x:?}");
}

fn negation(x: i32, y: i32, z: i32) -> i32 {
    return -x + y;
}
fn boolean_not(x: bool, y: bool, z: bool) -> bool {
    return (!x && !y) && z;
}
fn dereference(x: &mut u32, y: &u32, z: &&u32) {
    *x = y + *z;
}
