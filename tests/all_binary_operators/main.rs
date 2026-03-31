#![allow(unused)]

// All the operators are:
// Add => "+",
// Sub => "-",
// Mul => "*",
// Div => "/",
// Rem => "%",
// And => "&&",
// Or => "||",
// BitXor => "^",
// BitAnd => "&",
// BitOr => "|",
// Shl => "<<",
// Shr => ">>",
// Eq => "==",
// Lt => "<",
// Le => "<=",
// Ne => "!=",
// Ge => ">=",
// Gt => ">",

#[ignore]
fn main() {
    add(1, 2, 99);
    sub(2, 1, 99);
    mul(1, 2, 99);
    div(2, 1, 99);
    rem(6, 3, 99);
    and(true, false, false);
    or(false, false, true);
    bit_xor(1, 2, 99);
    bit_and(1, 2, 99);
    bit_or(1, 2, 99);
    shl(1, 2, 99);
    shr(8, 2, 99);
    eq(1, 2, 99);
    lt(1, 2, 99);
    le(1, 2, 99);
    ne(1, 2, 99);
    ge(2, 2, 99);
    gt(1, 2, 99);
}

fn add(x: u32, y: u32, z: u32) -> u32 {
    return x + y;
}
fn sub(x: u32, y: u32, z: u32) -> u32 {
    return x - y;
}
fn mul(x: u32, y: u32, z: u32) -> u32 {
    return x * y;
}
fn div(x: u32, y: u32, z: u32) -> u32 {
    return x / y;
}
fn rem(x: u32, y: u32, z: u32) -> u32 {
    return x % y;
}
fn and(x: bool, y: bool, z: bool) -> bool {
    return x && y
}
fn or(x: bool, y: bool, z: bool) -> bool {
    return x || y;
}
fn bit_xor(x: u32, y: u32, z: u32) -> u32 {
    return x ^ y;
}
fn bit_and(x: u32, y: u32, z: u32) -> u32 {
    return x & y;
}
fn bit_or(x: u32, y: u32, z: u32) -> u32 {
    return x | y;
}
fn shl(x: u32, y: u32, z: u32) -> u32 {
    return x << y;
}
fn shr(x: u32, y: u32, z: u32) -> u32 {
    return x >> y;
}
fn eq(x: u32, y: u32, z: u32) -> bool {
    return x == y;
}
fn lt(x: u32, y: u32, z: u32) -> bool {
    return x < y;
}
fn le(x: u32, y: u32, z: u32) -> bool {
    return x <= y;
}
fn ne(x: u32, y: u32, z: u32) -> bool {
    return x != y;
}
fn ge(x: u32, y: u32, z: u32) -> bool {
    return x >= y;
}
fn gt(x: u32, y: u32, z: u32) -> bool {
    return x > y;
}