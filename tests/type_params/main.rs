#![allow(unused)]

// TODO: change this test to use stuff like .collect::<Vec<_>>()
// maybe quickly reimplemnt vec in here?
#[ignore]
fn main() {
    let x: u32 = 10;
    let y: u32 = 20;
    let z: u32 = 30;

    let v: Vec<_> = Vec::<Box<u32>>::from([Box::new(z)]);

    foo(x, y, z, &v);
}

fn foo(x: u32, y: u32, z: u32, v: &Vec<Box<u32>>) -> u32 {
    let tmp = *v[0] + x;
    let tmp2 = *v[0] + y;

    z
}
