fn main() {
    foo(1, 2, 3);
}

fn foo(x: u32, y: u32, z: u32) -> u32 {
    let a = x + 100;
    let b = a + y;

    b
}
