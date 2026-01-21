mod basic;

fn main() {
    let a = 10;
    let b = 20;
    let c = a + b;
    println!("FROM MAIN");
    basic::deps::foo();

    let d = 300;
    let e = 400;
    let f = bar(d, e);

    return;
}

fn bar(x: u32, y: u32) -> u32 {
    let res = x + y;
    res
}
