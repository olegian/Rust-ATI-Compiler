use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn lis() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("lis:::ENTER")
            .register_array("haystack", vec![20], 0, vec![1])
    );
    expected.register_site(
        ExpectedSite::new("lis:::EXIT")
            .register_array("haystack", vec![20], 0, vec![1])
            .register("RET.0", 1)
            .register_array("RET.1", vec![6], 0, vec![1])
    );

    let executable = Path::new(file!()).parent().unwrap().join("lis.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
