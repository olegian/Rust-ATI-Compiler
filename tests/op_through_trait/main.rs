#![allow(unused)]
use std::ops::Add;

struct NewTuple(u32, u32, bool);
impl Add for NewTuple {
    type Output = NewTuple;

    fn add(self, rhs: Self) -> Self::Output {
        NewTuple(self.0 + rhs.0, self.1 + rhs.1, self.2 || self.2)
    }
}

struct NewStruct {
    a: u32,
    b: u32,
    c: bool,
}
impl std::ops::Mul for NewStruct {
    type Output = NewStruct;

    fn mul(self, rhs: Self) -> Self::Output {
        NewStruct {
            a: self.a * rhs.a,
            b: self.b * rhs.b,
            c: self.c && rhs.c,
        }
    }
}

struct Nested {
    a: u32,
    b: NewTuple,
}
impl Add for Nested {
    type Output = Nested;

    fn add(self, rhs: Self) -> Self::Output {
        Nested {
            a: self.a + rhs.a,
            b: self.b + rhs.b,
        }
    }
}

#[ignore]
fn main() {
    let a = NewTuple(0, 1, true);
    let b = NewTuple(2, 3, false);
    let c = NewTuple(99, 99, false);
    foo(a, b, c);

    let d = NewStruct {
        a: 0,
        b: 1,
        c: true,
    };
    let e = NewStruct {
        a: 2,
        b: 3,
        c: false,
    };
    let f = NewStruct {
        a: 99,
        b: 99,
        c: false,
    };
    bar(d, e, f);

    let g = Nested {
        a: 0,
        b: NewTuple(1, 2, true),
    };
    let h = Nested {
        a: 3,
        b: NewTuple(4, 5, false),
    };
    let i = Nested {
        a: 4,
        b: NewTuple(5, 6, false),
    };
    baz(g, h, i);
}

fn foo(a: NewTuple, b: NewTuple, c: NewTuple) -> NewTuple {
    a + b
}

fn bar(a: NewStruct, b: NewStruct, c: NewStruct) -> NewStruct {
    a * b
}

fn baz(a: Nested, b: Nested, c: Nested) -> Nested {
    a + b
}
