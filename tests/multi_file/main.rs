#![allow(unused)]

mod dep;

#[ignore]
fn main() {
    foo(1, 2, 3);
    let a = dep::foo(4, 5, 6);
    let b = dep::foo0(7, 8, 9);
}

fn foo(x: u32, y: u32, unused: u32) -> u32 {
    x + y
}
