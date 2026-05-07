//! Defines functions which can parse and include all items defined within the runtime libary
//! (all files within `src/ati/*.rs`, except the mod.rs file).
//!
//! The runtime libary is only injected into the crate root file (main.rs / lib.rs). All other
//! dependancy files should import the crate root (via a `use crate::*;`) statement to make all
//! types available. It's important to only inject the library once, specifically so the
//! `ATI_ANALYSIS` global, which holds value-interaction state, is only defined once in the
//! compiled binary.

use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use crate::callbacks::parsing;

// FIXME: should I make this an actual module import?? might lead to slightly cleaner code?

/// Adds the rust Items defined in `file` to the input `krate`.
///
/// `file` must be a path to a .rs file containing required struct defs,
/// enum defs, and thier associated impl blocks.
/// This function also handles use statements, removing them entirely. This means
/// the runtime libary files must only use fully qualified paths to standard libary types.
pub fn define_types_from_file(file: &std::path::Path, psess: &ParseSess, krate: &mut ast::Crate) {
    let code: String = std::fs::read_to_string(file).unwrap();

    let items = parsing::parse_items(psess, code, Some(file));

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
    let attr = parsing::parse_single_unstable_compiler_attribute(psess, attr.into(), None);
    krate.attrs.push(attr);
}
