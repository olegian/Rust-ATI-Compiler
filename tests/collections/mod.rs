use std::{collections::HashMap, path::Path};

use crate::common::{compile_and_execute, verify};

#[test]
fn collections() {
    let mut expected = HashMap::new();
    expected.insert("main::ENTER", HashMap::new());
    expected.insert("main::EXIT",  HashMap::new());
    // TODO: finish adding expected,
    // but this test is difficult. Look at statements.rs TODOs

    let test_dir = Path::new(file!()).parent().unwrap().to_str().unwrap();
    let ati_output = compile_and_execute(test_dir, "collections");
    verify(&ati_output, &expected);
}
