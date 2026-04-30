use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn generic_struct() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "generic_struct/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "generic_struct/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::MyStruct::<A, B>::new:::ENTER",
        ))
        .register("val", 0)
        .register("unused", 1),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::MyStruct::<A, B>::new:::EXIT",
        ))
        // .register("val", 0)  // dropped as genric is not ref or copy
        .register("unused", 1)
        .register("return.val", 0)
        .register("return.unused", 1),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::MyStruct::<A, B>::foo:::ENTER",
        ))
        .register("self.val", 0)
        .register("self.unused", 1)
        .register("val", 2),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::MyStruct::<A, B>::foo:::EXIT",
        ))
        .register("self.val", 0)
        .register("self.unused", 1)
        // .register("val", 2) // dropped as generic is not ref or copy
        .register("return", 1),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::foo:::ENTER",
        ))
        .register("a.val", 0)
        .register("a.unused", 1)
        .register("b", 2)
        .register("unused", 3),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "generic_struct/main.rs::foo:::EXIT",
        ))
        // .register("a.val", 0)  // dropped
        // .register("a.unused", 1)
        // .register("b", 2)
        .register("unused", 3)
        .register("return.val", 0)
        .register("return.unused", 1),
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("generic_struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
