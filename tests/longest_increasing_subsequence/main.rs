#![allow(unused)]

#[ignore]
fn main() {
    let mut arr: [u32; 20] = [ 1, 2, 3, 4, 1, 1, 2, 3, 1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1, 0];
    let a = lis(&arr);
}

fn lis<'a, T>(haystack: &'a [T]) -> (usize, &'a [T]) where T: std::cmp::Ord {
    let mut longest = (0, 0);
    let mut lo = 0;
    for hi in 1..haystack.len() {
        if haystack[hi-1] >= haystack[hi] {
            lo = hi;
        } else if longest.1 - longest.0 < hi - lo {
            longest = (lo, hi);
        }
    }

    (longest.1 - longest.0 + 1, &haystack[longest.0..(longest.1 + 1)])
}
