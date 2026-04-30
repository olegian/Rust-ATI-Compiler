use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn array_of_struct() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_of_struct/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_of_struct/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_of_struct/main.rs::foo:::ENTER",
        ))
        .register("a.length", 0)

        .register("a[0].a", 1)
        .register("a[0].b.length", 2)
        .register("a[0].b[0]", 3)
        .register("a[0].b[1]", 3)
        .register("a[0].b[2]", 3)

        .register("a[1].a", 1)
        .register("a[1].b.length", 2)
        .register("a[1].b[0]", 3)
        .register("a[1].b[1]", 3)
        .register("a[1].b[2]", 3)

        .register("a[2].a", 1)
        .register("a[2].b.length", 2)
        .register("a[2].b[0]", 3)
        .register("a[2].b[1]", 3)
        .register("a[2].b[2]", 3)

        .register("b.length", 4)

        .register("b[0].a", 5)
        .register("b[0].b.length", 6)
        .register("b[0].b[0]", 7)
        .register("b[0].b[1]", 7)
        .register("b[0].b[2]", 7)

        .register("b[1].a", 8)
        .register("b[1].b.length", 9)
        .register("b[1].b[0]", 10)
        .register("b[1].b[1]", 10)
        .register("b[1].b[2]", 10)

        .register("c", 11)
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_of_struct/main.rs::foo:::EXIT",
        ))
        .register("a.length", 0)
        .register("a[0].a", 1)
        .register("a[0].b.length", 2)
        .register("a[0].b[0]", 3)
        .register("a[0].b[1]", 3)
        .register("a[0].b[2]", 3)

        .register("a[1].a", 1)
        .register("a[1].b.length", 2)
        .register("a[1].b[0]", 3)
        .register("a[1].b[1]", 3)
        .register("a[1].b[2]", 3)

        .register("a[2].a", 1)
        .register("a[2].b.length", 2)
        .register("a[2].b[0]", 3)
        .register("a[2].b[1]", 3)
        .register("a[2].b[2]", 3)

        .register("b.length", 0)

        .register("b[0].a", 5)
        .register("b[0].b.length", 6)
        .register("b[0].b[0]", 7)
        .register("b[0].b[1]", 7)
        .register("b[0].b[2]", 7)

        .register("b[1].a", 1)
        .register("b[1].b.length", 9)
        .register("b[1].b[0]", 10)
        .register("b[1].b[1]", 10)
        .register("b[1].b[2]", 10)

        .register("c", 1)
        .register("return", 0)
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("array_of_struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
