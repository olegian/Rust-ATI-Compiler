use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn array_with_slices() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_with_slices/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array_with_slices/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_with_slices/main.rs::foo:::ENTER",
        ))
        // FIXME: specifying jagged dimensional arrays is really difficult actually
        .register("arr[0][0]", 0)
        .register("arr[0][1]", 0)
        .register("arr[0][2]", 0)
        .register("arr[0].length", 1)
        .register("arr[1][0]", 2)
        .register("arr[1][1]", 2)
        .register("arr[1][2]", 2)
        .register("arr[1][3]", 2)
        .register("arr[1].length", 3)
        .register("arr[2][0]", 4)
        .register("arr[2][1]", 4)
        .register("arr[2][2]", 4)
        .register("arr[2][3]", 4)
        .register("arr[2][4]", 4)
        .register("arr[2].length", 5)
        .register("arr.length", 6)
        .register("a", 7)
        .register("b", 8)
        .register("unused", 9),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_with_slices/main.rs::foo:::EXIT",
        ))
        .register("arr[0][0]", 8)
        .register("arr[0][1]", 0)
        .register("arr[0][2]", 0)
        .register("arr[0].length", 1)
        .register("arr[1][0]", 2)
        .register("arr[1][1]", 2)
        .register("arr[1][2]", 2)
        .register("arr[1][3]", 2)
        .register("arr[1].length", 3)
        .register("arr[2][0]", 4)
        .register("arr[2][1]", 4)
        .register("arr[2][2]", 4)
        .register("arr[2][3]", 4)
        .register("arr[2][4]", 4)
        .register("arr[2].length", 5)
        .register("arr.length", 1)
        .register("a", 3)
        .register("b", 8)
        .register("unused", 9)

        .register("return", 7),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_with_slices/main.rs::bar:::ENTER",
        ))
        // FIXME: specifying jagged dimensional arrays is really difficult actually
        .register("slice[0][0][0]", 0)
        .register("slice[0][0][1]", 0)
        .register("slice[0][0][2]", 0)
        .register("slice[0][0].length", 1)
        .register("slice[0][1][0]", 2)
        .register("slice[0][1][1]", 2)
        .register("slice[0][1][2]", 2)
        .register("slice[0][1][3]", 2)
        .register("slice[0][1].length", 3)
        .register("slice[0][2][0]", 4)
        .register("slice[0][2][1]", 4)
        .register("slice[0][2][2]", 4)
        .register("slice[0][2][3]", 4)
        .register("slice[0][2][4]", 4)
        .register("slice[0][2].length", 5)
        .register("slice[0].length", 1) // note same set as above because of previous interactions

        .register("slice[1][0][0]", 0)
        .register("slice[1][0][1]", 0)
        .register("slice[1][0][2]", 0)
        .register("slice[1][0].length", 1)
        .register("slice[1][1][0]", 2)
        .register("slice[1][1][1]", 2)
        .register("slice[1][1][2]", 2)
        .register("slice[1][1][3]", 2)
        .register("slice[1][1].length", 3)
        .register("slice[1][2][0]", 4)
        .register("slice[1][2][1]", 4)
        .register("slice[1][2][2]", 4)
        .register("slice[1][2][3]", 4)
        .register("slice[1][2][4]", 4)
        .register("slice[1][2].length", 5)
        .register("slice[1].length", 1)

        .register("slice[2][0][0]", 0)
        .register("slice[2][0][1]", 0)
        .register("slice[2][0][2]", 0)
        .register("slice[2][0].length", 1)
        .register("slice[2][1][0]", 2)
        .register("slice[2][1][1]", 2)
        .register("slice[2][1][2]", 2)
        .register("slice[2][1][3]", 2)
        .register("slice[2][1].length", 3)
        .register("slice[2][2][0]", 4)
        .register("slice[2][2][1]", 4)
        .register("slice[2][2][2]", 4)
        .register("slice[2][2][3]", 4)
        .register("slice[2][2][4]", 4)
        .register("slice[2][2].length", 5)
        .register("slice[2].length", 1)

        .register("slice.length", 6)

        .register("a", 7)
        .register("b", 8)
        .register("unused", 9),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "array_with_slices/main.rs::bar:::EXIT",
        ))
        .register("slice[0][0][0]", 0)
        .register("slice[0][0][1]", 0)
        .register("slice[0][0][2]", 0)
        .register("slice[0][0].length", 1)
        .register("slice[0][1][0]", 2)
        .register("slice[0][1][1]", 2)
        .register("slice[0][1][2]", 2)
        .register("slice[0][1][3]", 2)
        .register("slice[0][1].length", 3)
        .register("slice[0][2][0]", 4)
        .register("slice[0][2][1]", 4)
        .register("slice[0][2][2]", 4)
        .register("slice[0][2][3]", 4)
        .register("slice[0][2][4]", 4)
        .register("slice[0][2].length", 5)
        .register("slice[0].length", 1) // note same set as above because of previous interactions

        .register("slice[1][0][0]", 0)
        .register("slice[1][0][1]", 0)
        .register("slice[1][0][2]", 0)
        .register("slice[1][0].length", 1)
        .register("slice[1][1][0]", 2)
        .register("slice[1][1][1]", 2)
        .register("slice[1][1][2]", 2)
        .register("slice[1][1][3]", 2)
        .register("slice[1][1].length", 3)
        .register("slice[1][2][0]", 4)
        .register("slice[1][2][1]", 4)
        .register("slice[1][2][2]", 4)
        .register("slice[1][2][3]", 4)
        .register("slice[1][2][4]", 4)
        .register("slice[1][2].length", 5)
        .register("slice[1].length", 1)

        .register("slice[2][0][0]", 0)
        .register("slice[2][0][1]", 0)
        .register("slice[2][0][2]", 0)
        .register("slice[2][0].length", 1)
        .register("slice[2][1][0]", 2)
        .register("slice[2][1][1]", 2)
        .register("slice[2][1][2]", 2)
        .register("slice[2][1][3]", 2)
        .register("slice[2][1].length", 3)
        .register("slice[2][2][0]", 4)
        .register("slice[2][2][1]", 4)
        .register("slice[2][2][2]", 4)
        .register("slice[2][2][3]", 4)
        .register("slice[2][2][4]", 4)
        .register("slice[2][2].length", 5)
        .register("slice[2].length", 1)

        .register("slice.length", 6)

        .register("a", 7)
        .register("b", 8)
        .register("unused", 9)

        .register("return", 10),
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("array_with_slices.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
