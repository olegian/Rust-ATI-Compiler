use std::path::Path;

use crate::common::{ExpectedOutput, ExpectedSite, compile_and_execute, delete, verify};

#[test]
fn array_with_slices() {
    let mut expected = ExpectedOutput::new();
    expected.register_site(ExpectedSite::new("main:::ENTER"));
    expected.register_site(ExpectedSite::new("main:::EXIT"));

    expected.register_site(
        ExpectedSite::new("foo:::ENTER")
            // FIXME: specifying jagged dimensional arrays is really difficult actually
            .register("arr[0][0]", 0)
            .register("arr[0][1]", 0)
            .register("arr[0][2]", 0)
            .register("arr[0]_LEN", 1)
            .register("arr[1][0]", 0)
            .register("arr[1][1]", 0)
            .register("arr[1][2]", 0)
            .register("arr[1][3]", 0)
            .register("arr[1]_LEN", 1)
            .register("arr[2][0]", 0)
            .register("arr[2][1]", 0)
            .register("arr[2][2]", 0)
            .register("arr[2][3]", 0)
            .register("arr[2][4]", 0)
            .register("arr[2]_LEN", 1)
            .register("arr_LEN", 2)
            .register("a", 3)
            .register("b", 4)
            .register("unused", 5),
    );
    expected.register_site(
        ExpectedSite::new("foo:::EXIT")
            .register("arr[0][0]", 0)
            .register("arr[0][1]", 0)
            .register("arr[0][2]", 0)
            .register("arr[0]_LEN", 1)
            .register("arr[1][0]", 0)
            .register("arr[1][1]", 0)
            .register("arr[1][2]", 0)
            .register("arr[1][3]", 0)
            .register("arr[1]_LEN", 1)
            .register("arr[2][0]", 0)
            .register("arr[2][1]", 0)
            .register("arr[2][2]", 0)
            .register("arr[2][3]", 0)
            .register("arr[2][4]", 0)
            .register("arr[2]_LEN", 1)
            .register("arr_LEN", 1)
            .register("a", 1)
            .register("b", 0)
            .register("unused", 5)
            .register("RET", 7),
    );

    expected.register_site(
        ExpectedSite::new("bar:::ENTER")
            // FIXME: specifying jagged dimensional arrays is really difficult actually
            .register("slice[0][0][0]", 0)
            .register("slice[0][0][1]", 0)
            .register("slice[0][0][2]", 0)
            .register("slice[0][0]_LEN", 1)
            .register("slice[0][1][0]", 0)
            .register("slice[0][1][1]", 0)
            .register("slice[0][1][2]", 0)
            .register("slice[0][1][3]", 0)
            .register("slice[0][1]_LEN", 1)
            .register("slice[0][2][0]", 0)
            .register("slice[0][2][1]", 0)
            .register("slice[0][2][2]", 0)
            .register("slice[0][2][3]", 0)
            .register("slice[0][2][4]", 0)
            .register("slice[0][2]_LEN", 1)
            .register("slice[0]_LEN", 1)  // note same set as above because of previous interactions

            .register("slice[1][0][0]", 0)
            .register("slice[1][0][1]", 0)
            .register("slice[1][0][2]", 0)
            .register("slice[1][0]_LEN", 1)
            .register("slice[1][1][0]", 0)
            .register("slice[1][1][1]", 0)
            .register("slice[1][1][2]", 0)
            .register("slice[1][1][3]", 0)
            .register("slice[1][1]_LEN", 1)
            .register("slice[1][2][0]", 0)
            .register("slice[1][2][1]", 0)
            .register("slice[1][2][2]", 0)
            .register("slice[1][2][3]", 0)
            .register("slice[1][2][4]", 0)
            .register("slice[1][2]_LEN", 1)
            .register("slice[1]_LEN", 1)

            .register("slice[2][0][0]", 0)
            .register("slice[2][0][1]", 0)
            .register("slice[2][0][2]", 0)
            .register("slice[2][0]_LEN", 1)
            .register("slice[2][1][0]", 0)
            .register("slice[2][1][1]", 0)
            .register("slice[2][1][2]", 0)
            .register("slice[2][1][3]", 0)
            .register("slice[2][1]_LEN", 1)
            .register("slice[2][2][0]", 0)
            .register("slice[2][2][1]", 0)
            .register("slice[2][2][2]", 0)
            .register("slice[2][2][3]", 0)
            .register("slice[2][2][4]", 0)
            .register("slice[2][2]_LEN", 1)
            .register("slice[2]_LEN", 1)
            .register("slice_LEN", 2)

            .register("a", 3)
            .register("b", 4)
            .register("unused", 5),
    );
    expected.register_site(
        ExpectedSite::new("bar:::EXIT")
            .register("slice[0][0][0]", 0)
            .register("slice[0][0][1]", 0)
            .register("slice[0][0][2]", 0)
            .register("slice[0][0]_LEN", 1)
            .register("slice[0][1][0]", 0)
            .register("slice[0][1][1]", 0)
            .register("slice[0][1][2]", 0)
            .register("slice[0][1][3]", 0)
            .register("slice[0][1]_LEN", 1)
            .register("slice[0][2][0]", 0)
            .register("slice[0][2][1]", 0)
            .register("slice[0][2][2]", 0)
            .register("slice[0][2][3]", 0)
            .register("slice[0][2][4]", 0)
            .register("slice[0][2]_LEN", 1)
            .register("slice[0]_LEN", 1)

            .register("slice[1][0][0]", 0)
            .register("slice[1][0][1]", 0)
            .register("slice[1][0][2]", 0)
            .register("slice[1][0]_LEN", 1)
            .register("slice[1][1][0]", 0)
            .register("slice[1][1][1]", 0)
            .register("slice[1][1][2]", 0)
            .register("slice[1][1][3]", 0)
            .register("slice[1][1]_LEN", 1)
            .register("slice[1][2][0]", 0)
            .register("slice[1][2][1]", 0)
            .register("slice[1][2][2]", 0)
            .register("slice[1][2][3]", 0)
            .register("slice[1][2][4]", 0)
            .register("slice[1][2]_LEN", 1)
            .register("slice[1]_LEN", 1)

            .register("slice[2][0][0]", 0)
            .register("slice[2][0][1]", 0)
            .register("slice[2][0][2]", 0)
            .register("slice[2][0]_LEN", 1)
            .register("slice[2][1][0]", 0)
            .register("slice[2][1][1]", 0)
            .register("slice[2][1][2]", 0)
            .register("slice[2][1][3]", 0)
            .register("slice[2][1]_LEN", 1)
            .register("slice[2][2][0]", 0)
            .register("slice[2][2][1]", 0)
            .register("slice[2][2][2]", 0)
            .register("slice[2][2][3]", 0)
            .register("slice[2][2][4]", 0)
            .register("slice[2][2]_LEN", 1)
            .register("slice[2]_LEN", 1)
            .register("slice_LEN", 1)

            .register("a", 1)
            .register("b", 0)
            .register("unused", 5)
            .register("RET", 6),
    );

    let executable = Path::new(file!())
        .parent()
        .unwrap()
        .join("array_with_slices.out");
    delete(&executable);

    let ati_output = compile_and_execute(&executable);
    verify(&ati_output, expected.inner());
}
