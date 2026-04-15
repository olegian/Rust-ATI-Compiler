use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn binary_operators() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    register_arith_sites_for("add", &mut expected);
    register_arith_sites_for("sub", &mut expected);
    register_arith_sites_for("mul", &mut expected);
    register_arith_sites_for("div", &mut expected);
    register_arith_sites_for("rem", &mut expected);
    register_arith_sites_for("and", &mut expected);
    register_arith_sites_for("or", &mut expected);

    register_bitwise_sites_for("bit_xor", &mut expected);
    register_bitwise_sites_for("bit_and", &mut expected);
    register_bitwise_sites_for("bit_or", &mut expected);
    register_bitwise_sites_for("shl", &mut expected);
    register_bitwise_sites_for("shr", &mut expected);

    register_comp_sites_for("eq", &mut expected);
    register_comp_sites_for("lt", &mut expected);
    register_comp_sites_for("le", &mut expected);
    register_comp_sites_for("ne", &mut expected);
    register_comp_sites_for("ge", &mut expected);
    register_comp_sites_for("gt", &mut expected);

    let executable = Path::new(file!()).parent().unwrap().join("bin_ops.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}

fn register_arith_sites_for(op_name: &str, expected: &mut ExpectedOutput) {
    expected.register_site(ExpectedSite::new(&format!("{op_name}:::ENTER"))
        .register("x", 0)
        .register("y", 1)
        .register("z", 2));

    expected.register_site(ExpectedSite::new(&format!("{op_name}:::EXIT"))
        .register("x", 0)
        .register("y", 0)
        .register("z", 1)
        .register("RET", 0));
}

fn register_bitwise_sites_for(op_name: &str, expected: &mut ExpectedOutput) {
    expected.register_site(ExpectedSite::new(&format!("{op_name}:::ENTER"))
        .register("x", 0)
        .register("y", 1)
        .register("z", 2));

    expected.register_site(ExpectedSite::new(&format!("{op_name}:::EXIT"))
        .register("x", 0)
        .register("y", 1)
        .register("z", 2)
        .register("RET", 3));
}

fn register_comp_sites_for(op_name: &str, expected: &mut ExpectedOutput) {
    expected.register_site(ExpectedSite::new(&format!("{op_name}:::ENTER"))
        .register("x", 0)
        .register("y", 1)
        .register("z", 2));

    expected.register_site(ExpectedSite::new(&format!("{op_name}:::EXIT"))
        .register("x", 0)
        .register("y", 0)
        .register("z", 1)
        .register("RET", 2));
}

