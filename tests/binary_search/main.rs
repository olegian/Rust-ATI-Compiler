#![allow(unused)]

#[ignore]
fn main() {
    // This would be really nice to write, but iterators are not yet functional...
    // let arr: Vec<u32> = (0..100).collect();
    let arr = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
        49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,
        72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94,
        95, 96, 97, 98, 99, 100,
    ];

    let a = concrete_bin_search(&arr, 50, 0, arr.len());
    let b = concrete_bin_search(&arr, 101, 0, arr.len());
    let c = concrete_bin_search(&arr, 3, 0, arr.len());

    println!("{a:?}, {b:?}, {c:?}")
}

fn generic_bin_search<T>(haystack: &[T], needle: T, lo: usize, hi: usize) -> Option<usize>
where
    T: std::cmp::PartialOrd,
{
    if haystack.len() <= lo {
        return None;
    }

    let mid = lo + (hi - lo) / 2;
    return if haystack[mid] == needle {
        Some(mid)
    } else if needle < haystack[mid] {
        generic_bin_search(haystack, needle, lo, mid - 1)
    } else {
        generic_bin_search(haystack, needle, mid + 1, hi)
    }
}


fn concrete_bin_search(haystack: &[u32], needle: u32, lo: usize, hi: usize) -> Option<usize> {
    if haystack.len() <= lo {
        return None;
    }

    let mid = lo + (hi - lo) / 2;
    return if haystack[mid] == needle {
        Some(mid)
    } else if needle < haystack[mid] {
        concrete_bin_search(haystack, needle, lo, mid - 1)
    } else {
        concrete_bin_search(haystack, needle, mid + 1, hi)
    }
}
