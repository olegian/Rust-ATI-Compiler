#![allow(unused)]

#[ignore]
fn main() {
    addassign(1, 2, 3);
    subassign(2, 1, 3);
    mulassign(1, 2, 3);
    divassign(4, 2, 3);
    remassign(4, 2, 3);
    bitxorassign(1, 2, 3);
    bitandassign(1, 2, 3);
    bitorassign(1, 2, 3);
    shlassign(1, 2, 3);
    shrassign(8, 2, 3);
}

fn addassign(mut x: u32, y: u32, z: u32) -> u32 {
    x += y;
    z
}

fn subassign(mut x: u32, y: u32, z: u32) -> u32 {
    x -= y;
    z
}

fn mulassign(mut x: u32, y: u32, z: u32) -> u32 {
    x *= y;
    z
}
fn divassign(mut x: u32, y: u32, z: u32) -> u32 {
    x /= y;
    z
}
fn remassign(mut x: u32, y: u32, z: u32) -> u32 {
    x %= y;
    z
}
fn bitxorassign(mut x: u32, y: u32, z: u32) -> u32 {
    x ^= y;
    z
}
fn bitandassign(mut x: u32, y: u32, z: u32) -> u32 {
    x &= y;
    z
}
fn bitorassign(mut x: u32, y: u32, z: u32) -> u32 {
    x |= y;
    z
}
fn shlassign(mut x: u32, y: u32, z: u32) -> u32 {
    x <<= y;
    z
}
fn shrassign(mut x: u32, y: u32, z: u32) -> u32 {
    x >>= y;
    z
}