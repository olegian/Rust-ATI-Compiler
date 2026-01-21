fn main() {
    let a = 10;
    let b = 20;
    let c = a + b;

    let d = 1;
    let e = 2;
    let f = max(d, e);

    return;
}

fn max(x: u32, y: u32) -> u32 {
    let res = if x < y {
        y
    } else {
        x
    };

    return res;
}
