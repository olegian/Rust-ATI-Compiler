/// FIXME: FINISH THIS TEST
/// how should slices work??
/// consider:
/// [
///   &a,  --> [1, 1, 1]
///   &b,  --> [2, 2, 2, 2]    <- would a value interacting with this b's length mean that 
///   &c,  --> [3, 3, 3, 3, 3]      it is also comparable with other slice lengths?
/// ]              ^
///          would a value interacting with this element in slice c
///        be comparable with all other elements in all other slices?
fn main() {
    let a = [1; 3];
    let b = [2; 4];
    let c = [3; 5];
    let arr = [&a[..], &b[..], &c[..]];
    foo(arr, 1, 2, 99);

    let slice = &mut [arr; 3][..];
    bar(slice, 1, 2, 99);
}

fn foo(arr: [&[u32]; 3], a: usize, b: u32, unused: u32) -> usize {
    let tmp = if arr[0].len() > arr.len() {
        arr[0].len()
    } else {
        arr[1].len()
    };

    let tmp2 = tmp + a;
    let tmp3 = arr[0][0] * b;

    99
}

fn bar(slice: &mut [[&[u32]; 3]], a: usize, b: u32, unused: u32) -> usize {
    let tmp = if slice[0].len() > slice.len() {
        slice[0].len()
    } else {
        slice[1].len()
    };

    let tmp2 = tmp + a;
    let tmp3 = slice[0][0][0] * b;

    99
}