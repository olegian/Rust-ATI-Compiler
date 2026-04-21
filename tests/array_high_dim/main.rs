fn main() {
    let repeat = [[[1; 3], [2; 3], [3; 3]]; 3];
    foo(repeat, 1, 2, 99);
}

fn foo(arr: [[[u32; 3]; 3]; 3], a: usize, b: u32, unused: u32) -> usize {
    let tmp = if arr[0].len() > arr.len() {
        arr[0].len()
    } else {
        arr[1].len()
    };

    let tmp2 = tmp + a;
    let tmp3 = arr[0][0][0] * b;

    99
}