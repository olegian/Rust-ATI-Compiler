use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn generic_struct() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new("main::ENTER"));
    expected.register_site(ExpectedSite::new("main::EXIT"));

    expected.register_site(
        ExpectedSite::new("MyStruct::new::ENTER")
            .register("val", 0)
            .register("unused", 1)
    );
    expected.register_site(
        ExpectedSite::new("MyStruct::new::EXIT")
            .register("val", 0)
            .register("unused", 1)
            .register("RET.val", 0)
            .register("RET.unused", 1)
    );

    expected.register_site(
        ExpectedSite::new("MyStruct::foo::ENTER")
            .register("self.val", 0)
            .register("self.unused", 1)
            .register("val", 2)
    );
    expected.register_site(
        ExpectedSite::new("MyStruct::foo::EXIT")
            .register("self.val", 0)
            .register("self.unused", 1)
            .register("val", 2)
    );


    let executable = Path::new(file!()).parent().unwrap().join("generic_struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
