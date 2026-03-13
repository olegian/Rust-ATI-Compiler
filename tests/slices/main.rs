fn main() {
    let array = [1, 2, 3];
    let v = array.to_vec();

    // foo(1, 2, slice);
}

fn foo<'a>(x: u32, y: u32, slice: &'a [u32]) -> &'a [u32] {
    let AAA = slice.len();

    let tmp = slice[1] + x;
    let tmp2 = slice[1] + y;

    slice
}
