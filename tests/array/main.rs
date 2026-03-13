fn main() {
    let repeat = [1; 3];
    foo(repeat, 2, 3, 4);

    let array = [10, 20, 30];
    bar(&array, 2, 3, 4);
}

fn foo(arr: [u32; 3], x: u32, y: u32, z: u32) -> u32 {
    let tmp = arr[0] + x;
    let tmp2 = arr[0] + y;

    arr[1] // note this should be in the same AT as arrays hold values of same tag
}

fn bar(arr: &[usize], x: usize, y: usize, z: usize) -> usize {
    let tmp = arr.len() + z;
    let tmp2 = arr.len() + y;

    arr[0]
}
