
use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

// TODO: probably a good idea to make sites qualify the file name they are in too
#[test]
fn array() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));
    expected.register_site(
        ExpectedSite::new("foo::ENTER")
            .register_array("arr", 3, 0, 1)
            .register("x", 2)
            .register("y", 3)
            .register("unused", 4)
    );
    expected.register_site(
        ExpectedSite::new("foo::EXIT")
            .register_array("arr", 3, 0, 1)
            .register("x", 0)
            .register("y", 0)
            .register("unused", 4)
            .register("RET", 0)
    );
    expected.register_site(
        ExpectedSite::new("bar::ENTER")
            .register_array("arr", 3, 0, 1)
            .register("unused", 2)
            .register("y", 3)
            .register("z", 4),
    );
    expected.register_site(
        ExpectedSite::new("bar::EXIT")
            .register_array("arr", 3, 0, 1)
            .register("unused", 2)
            .register("y", 1)
            .register("z", 1)
            .register("RET", 0)
    );

    let executable = Path::new(file!()).parent().unwrap().join("multi_file.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
