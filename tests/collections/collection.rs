// This test is difficult and scary
// look at statements.rs for why
fn main() {
    let v = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);

    foo(1, 2, 3);
}

fn foo(vec: Vec<u32>, x: u32, y: u32) -> u32 {
    let a = vec[0] + x;
    let b = vec[0] + y;

    vec[0]
}
