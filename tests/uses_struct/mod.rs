use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn uses_struct() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("func:::ENTER")
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
        ExpectedSite::new("func:::EXIT")
            .register("x", 0)
            .register("y", 0)
            .register("s.x", 0)
            .register("s.y", 0)
            .register("RET", 0)
            .register("z", 1)
            .register("z2", 2)
            .register("s.z.x", 2)
            .register("s.z.y", 3),
    );

    expected.register_site(
        ExpectedSite::new("foo:::ENTER")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c.x", 2)
            .register("a.c.b", 3)
            .register("v", 4),
    );
    expected.register_site(
        ExpectedSite::new("foo:::EXIT")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c.x", 0)
            .register("a.c.b", 3)
            .register("v", 0)
            .register("RET.a", 0)
            .register("RET.b", 1)
            .register("RET.c.x", 0)
            .register("RET.c.b", 3),
    );

    expected.register_site(
        ExpectedSite::new("bar:::ENTER")
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
        ExpectedSite::new("bar:::EXIT")
            .register("a.0", 0)
            .register("a.1", 1)
            .register("a.2.x", 2)
            .register("a.2.b", 3)
            .register("b.a", 4)
            .register("b.b", 1)
            .register("b.c.x", 4)
            .register("b.c.b", 6)
            .register("RET.0", 0)
            .register("RET.1", 1)
            .register("RET.2.x", 2)
            .register("RET.2.b", 3),
    );

    expected.register_site(
        ExpectedSite::new("baz:::ENTER")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c", 2)
            .register("a.d", 3)
            .register("v", 4),
    );
    expected.register_site(
        ExpectedSite::new("baz:::EXIT")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c", 0) // a.c got reassigned, to a.a + a.d + 2*v. it should now be in set 0?
            .register("a.d", 0) // ^ this is a problem w.r.t. how site management is done, specifically when variables are bound to exit sites.
            .register("v", 0),  //  it's related to ownership, which is the most interesting part. Should formals that ar ecaptured by value
                                //  just outright be removed from the exit site? they don't exist after the function runs...
                                //  but also, in this exact example we capture a struct by value, that contains a reference to a value that
                                //  survives beyond the struct's lifetime...
    );

    expected.register_site(
        ExpectedSite::new("struct_defs.Inner.new:::ENTER")
            .register("x", 0)
            .register("b", 1),
    );
    expected.register_site(
        ExpectedSite::new("struct_defs.Inner.new:::EXIT")
            .register("x", 0)
            .register("b", 1)
            .register("RET.x", 0)
            .register("RET.b", 1),
    );

    expected.register_site(
        ExpectedSite::new("struct_defs.Inner.add_x:::ENTER")
            .register("self.x", 0)
            .register("self.b", 1)
            .register("x", 2),
    );
    expected.register_site(
        ExpectedSite::new("struct_defs.Inner.add_x:::EXIT")
            .register("self.x", 0)
            .register("self.b", 1)
            .register("x", 0),
    );

    let executable = Path::new(file!()).parent().unwrap().join("struct.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
