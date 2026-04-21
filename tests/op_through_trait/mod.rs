use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn op_through_trait() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    // FIXME: make specifying tuples easier
    expected.register_site(
        ExpectedSite::new("foo:::ENTER")
            .register("a.0", 0)
            .register("a.1", 1)
            .register("a.2", 2)
            .register("b.0", 3)
            .register("b.1", 4)
            .register("b.2", 5)
            .register("c.0", 6)
            .register("c.1", 7)
            .register("c.2", 8)
    );
    expected.register_site(
        ExpectedSite::new("foo:::EXIT")
            .register("a.0", 0)
            .register("a.1", 1)
            .register("a.2", 2)
            .register("b.0", 0)
            .register("b.1", 1)
            .register("b.2", 2)
            .register("c.0", 6)
            .register("c.1", 7)
            .register("c.2", 8)
            .register("RET.0", 0)
            .register("RET.1", 1)
            .register("RET.2", 2)
    );

    expected.register_site(
        ExpectedSite::new("bar:::ENTER")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c", 2)
            .register("b.a", 3)
            .register("b.b", 4)
            .register("b.c", 5)
            .register("c.a", 6)
            .register("c.b", 7)
            .register("c.c", 8)
    );
    expected.register_site(
        ExpectedSite::new("bar:::EXIT")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("a.c", 2)
            .register("b.a", 0)
            .register("b.b", 1)
            .register("b.c", 2)
            .register("c.a", 6)
            .register("c.b", 7)
            .register("c.c", 8)
            .register("RET.a", 0)
            .register("RET.b", 1)
            .register("RET.c", 2)
    );

    expected.register_site(
        ExpectedSite::new("baz:::ENTER")
            .register("a.a", 0)
            .register("a.b.0", 1)
            .register("a.b.1", 2)
            .register("a.b.2", 3)
            .register("b.a", 4)
            .register("b.b.0", 5)
            .register("b.b.1", 6)
            .register("b.b.2", 7)
            .register("c.a", 8)
            .register("c.b.0", 9)
            .register("c.b.1", 10)
            .register("c.b.2", 11)
    );
    expected.register_site(
        ExpectedSite::new("baz:::EXIT")
            .register("a.a", 0)
            .register("a.b.0", 1)
            .register("a.b.1", 2)
            .register("a.b.2", 3)
            .register("b.a", 0)
            .register("b.b.0", 1)
            .register("b.b.1", 2)
            .register("b.b.2", 3)
            .register("c.a", 8)
            .register("c.b.0", 9)
            .register("c.b.1", 10)
            .register("c.b.2", 11)
            .register("RET.a", 0)
            .register("RET.b.0", 1)
            .register("RET.b.1", 2)
            .register("RET.b.2", 3)
    );

    expected.register_site(
        ExpectedSite::new("NewTuple.add:::ENTER")
            .register("self.0", 0)
            .register("self.1", 1)
            .register("self.2", 2)
            .register("rhs.0", 3)
            .register("rhs.1", 4)
            .register("rhs.2", 5)
    );
    expected.register_site(
        ExpectedSite::new("NewTuple.add:::EXIT")
            .register("self.0", 0)
            .register("self.1", 1)
            .register("self.2", 2)
            .register("rhs.0", 0)
            .register("rhs.1", 1)
            .register("rhs.2", 2)
            .register("RET.0", 0)
            .register("RET.1", 1)
            .register("RET.2", 2)
    );

    expected.register_site(
        ExpectedSite::new("NewStruct.mul:::ENTER")
            .register("self.a", 0)
            .register("self.b", 1)
            .register("self.c", 2)
            .register("rhs.a", 3)
            .register("rhs.b", 4)
            .register("rhs.c", 5)
    );
    expected.register_site(
        ExpectedSite::new("NewStruct.mul:::EXIT")
            .register("self.a", 0)
            .register("self.b", 1)
            .register("self.c", 2)
            .register("rhs.a", 0)
            .register("rhs.b", 1)
            .register("rhs.c", 2)
            .register("RET.a", 0)
            .register("RET.b", 1)
            .register("RET.c", 2)
    );

    expected.register_site(
        ExpectedSite::new("Nested.add:::ENTER")
            .register("self.a", 0)
            .register("self.b.0", 1)
            .register("self.b.1", 2)
            .register("self.b.2", 3)
            .register("rhs.a", 4)
            .register("rhs.b.0", 5)
            .register("rhs.b.1", 6)
            .register("rhs.b.2", 7)
    );
    expected.register_site(
        ExpectedSite::new("Nested.add:::EXIT")
            .register("self.a", 0)
            .register("self.b.0", 1)
            .register("self.b.1", 2)
            .register("self.b.2", 3)
            .register("rhs.a", 0)
            .register("rhs.b.0", 1)
            .register("rhs.b.1", 2)
            .register("rhs.b.2", 3)
            .register("RET.a", 0)
            .register("RET.b.0", 1)
            .register("RET.b.1", 2)
            .register("RET.b.2", 3)
    );

    let executable = Path::new(file!()).parent().unwrap().join("assign_tuples.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
