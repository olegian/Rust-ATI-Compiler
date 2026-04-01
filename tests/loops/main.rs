#![allow(unused)]

/// todo: more interesting control flow structures, try out ranges / iterators / whatever
#[ignore]
fn main() {
    // will merge together a, b
    let a = if_expr(20, 1, 2, 3, 4);
    println!("a={a}");
    // will merge together a, d
    let b = if_expr(2,  1, 2, 3, 4);
    println!("b={b}");

    // while_expr(5, 0, 99);
    // loop_expr(5, 0, 99);
}

fn if_expr(branch: u32, a: u32, b: u32, c: u32, d: u32) -> u32 {
    if (branch > 10) {
        a + b
    } else if { branch > 5} {
        a + c
    } else {
        a + d
    }
}

fn while_expr(iters: usize, mut a: usize, unused: u32) {
    // i is compared to iters, and added to a, so a and iters in same AT
    let mut i: usize = 0;
    while i < iters {
        a += i;
        i += 1;
    }
}

fn loop_expr(iters: usize, mut a: usize, unused: u32) {
    let mut i: usize = 0;
    loop {
        if i < iters {
            break;
        }

        a += i;
        i += 1;
    }
}

// FIXME: ranges / iterators need to be figured out before this will work
// fn for_loop(iters: usize, mut a: usize, unused: u32) {
//     for i in 0..iters { }
// }