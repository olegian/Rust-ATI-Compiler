use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn match_expr() {
    let mut expected = ExpectedOutput::new();

    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "match_expr/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "match_expr/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::foo:::ENTER",
        ))
        .register("x::V2.0", 0)
        .register("y", 1),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("match_expr/main.rs::foo:::EXIT"))
            .register("x::V2.0", 0)
            .register("y", 0)
            .register("return", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::bar:::ENTER",
        ))
        .register("x::V4.0[0]", 0)
        .register("x::V4.0[1]", 1)
        .register("x::V4.0[2]", 2)
        .register("x::V4.0.length", 3)
        .register("y", 4),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("match_expr/main.rs::bar:::EXIT"))
            .register("x::V4.0[0]", 0)
            .register("x::V4.0[1]", 1)
            .register("x::V4.0[2]", 2)
            .register("x::V4.0.length", 3)
            .register("y", 3)
            .register("return", 3),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::baz:::ENTER",
        ))
        .register("x::V3.0.x", 0)
        .register("x::V3.0.y", 1)
        .register("y", 3),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root("match_expr/main.rs::baz:::EXIT"))
            .register("y", 3)
            .register("return", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::quux:::ENTER",
        ))
        .register("x::V3.0.x", 0)
        .register("x::V3.0.y", 1)
        .register("y", 3),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::quux:::EXIT",
        ))
        .register("y", 3)
        .register("return", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::primitive:::ENTER",
        ))
        .register("x", 0)
        .register("y", 1),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::primitive:::EXIT",
        ))
        .register("x", 0)
        .register("y", 1)
        .register("return", 1),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::primitive_mut:::ENTER",
        ))
        .register("x", 0)
        .register("y", 1),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::primitive_mut:::EXIT",
        ))
        .register("x", 0)
        .register("y", 0)
        .register("return", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::untracked_primitive:::ENTER",
        ))
        .register("a", 0)
        .register("b", 1)
        .register("c", 2),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::untracked_primitive:::EXIT",
        ))
        .register("a", 0)
        .register("b", 1)
        .register("c", 2)
        .register("return", 1),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::destructure_to_value:::ENTER",
        ))
        .register("x::V2.0", 0)
        .register("y", 1)
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "match_expr/main.rs::destructure_to_value:::EXIT",
        ))
        .register("x::V2.0", 0)
        .register("y", 1)
        .register("return", 1)
    );

    let executable = Path::new(file!()).parent().unwrap().join("match_expr.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
