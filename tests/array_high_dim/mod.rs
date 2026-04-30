use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn array_2d() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_high_dim/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_high_dim/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_high_dim/main.rs::foo:::ENTER",
        ))
        // holy specifying this is so tedious
        // im really not sure what a better method is
        // given that each element now really can be in a different AT.
        .register("arr.length", 7)
        .register("arr[0].length", 6)
        .register("arr[1].length", 6)
        .register("arr[2].length", 6)
        .register("arr[0][0].length", 3)
        .register("arr[0][1].length", 4)
        .register("arr[0][2].length", 5)
        .register("arr[1][0].length", 3)
        .register("arr[1][1].length", 4)
        .register("arr[1][2].length", 5)
        .register("arr[2][0].length", 3)
        .register("arr[2][1].length", 4)
        .register("arr[2][2].length", 5)

        .register("arr[0][0][0]", 0)
        .register("arr[0][0][1]", 0)
        .register("arr[0][0][2]", 0)
        .register("arr[0][1][0]", 1)
        .register("arr[0][1][1]", 1)
        .register("arr[0][1][2]", 1)
        .register("arr[0][2][0]", 2)
        .register("arr[0][2][1]", 2)
        .register("arr[0][2][2]", 2)

        .register("arr[1][0][0]", 0)
        .register("arr[1][0][1]", 0)
        .register("arr[1][0][2]", 0)
        .register("arr[1][1][0]", 1)
        .register("arr[1][1][1]", 1)
        .register("arr[1][1][2]", 1)
        .register("arr[1][2][0]", 2)
        .register("arr[1][2][1]", 2)
        .register("arr[1][2][2]", 2)

        .register("arr[2][0][0]", 0)
        .register("arr[2][0][1]", 0)
        .register("arr[2][0][2]", 0)
        .register("arr[2][1][0]", 1)
        .register("arr[2][1][1]", 1)
        .register("arr[2][1][2]", 1)
        .register("arr[2][2][0]", 2)
        .register("arr[2][2][1]", 2)
        .register("arr[2][2][2]", 2)

        .register("a", 8)
        .register("b", 9)
        .register("unused", 10),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_high_dim/main.rs::foo:::EXIT",
        ))
        .register("arr.length", 6)
        .register("arr[0].length", 6)
        .register("arr[1].length", 6)
        .register("arr[2].length", 6)
        .register("arr[0][0].length", 3)
        .register("arr[0][1].length", 4)
        .register("arr[0][2].length", 5)
        .register("arr[1][0].length", 3)
        .register("arr[1][1].length", 4)
        .register("arr[1][2].length", 5)
        .register("arr[2][0].length", 3)
        .register("arr[2][1].length", 4)
        .register("arr[2][2].length", 5)

        .register("arr[0][0][0]", 0)
        .register("arr[0][0][1]", 0)
        .register("arr[0][0][2]", 0)
        .register("arr[0][1][0]", 1)
        .register("arr[0][1][1]", 1)
        .register("arr[0][1][2]", 1)
        .register("arr[0][2][0]", 2)
        .register("arr[0][2][1]", 2)
        .register("arr[0][2][2]", 2)

        .register("arr[1][0][0]", 0)
        .register("arr[1][0][1]", 0)
        .register("arr[1][0][2]", 0)
        .register("arr[1][1][0]", 1)
        .register("arr[1][1][1]", 1)
        .register("arr[1][1][2]", 1)
        .register("arr[1][2][0]", 2)
        .register("arr[1][2][1]", 2)
        .register("arr[1][2][2]", 2)

        .register("arr[2][0][0]", 0)
        .register("arr[2][0][1]", 0)
        .register("arr[2][0][2]", 0)
        .register("arr[2][1][0]", 1)
        .register("arr[2][1][1]", 1)
        .register("arr[2][1][2]", 1)
        .register("arr[2][2][0]", 2)
        .register("arr[2][2][1]", 2)
        .register("arr[2][2][2]", 2)

        .register("a", 6)
        .register("b", 0)
        .register("unused", 10)
        .register("return", 11)
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("array_high_dim.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
