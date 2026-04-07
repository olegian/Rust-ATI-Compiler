#![allow(unused)]

#[ignore]
fn main() {
    let a1 = [1; 10];
    let mut a2 = [2; 10];

    let s1 = &a1[..a1.len()];
    let a2_len = a2.len();
    let s2 = &mut a2[a2_len..];

    copy(s1, s2, 99);
}

fn copy(from: &[u32], to: &mut [u32], unused: u32) -> Option<()> {
    if from.len() != to.len() {
        return None;
    }

    for i in 0..from.len() {
        to[i] = from[i];
    }

    Some(())
}
