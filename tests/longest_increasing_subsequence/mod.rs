use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn lis() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "longest_increasing_subsequence/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "longest_increasing_subsequence/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "longest_increasing_subsequence/main.rs::lis:::ENTER",
        ))
        .register("haystack.length", 0)
        .register("haystack[0]", 1)
        .register("haystack[1]", 2)
        .register("haystack[2]", 3)
        .register("haystack[3]", 4)
        .register("haystack[4]", 5)
        .register("haystack[5]", 6)
        .register("haystack[6]", 7)
        .register("haystack[7]", 8)
        .register("haystack[8]", 9)
        .register("haystack[9]", 10)
        .register("haystack[10]", 11)
        .register("haystack[11]", 12)
        .register("haystack[12]", 13)
        .register("haystack[13]", 14)
        .register("haystack[14]", 15)
        .register("haystack[15]", 16)
        .register("haystack[16]", 17)
        .register("haystack[17]", 18)
        .register("haystack[18]", 19)
        .register("haystack[19]", 20)
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "longest_increasing_subsequence/main.rs::lis:::EXIT",
        ))
        .register("haystack.length", 0)
        .register("haystack[0]", 1)
        .register("haystack[1]", 1)
        .register("haystack[2]", 1)
        .register("haystack[3]", 1)
        .register("haystack[4]", 1)
        .register("haystack[5]", 1)
        .register("haystack[6]", 1)
        .register("haystack[7]", 1)
        .register("haystack[8]", 1)
        .register("haystack[9]", 1)
        .register("haystack[10]", 1)
        .register("haystack[11]", 1)
        .register("haystack[12]", 1)
        .register("haystack[13]", 1)
        .register("haystack[14]", 1)
        .register("haystack[15]", 1)
        .register("haystack[16]", 1)
        .register("haystack[17]", 1)
        .register("haystack[18]", 1)
        .register("haystack[19]", 1)

        .register("return.0", 0)
        .register("return.1.length", 0)
        .register("return.1[0]", 1)
        .register("return.1[1]", 1)
        .register("return.1[2]", 1)
        .register("return.1[3]", 1)
        .register("return.1[4]", 1)
        .register("return.1[5]", 1)
    );

    let executable = Path::new(file!()).parent().unwrap().join("lis.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
