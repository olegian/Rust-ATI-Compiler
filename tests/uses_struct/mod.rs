use std::path::Path;

use crate::common::{
    ExpectedOutput, ExpectedSite, compile_and_execute, delete, prefix_with_path_from_root, verify,
};

#[test]
fn uses_struct() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "uses_struct/main.rs::main:::ENTER",
    )));
    expected.register_site(ExpectedSite::new(prefix_with_path_from_root(
        "uses_struct/main.rs::main:::EXIT",
    )));

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::func:::ENTER",
        ))
        .register("x", 0)
        .register("y", 1)
        .register("z", 2)
        .register("z2", 7)
        .register("s.x", 3)
        .register("s.y", 4)
        .register("s.z.x", 5)
        .register("s.z.y", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::func:::EXIT",
        ))
        // s is captured by value
        // .register("s.x", 0)
        // .register("s.y", 0)
        // .register("s.z.x", 2)
        // .register("s.z.y", 3),
        .register("x", 0)
        .register("y", 0)
        .register("return", 0)
        .register("z", 1)
        .register("z2", 2),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::foo:::ENTER",
        ))
        .register("a.a", 0)
        .register("a.b", 1)
        .register("a.c.x", 2)
        .register("a.c.b", 3)
        .register("v", 4),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::foo:::EXIT",
        ))
        // a is captured by value
        // .register("a.a", 0)
        // .register("a.b", 1)
        // .register("a.c.x", 0)
        // .register("a.c.b", 3)
        .register("v", 0)
        .register("return.a", 0)
        .register("return.b", 1)
        .register("return.c.x", 0)
        .register("return.c.b", 3),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::bar:::ENTER",
        ))
        .register("a.0", 0)
        .register("a.1", 1)
        .register("a.2.x", 2)
        .register("a.2.b", 3)
        .register("b.a", 4)
        .register("b.b", 5)
        .register("b.c.x", 4)
        .register("b.c.b", 6),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::bar:::EXIT",
        ))
        // a and b are captured by value
        // .register("a.0", 0)
        // .register("a.1", 1)
        // .register("a.2.x", 2)
        // .register("a.2.b", 3)
        // .register("b.a", 4)
        // .register("b.b", 1)
        // .register("b.c.x", 4)
        // .register("b.c.b", 6)
        .register("return.0", 0)
        .register("return.1", 1)
        .register("return.2.x", 2)
        .register("return.2.b", 3),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::baz:::ENTER",
        ))
        .register("a.a", 0)
        .register("a.b", 1)
        .register("a.c", 2)
        .register("a.d", 3)
        .register("v", 4),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/main.rs::baz:::EXIT",
        ))
        // a was captured by value
        // .register("a.a", 0)
        // .register("a.b", 1)
        // .register("a.c", 0) 
        // .register("a.d", 0) 
        .register("v", 0),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/struct_defs.rs::struct_defs::Inner::new:::ENTER",
        ))
        .register("x", 0)
        .register("b", 1),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/struct_defs.rs::struct_defs::Inner::new:::EXIT",
        ))
        .register("x", 0)
        .register("b", 1)
        .register("return.x", 0)
        .register("return.b", 1),
    );

    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/struct_defs.rs::struct_defs::Inner::add_x:::ENTER",
        ))
        .register("self.x", 0)
        .register("self.b", 1)
        .register("x", 2),
    );
    expected.register_site(
        ExpectedSite::new(prefix_with_path_from_root(
            "uses_struct/struct_defs.rs::struct_defs::Inner::add_x:::EXIT",
        ))
        .register("self.x", 0)
        .register("self.b", 1)
        .register("x", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
