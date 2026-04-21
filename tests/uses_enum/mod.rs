use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn uses_enum() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("use_color:::ENTER")
            .register("c::Blue.0", 0)
            .register("scale", 1),
    );
    expected.register_site(
        ExpectedSite::new("use_color:::EXIT")
            .register("c::Blue.0", 0)
            .register("scale", 0)
            .register("RET", 0),
    );

    expected.register_site(
        ExpectedSite::new("use_point:::ENTER")
            .register("p::D1.x", 0)
            .register("p::D1.y", 1)
            .register("p::D2.a", 2)
            .register("p::D2.y", 3)
            .register("z", 4),
    );
    expected.register_site(
        ExpectedSite::new("use_point:::EXIT")
            .register("p::D1.x", 0)
            .register("p::D1.y", 1)
            .register("p::D2.a", 2)
            .register("p::D2.y", 1)
            .register("z", 1)
            .register("RET", 1),
    );

    let executable = Path::new(file!()).parent().unwrap().join("enum.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
