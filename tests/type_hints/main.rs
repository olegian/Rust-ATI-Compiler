#![allow(unused)]

struct Point { x: u32, y: u32 }

fn struct_hints(a: u32, b: u32, unused: u32) -> Point {
    let p: Point = Point { x: a, y: b };
    p
}

fn primitive_hints(a: u32, b: u32, unused: u32) -> u32 {
    let sum: u32 = a + b;
    sum
}

fn turbofish_hints(a: u32, unused: u32) -> u32 {
    let v: Vec<u32> = Vec::<u32>::new();
    a
}

#[ignore]
fn main() {
    struct_hints(1, 2, 99);
    primitive_hints(3, 4, 99);
    turbofish_hints(5, 99);
}
