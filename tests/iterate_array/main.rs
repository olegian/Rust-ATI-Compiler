fn main() {
    let array = [1, 2, 3, 4, 5];
    implicit_iter(&array, 1);

    let array = [1, 2, 3, 4, 5];
    implicit_iter_mut(array, 1);

    let array = [1, 2, 3, 4, 5];
    explicit_iter(&array, 1);

    let mut array = [1, 2, 3, 4, 5];
    explicit_iter_mut(&mut array, 1);

    let array = [1, 2, 3, 4, 5];
    explicit_into_iter(array, 1);

    let array = [1, 2, 3, 4, 5];
    implicit_iter_slice(&array, 1);

    let array = [1, 2, 3, 4, 5];
    explicit_iter_slice(&array, 1);

    let mut array = [1, 2, 3, 4, 5];
    explicit_iter_mut_slice(&mut array, 1);

    let mut array = [1, 2, 3, 4, 5];
    enumerate_iter(&mut array, 1);
}

fn implicit_iter(arr: &[u32; 5], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr {
        acc += elem + val;
    }

    acc
}

fn implicit_iter_mut(mut arr: [u32; 5], val: u32) -> [u32; 5] {
    let mut acc = 0;

    for elem in &mut arr {
        acc += *elem + val;
    }

    arr
}


fn explicit_iter(arr: &[u32; 5], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr.iter() {
        acc += elem + val;
    }

    acc
}

fn explicit_iter_mut(arr: &mut [u32; 5], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr.iter_mut() {
        acc += *elem + val;
    }

    acc
}

fn explicit_into_iter(arr: [u32; 5], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr.into_iter() {
        acc += elem + val;
    }

    acc
}

fn implicit_iter_slice(arr: &[u32], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr {
        acc += elem + val;
    }

    acc
}

fn explicit_iter_slice(arr: &[u32], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr.iter() {
        acc += elem + val;
    }

    acc
}

fn explicit_iter_mut_slice(arr: &mut [u32], val: u32) -> u32 {
    let mut acc = 0;

    for elem in arr.iter_mut() {
        acc += *elem + val;
    }

    acc
}

fn enumerate_iter(arr: &mut [u32], val: u32) -> usize {
    let mut acc = 0;

    for (i, elem) in arr.iter_mut().enumerate() {
        acc += i;
        *elem += val;
    }

    acc
}

