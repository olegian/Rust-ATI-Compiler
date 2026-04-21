use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn references() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("same_value:::ENTER")
            .register("a", 0)
            .register("b", 0)
            .register("c", 0)
    );
    expected.register_site(
        ExpectedSite::new("same_value:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("c", 0)
            .register("RET", 0)
    );

    expected.register_site(
        ExpectedSite::new("returns_ref:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("returns_ref:::EXIT")
            .register("a", 0)
            .register("b", 1)
            .register("RET", 0)
    );

    expected.register_site(
        ExpectedSite::new("returns_nested_ref:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("returns_nested_ref:::EXIT")
            .register("a", 0)
            .register("b", 1)
            .register("RET", 0)
    );

    expected.register_site(
        ExpectedSite::new("compares_ref_to_value:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("compares_ref_to_value:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("RET", 2)
    );

    expected.register_site(
        ExpectedSite::new("compares_ref_to_ref:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("compares_ref_to_ref:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("RET", 2)
    );

    expected.register_site(
        ExpectedSite::new("compares_ref_to_ref_mut:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("compares_ref_to_ref_mut:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("RET", 2)
    );

    expected.register_site(
        ExpectedSite::new("compares_ref_mut_to_ref_mut:::ENTER")
            .register("a", 0)
            .register("b", 1)
    );
    expected.register_site(
        ExpectedSite::new("compares_ref_mut_to_ref_mut:::EXIT")
            .register("a", 0)
            .register("b", 0)
            .register("RET", 2)
    );

    let executable = Path::new(file!()).parent().unwrap().join("references.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
