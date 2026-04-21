fn main() {
    let n: usize = 5;
    sum_range(0, n, 0, 99);
    sum_range_inclusive(0, n, 0, 99);

    pass_range(1..5, 99);
    get_length(1..100, 1);
    reverse_sum(1..5);
    count_elements(1..5);
    fused_next(1..3);
    check_bounds(1..5);

    let arr = &[1; 10];
    index_with_range(arr, 1, 5);

    slice_and_modify([2; 10], 0..5, 3);
}

fn sum_range(lo: usize, hi: usize, mut acc: usize, unused: usize) -> usize {
    for i in lo..hi {
        acc = acc + i;
    }
    acc
}

fn sum_range_inclusive(lo: usize, hi: usize, mut acc: usize, unused: usize) -> usize {
    for i in lo..=hi {
        acc = acc + i;
    }
    acc
}

fn get_length(range: std::ops::Range<usize>, a: usize) -> usize {
    a + range.len()
}

fn pass_range(range: std::ops::Range<usize>, unused: usize) -> usize{
    let sum: usize = range.sum();
    sum
}

fn reverse_sum(range: std::ops::Range<usize>) -> usize {
    range.rev().sum()
}

fn count_elements(range: std::ops::Range<usize>) {
    let _n = range.count();
}

fn fused_next(range: std::ops::Range<usize>) -> usize {
    range.fuse().next().unwrap()
}

// Disabled: UFCS `ExactSizeIterator::len(&range)` requires `&Self` where
// Self impls ExactSizeIterator (supertrait: Iterator). Post-instrumentation,
// `&range` uniformly rewrites to `range.as_tagged_ref()` - yielding a
// `TaggedRef<Range<..>>`. We cannot impl `Iterator` on TaggedRef (next()
// needs `&mut` mutation through the inner field, but TaggedRef holds `&T`,
// not `&mut T`), so `ExactSizeIterator` is unreachable there too. The call
// shape is fundamentally incompatible with the owned-TaggedRef invariant;
// method-call style (`range.len()`) works via the inherent Tagged::len.
// fn exact_size(range: std::ops::Range<usize>) {
//     let _n = std::iter::ExactSizeIterator::len(&range);
// }

fn check_bounds(range: std::ops::Range<usize>) {
    use std::ops::RangeBounds;
    let _s = range.start_bound();
    let _e = range.end_bound();
}

fn index_with_range<'a>(arr: &'a [u32; 10], lo: usize, hi: usize) -> &'a [u32] {
    &arr[lo..hi]
}

fn slice_and_modify(mut arr: [u32; 10], range: std::ops::Range<usize>, value: u32) -> [u32; 10] {
    for i in range {
        arr[i] = value;
    }
    arr
}
