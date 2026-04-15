fn main() {
    let repeat = [[[1; 3], [2; 3], [3; 3]]; 3];
    println!("{:#?}", repeat);
    foo(repeat, 1, 99);
}

fn foo(arr: [[[u32; 3]; 3]; 3], a: usize, unusued: u32) -> usize {
    let tmp = if arr[0].len() > arr.len() {
        arr[0].len()
    } else {
        arr[1].len()
    };

    0
}