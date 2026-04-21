/* Defines a function which reads a file, parses it, and adds every single
 * type definition into the crate being instrumented. This is used to effectively
 * import a file at compile time, providing access to the necessary definitions to
 * perform ATI.
*/
use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use crate::common;

// FIXME: should I make this an actual module import?? might lead to slightly cleaner code?

/// `file` must be a path to a .rs file containing required struct defs,
/// enum defs, and thier associated impl blocks, to be added to the target
/// program. Also handles use statements (by removing them)!
pub fn define_types_from_file(file: &std::path::Path, psess: &ParseSess, krate: &mut ast::Crate) {
    let code: String = std::fs::read_to_string(file).unwrap();

    let items = common::parse_items(psess, code, Some(file));

    // actually add the stuff we've collected to the crate
    // removing any use statements as everything
    // is now going to be in the same file.
    for (i, item) in items
        .into_iter()
        .filter(|item| !matches!(item.kind, ast::ItemKind::Use(_)))
        .enumerate()
    {
        krate.items.insert(i, item);
    }
}

/// Adds a crate attribute tag (#![feature(...)]) to the crate.
pub fn add_crate_attribute(attr: &str, psess: &ParseSess, krate: &mut ast::Crate) {
    let attr = common::parse_single_unstable_compiler_attribute(psess, attr.into(), None);
    krate.attrs.push(attr);
}

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
