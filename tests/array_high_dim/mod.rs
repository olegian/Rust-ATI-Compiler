use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

// TODO: FINISH THIS SPEC, REQUIRES HIGHER-DIM ARRAY PROCESSING
#[test]
fn array_2d() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    let executable = Path::new(file!()).parent().unwrap().join("array_high_dim.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
