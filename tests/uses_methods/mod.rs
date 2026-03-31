use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn uses_methods() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));

    expected.register_site(
        ExpectedSite::new("Counter::new::ENTER")
            .register("initial", 0)
            .register("unused_param", 1),
    );
    expected.register_site(
        ExpectedSite::new("Counter::new::EXIT")
            .register("initial", 0)
            .register("unused_param", 1)
            .register("RET.val", 0)
            .register("RET.unused", 2),
    );

    expected.register_site(
        ExpectedSite::new("Counter::add_1::ENTER")
            .register("self.val", 0)
            .register("self.unused", 3)
            .register("amount", 1)
            .register("unused_param", 2),
    );
    expected.register_site(
        ExpectedSite::new("Counter::add_1::EXIT")
            .register("self.val", 0)
            .register("self.unused", 3)
            .register("amount", 0)
            .register("unused_param", 2)
            .register("RET", 0),
    );

    expected.register_site(
        ExpectedSite::new("Counter::add_2::ENTER")
            .register("self.val", 0)
            .register("self.unused", 3)
            .register("amount", 1)
            .register("unused_param", 2),
    );
    expected.register_site(
        ExpectedSite::new("Counter::add_2::EXIT")
            .register("self.val", 0)
            .register("self.unused", 3)
            .register("amount", 0)
            .register("unused_param", 2),
    );

    expected.register_site(
        ExpectedSite::new("Counter::add_3::ENTER")
            .register("self.val", 0)
            .register("self.unused", 1)
            .register("unused_param", 2),
    );
    expected.register_site(
        ExpectedSite::new("Counter::add_3::EXIT")
            .register("self.val", 0)
            .register("self.unused", 1)
            .register("unused_param", 2)
            .register("RET.val", 0)
            .register("RET.unused", 1)
    );

    let executable = Path::new(file!()).parent().unwrap().join("methods.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
