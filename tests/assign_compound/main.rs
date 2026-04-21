
struct MyTuple(u32, u32);
struct MyStruct {
    a: u32,
    b: u32,
}
enum MyEnum {
    StructVariant {
        a: u32,
        b: u32,
    },
    TupleVariant(u32, u32),
    UnitVariant,
}


#[ignore]
fn main() {
    let a = MyTuple(1, 2);
    let b = MyTuple(3, 4);
    assign_tuple(a, b);


    let c = MyStruct {
        a: 1,
        b: 2,
    };
    let d = MyStruct {
        a: 3,
        b: 4,
    };
    assign_struct(c, d);

    let e = MyEnum::StructVariant { a: 1, b: 2 };
    let f = MyEnum::StructVariant { a: 3, b: 4 };
    let g = MyEnum::TupleVariant(1, 2);
    let h = MyEnum::TupleVariant(3, 4);
    let i = MyEnum::UnitVariant;
    let j = MyEnum::UnitVariant;

    assign_enum(e, f);
    assign_enum(g, h);
    assign_enum(i, j);

    // assigning between different variants
    let k = MyEnum::StructVariant { a: 3, b: 4 };
    let l = MyEnum::TupleVariant(3, 4);
    assign_enum(k, l);
}

fn assign_struct(mut a: MyStruct, b: MyStruct) {
    a = b;
}

fn assign_tuple(mut a: MyTuple, b: MyTuple) {
    a = b;
}

fn assign_enum(mut a: MyEnum, b: MyEnum) {
    a = b;
}
