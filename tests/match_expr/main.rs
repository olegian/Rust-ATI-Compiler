struct MyStruct {
    x: usize,
    y: usize,
}

enum MyEnum<'a> {
    V1,
    V2(usize),
    V3(MyStruct),
    V4(&'a [usize])
}

fn main() {
    let x = MyEnum::V1;
    let y = 10;
    foo(&x, y);

    let x = MyEnum::V2(30);
    let y = 40;
    foo(&x, y);

    let x = MyEnum::V4(&[1, 2, 3]);
    let y = 20;
    bar(&x, y);

    let x = MyEnum::V3(MyStruct {
        x: 0,
        y: 1,
    });
    let y = 20;
    baz(x, y);

    let x = MyEnum::V3(MyStruct {
        x: 2,
        y: 3,
    });
    let y = 30;
    quux(x, y);

    let x = 100;
    let y = 300;
    primitive(x, y);

    let a = 1;
    let b = 2;
    let c = 3;
    untracked_primitive("world", a, b, c);

    let a = 1;
    let b = 2;
    primitive_mut(a, b);
}

fn foo(x: &MyEnum, y: usize) -> usize {
    match x {
        MyEnum::V1 => 100,
        MyEnum::V2(x) => x + y,
        MyEnum::V3(MyStruct {
            x,
            ..
        }) => {
            x + y
        },
        MyEnum::V4(x) => {
            x.len() + y
        },
    }
}

fn bar(x: &MyEnum, y: usize) -> usize {
    match x {
        MyEnum::V1 => 100,
        MyEnum::V2(x) => x + y,
        MyEnum::V3(MyStruct {
            x,
            ..
        }) => {
            x + y
        },
        MyEnum::V4(x) => {
            x.len() + y
        },
    }
}

fn baz(mut x: MyEnum, y: usize) -> usize {
    match x {
        MyEnum::V1 => {
            y
        },
        MyEnum::V2(ref x) => {
            x + y
        },
        MyEnum::V3(ref mut my_struct) => {
            let MyStruct {
                x,
                y,
            } = my_struct;

            *x + *y
        },
        MyEnum::V4(ref slice) => {
            slice.len() + y
        },
    }
}

fn quux(mut x: MyEnum, y: usize) -> usize {
    match &mut x {
        MyEnum::V1 => {
            y
        },
        MyEnum::V2(x) => {
            *x + y
        },
        MyEnum::V3(my_struct) => {
            let MyStruct {
                x,
                y,
            } = my_struct;

            *x + *y
        },
        MyEnum::V4(slice) => {
            slice.len() + y
        },
    }
}

fn primitive(x: u32, y: u32) -> u32 {
    match x {
        0..=5  => { x + y }
        6..=10 => { x + y }
        _      => { y }
    }
}

fn primitive_mut(mut x: u32, y: u32) -> u32 {
    match &mut x {
        0..=5  => { x + y }
        6..=10 => { x + y }
        _      => { y }
    }
}

fn untracked_primitive(x: &str, a: u32, b: u32, c: u32) -> u32 {
    match x {
        "hello" => a,
        "world" => b,
        _ => c
    }
}
