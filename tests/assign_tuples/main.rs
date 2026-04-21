#![allow(unused)]

#[ignore]
fn main() {
    let a1 = (10, (20, "hello"), true);
    let b1 = (30, (40, "world"), false);
    assign_nested_tuple(a1, b1);

    let mut a2 = (1, 2, true);
    let b2 = (3, 4, false);
    mutate_tuple(&mut a2, b2, 1);
}

fn assign_nested_tuple<'a>(mut a: (i32, (i32, &'a str), bool), b: (i32, (i32, &'a str), bool)) {
    a = b;
}

fn mutate_tuple(target: &mut (u32, u32, bool), value: (u32, u32, bool), a: u32) {
    let tmp = (value.0, value.1 + a, value.2);
    *target = tmp;
}
