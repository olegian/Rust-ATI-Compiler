use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn type_hints() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("struct_hints:::ENTER")
            .register("a", 0)
            .register("b", 1)
            .register("unused", 2),
    );
    expected.register_site(
        ExpectedSite::new("struct_hints:::EXIT")
            .register("a", 0)
            .register("b", 1)
            .register("unused", 2)
            .register("RET.x", 0)
            .register("RET.y", 1),
    );

    expected.register_site(
        ExpectedSite::new("primitive_hints:::ENTER")
            .register("a", 0)
            .register("b", 1)
            .register("unused", 2),
    );
    expected.register_site(
        ExpectedSite::new("primitive_hints:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("unused", 2)
            .register("RET", 0),
    );

    expected.register_site(
        ExpectedSite::new("turbofish_hints:::ENTER")
            .register("a", 0)
            .register("unused", 2),
    );
    expected.register_site(
        ExpectedSite::new("turbofish_hints:::EXIT")
            .register("a", 0)
            .register("unused", 2)
            .register("RET", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("type_hints.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
