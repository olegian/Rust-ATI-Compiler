use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn unary_operators() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("negation:::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2)
    );
    expected.register_site(
        ExpectedSite::new("negation:::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 1)
            .register("RET", 0)
    );

    expected.register_site(
        ExpectedSite::new("boolean_not:::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2)
    );
    expected.register_site(
        ExpectedSite::new("boolean_not:::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("z", 0)
            .register("RET", 0)
    );

    expected.register_site(
        ExpectedSite::new("dereference:::ENTER")
            .register("x", 0)
            .register("y", 1)
            .register("z", 2)
    );
    expected.register_site(
        ExpectedSite::new("dereference:::EXIT")
            .register("x", 0)
            .register("y", 1)
            .register("z", 1)
    );


    let executable = Path::new(file!()).parent().unwrap().join("unary_ops.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
