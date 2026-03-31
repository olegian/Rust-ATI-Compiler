/* Creates function stubs for each tracked function that was discovered.
 * Each stub sets up enter and exit sites before invoking the actual function.
 * Any formals are registered to both enter and exit sites, the return value
 * is also registered to the exit site, under the name "RET".
*/
use rustc_ast as ast;

use rustc_session::parse::ParseSess;

use crate::common;

/// Gives access to ATI types to all files being compiled
pub fn import_root_crate(krate: &mut ast::Crate, psess: &ParseSess) {
    let code = r#"
        use crate::*;
    "#;

    let items = common::parse_items(psess, code.into(), None);
    for item in items {
        krate.items.insert(0, item);
    }
}
