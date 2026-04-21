use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn binary_search() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));
    expected.register_site(
        ExpectedSite::new("generic_bin_search:::ENTER")
            .register_array("haystack", vec![50], 0, vec![1])
            .register("needle", 2)
            .register("lo", 3)
            .register("hi", 4)
    );
    expected.register_site(
        ExpectedSite::new("generic_bin_search:::EXIT")
            .register_array("haystack", vec![50], 0, vec![1])
            .register("needle", 0)
            .register("lo", 1)
            .register("hi", 1)
            // FIXME: support Option variants
            // .register("RET", 0),
    );

    expected.register_site(
        ExpectedSite::new("concrete_bin_search:::ENTER")
            .register_array("haystack", vec![25], 0, vec![1])
            .register("needle", 2)
            .register("lo", 3)
            .register("hi", 4)
    );
    expected.register_site(
        ExpectedSite::new("concrete_bin_search:::EXIT")
            .register_array("haystack", vec![25], 0, vec![1])
            .register("needle", 0)
            .register("lo", 1)
            .register("hi", 1)
            // FIXME: support Option variants
            // .register("RET", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("binary_search.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
