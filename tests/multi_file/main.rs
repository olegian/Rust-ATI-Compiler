#![allow(unused)]

mod dep;

#[ignore]
fn main() {
    foo(1, 2, 3);

    let a = dep::from_dep(4, 5, 6);
}

fn foo(x: u32, y: u32, unused: u32) -> u32 {
    x + y
}
