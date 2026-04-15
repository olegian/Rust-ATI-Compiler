use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn ranges() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));
    expected.register_site(
        ExpectedSite::new("copy:::ENTER")
            .register_array("from", vec![5], 0, vec![1])
            .register_array("to", vec![5], 2, vec![3])
            .register("unused", 4),
    );
    expected.register_site(
        ExpectedSite::new("copy:::EXIT")
            .register_array("from", vec![5], 0, vec![1])
            .register_array("to", vec![5], 0, vec![1])
            .register("unused", 4)
            // .register("RET", 9)
    );

    let executable = Path::new(file!()).parent().unwrap().join("ranges.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
