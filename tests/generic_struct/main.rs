#![allow(unused)]

struct MyStruct<A, B> {
    val: A,
    unused: B,
}

impl<A, B> MyStruct<A, B> where A: std::ops::AddAssign, B: Copy {
    fn new(val: A, unused: B) -> Self {
        MyStruct {
            val, unused
        }
    }

    fn foo(&mut self, val: A) -> B {
        self.val += val;
        self.unused
    }
}

fn main() {
    let a = MyStruct::new(10, 99.9);
    foo(a, 1, 99.9);
}

fn foo<A, B>(mut a: MyStruct<A, B>, b: A, unused: B) -> MyStruct<A, B> where A: std::ops::AddAssign, B: Copy {
    // merge together a.val and b
    a.foo(b);
    a
}
