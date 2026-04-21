use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn assign_compound() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("assign_tuple:::ENTER")
            .register("a.0", 0)
            .register("a.1", 1)
            .register("b.0", 2)
            .register("b.1", 3)
    );
    expected.register_site(
        ExpectedSite::new("assign_tuple:::EXIT")
            .register("a.0", 2)
            .register("a.1", 3)
            .register("b.0", 2)
            .register("b.1", 3)
    );

    expected.register_site(
        ExpectedSite::new("assign_struct:::ENTER")
            .register("a.a", 0)
            .register("a.b", 1)
            .register("b.a", 2)
            .register("b.b", 3)
    );
    expected.register_site(
        ExpectedSite::new("assign_struct:::EXIT")
            .register("a.a", 2)
            .register("a.b", 3)
            .register("b.a", 2)
            .register("b.b", 3)
    );

    expected.register_site(
        ExpectedSite::new("assign_enum:::ENTER")
            .register("a::StructVariant.a", 0)
            .register("a::StructVariant.b", 1)
            .register("a::TupleVariant.0", 2)
            .register("a::TupleVariant.1", 3)

            .register("b::StructVariant.a", 4)
            .register("b::StructVariant.b", 5)
            .register("b::TupleVariant.0", 6)
            .register("b::TupleVariant.1", 7)
    );
    expected.register_site(
        ExpectedSite::new("assign_enum:::EXIT")
            .register("a::StructVariant.a", 4)
            .register("a::StructVariant.b", 5)
            .register("a::TupleVariant.0", 6)
            .register("a::TupleVariant.1", 7)

            .register("b::StructVariant.a", 4)
            .register("b::StructVariant.b", 5)
            .register("b::TupleVariant.0", 6)
            .register("b::TupleVariant.1", 7)
    );

    let executable = Path::new(file!()).parent().unwrap().join("assign_compound.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
