#![allow(unused)]

struct Inner {
    x: f64,
    y: bool,
}

struct MyStruct {
    x: u32,
    y: u32,
    z: Inner,
}

#[ignore]
fn main() {
    let s = MyStruct {
        x: 1,
        y: 2,
        z: Inner { x: 3.0, y: true },
    };

    let z2 = 1.0;
    func(s, 10, 20, 30, z2);
}

fn func(s: MyStruct, x: u32, y: u32, z: u32, z2: f64) -> u32 {
    let a = s.x + x;
    let b = s.y + y;
    let c = s.y + s.x;
    let d = z2 + s.z.x;

    y
}
