use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn assign_tuples() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    // FIXME: better specification for nested tuples -- similar problem to 
    // jagged arrays / slices of slices
    expected.register_site(
        ExpectedSite::new("assign_nested_tuple:::ENTER")
            .register("a.0", 0)
            .register("a.1.0", 1)
            .register("a.2", 2)
            .register("b.0", 3)
            .register("b.1.0", 4)
            .register("b.2", 5)
    );
    expected.register_site(
        ExpectedSite::new("assign_nested_tuple:::EXIT")
            .register("a.0", 0)
            .register("a.1.0", 1)
            .register("a.2", 2)
            .register("b.0", 0)
            .register("b.1.0", 1)
            .register("b.2", 2)
    );

    expected.register_site(
        ExpectedSite::new("mutate_tuple:::ENTER")
            .register("target.0", 0)
            .register("target.1", 1)
            .register("target.2", 2)
            .register("value.0", 3)
            .register("value.1", 4)
            .register("value.2", 5)
            .register("a", 6)
    );
    expected.register_site(
        ExpectedSite::new("mutate_tuple:::EXIT")
            .register("target.0", 0)
            .register("target.1", 1)
            .register("target.2", 2)
            .register("value.0", 0)
            .register("value.1", 1)
            .register("value.2", 2)
            .register("a", 1)
    );

    let executable = Path::new(file!()).parent().unwrap().join("assign_tuples.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
