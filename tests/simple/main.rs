#![allow(unused)]

#[ignore]
fn main() {
    let x = 1;
    let y = 2;
    let z = 3;
    foo(x, y, z);
    foo(z, x, y);
}

fn foo(x: u32, y: u32, z: u32) -> u32 {
    let tmp = x + y;

    tmp
}
