use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn array() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "array/main.rs::main:::EXIT",
    )));
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("array/main.rs::foo:::ENTER"))
            // .register_array("arr", vec![3], 0, vec![1])
            .register("arr.length", 1)
            .register("arr[0]", 2)  // constructed via repeat op
            .register("arr[1]", 2)
            .register("arr[2]", 2)
            .register("x", 3)
            .register("y", 4)
            .register("unused", 5),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("array/main.rs::foo:::EXIT"))
            .register("arr.length", 1)
            .register("arr[0]", 0)
            .register("arr[1]", 3)
            .register("arr[2]", 3)
            .register("x", 0)
            .register("y", 0)
            .register("unused", 4)
            .register("return", 3),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("array/main.rs::bar:::ENTER"))
            .register("arr.length", 1)
            .register("arr[0]", 2)  // constructed as explcit array
            .register("arr[1]", 3)
            .register("arr[2]", 4)
            .register("unused", 5)
            .register("y", 6)
            .register("z", 7),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("array/main.rs::bar:::EXIT"))
            .register("arr.length", 1)
            .register("arr[0]", 2)
            .register("arr[1]", 3)
            .register("arr[2]", 4)
            .register("unused", 5)
            .register("y", 1)
            .register("z", 1)
            .register("return", 2),
    );

    let executable = Path::new(file!()).parent().unwrap().join("array.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
