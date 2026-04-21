#![allow(unused)]
mod struct_defs;

struct Inner {
    x: f64,
    y: bool,
}

struct MyStruct {
    x: u32,
    y: u32,
    z: Inner,
}

#[ignore]
fn main() {
    let s = MyStruct {
        x: 1,
        y: 2,
        z: Inner { x: 3.0, y: true },
    };

    let z2 = 1.0;
    func(s, 10, 20, 30, z2);


    let a = struct_defs::Struct {
        a: 0,
        b: false,
        c: struct_defs::Inner::new(9, 10),
    };
    let a = foo(a, 10);

    // introducing a little bit of dependancy, the previous foo call should
    // have merged a.a and a.c.x. This means that ENTER site to bar
    // should see these vars already in the same AT, as this is the only
    // place we invoke this function.
    let b = struct_defs::TupleStruct(0, true, struct_defs::Inner::new(10, 9));
    bar(b, a);

    let ca = 10;
    let cb = 20;
    let mut cc = 30;
    let cd = 40;

    let c = struct_defs::ContainsRef {
        a: ca,
        b: &cb,
        c: &mut cc,
        d: &&&cd,
    };
    baz(c, 100);
}

fn func(s: MyStruct, x: u32, y: u32, z: u32, z2: f64) -> u32 {
    let a = s.x + x;
    let b = s.y + y;
    let c = s.y + s.x;
    let d = z2 + s.z.x;

    y
}

fn foo(mut a: struct_defs::Struct, v: u64) -> struct_defs::Struct {
    a.a += v;
    a.c.add_x(v);
    a
}

fn bar(mut a: struct_defs::TupleStruct, b: struct_defs::Struct) -> struct_defs::TupleStruct {
    a.1 ^= b.b;
    a
}

// TODO: this mut is visious,
// its unnecessary with normal rustc, but with DATIR is required
// due to the way .assign captures &mut self
fn baz(mut a: struct_defs::ContainsRef, v: i64) {
    let tmp = a.a + v;
    let tmp2 = **a.d + v;
    *a.c = tmp + tmp2;
}
