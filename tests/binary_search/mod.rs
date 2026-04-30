use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

// specifying this test just became really hard because of 
// allowing arrays to contain different ATs for elements.
#[ignore]
#[test]
fn binary_search() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "binary_search/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "binary_search/main.rs::main:::EXIT",
    )));
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "binary_search/main.rs::generic_bin_search:::ENTER",
        ))
        // .register_array("haystack", vec![50], 0, vec![1])
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
        .register("haystack[20]", 21)
        .register("haystack[21]", 22)
        .register("haystack[22]", 23)
        .register("haystack[23]", 24)
        .register("haystack[24]", 25)
        .register("haystack[25]", 26)
        .register("haystack[26]", 27)
        .register("haystack[27]", 28)
        .register("haystack[28]", 29)
        .register("haystack[29]", 30)
        .register("haystack[30]", 31)
        .register("haystack[31]", 32)
        .register("haystack[32]", 33)
        .register("haystack[33]", 34)
        .register("haystack[34]", 35)
        .register("haystack[35]", 36)
        .register("haystack[36]", 37)
        .register("haystack[37]", 38)
        .register("haystack[38]", 39)
        .register("haystack[39]", 40)
        .register("haystack[40]", 41)
        .register("haystack[41]", 42)
        .register("haystack[42]", 43)
        .register("haystack[43]", 44)
        .register("haystack[44]", 45)
        .register("haystack[45]", 46)
        .register("haystack[46]", 47)
        .register("haystack[47]", 48)
        .register("haystack[48]", 49)
        .register("haystack[49]", 50)

        .register("needle", 51)
        .register("lo", 52)
        .register("hi", 53),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "binary_search/main.rs::generic_bin_search:::EXIT",
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
        .register("haystack[20]", 21)
        .register("haystack[21]", 22)
        .register("haystack[22]", 23)
        .register("haystack[23]", 24)
        .register("haystack[24]", 25)
        .register("haystack[25]", 26)
        .register("haystack[26]", 27)
        .register("haystack[27]", 28)
        .register("haystack[28]", 29)
        .register("haystack[29]", 30)
        .register("haystack[30]", 31)
        .register("haystack[31]", 32)
        .register("haystack[32]", 33)
        .register("haystack[33]", 34)
        .register("haystack[34]", 35)
        .register("haystack[35]", 36)
        .register("haystack[36]", 37)
        .register("haystack[37]", 38)
        .register("haystack[38]", 39)
        .register("haystack[39]", 40)
        .register("haystack[40]", 41)
        .register("haystack[41]", 42)
        .register("haystack[42]", 43)
        .register("haystack[43]", 44)
        .register("haystack[44]", 45)
        .register("haystack[45]", 46)
        .register("haystack[46]", 47)
        .register("haystack[47]", 48)
        .register("haystack[48]", 49)
        .register("haystack[49]", 50)

        .register("needle", 0)
        .register("lo", 1)
        .register("hi", 1), // FIXME: support Option variants
                            // .register("return", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "binary_search/main.rs::concrete_bin_search:::ENTER",
        ))
        // .register_array("haystack", vec![25], 0, vec![1])
        .register("needle", 2)
        .register("lo", 3) // due to recursion lo and hi start in same AT
        .register("hi", 3),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "binary_search/main.rs::concrete_bin_search:::EXIT",
        ))
        // .register_array("haystack", vec![25], 0, vec![1])
        .register("needle", 0)
        .register("lo", 1)
        .register("hi", 1), // FIXME: support Option variants
                            // .register("return", 0),
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("binary_search.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
