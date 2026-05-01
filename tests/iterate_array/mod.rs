use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn iter_array() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "iterate_array/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "iterate_array/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter_mut:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter_mut:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return.length", 0)
        .register("return[0]", 1)
        .register("return[1]", 1)
        .register("return[2]", 1)
        .register("return[3]", 1)
        .register("return[4]", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_mut:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_mut:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_into_iter:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_into_iter:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter_slice:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::implicit_iter_slice:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );


    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_slice:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_slice:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );


    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_mut_slice:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::explicit_iter_mut_slice:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 1)
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::enumerate_iter:::ENTER",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 2)
        .register("arr[2]", 3)
        .register("arr[3]", 4)
        .register("arr[4]", 5)
        .register("val", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "iterate_array/main.rs::enumerate_iter:::EXIT",
        ))
        .register("arr.length", 0)
        .register("arr[0]", 1)
        .register("arr[1]", 1)
        .register("arr[2]", 1)
        .register("arr[3]", 1)
        .register("arr[4]", 1)
        .register("val", 1)
        .register("return", 0)
    );




    let executable = Path::new(file!()).parent().unwrap().join("array.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
