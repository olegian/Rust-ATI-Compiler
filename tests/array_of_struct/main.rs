#[derive(Clone, Copy)]
struct MyStruct {
    a: u32,
    b: [u32; 3],
}

fn main() {
    let a = [MyStruct {a: 0, b: [1; 3]}; 3];
    let b = [
        MyStruct {
            a: 2, 
            b: [3; 3],
        },
        MyStruct {
            a: 4, 
            b: [5; 3],
        },
    ];

    foo(&a, b, 100);
}

fn foo(a: &[MyStruct], b: [MyStruct; 2], c: u32) -> usize {
    let tmp = a[0].a + c;
    let tmp2 = b[1].a + c;

    a.len() + b.len()
}

