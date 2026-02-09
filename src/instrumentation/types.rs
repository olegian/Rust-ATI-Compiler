// MDE: Documentation of the function should be written at the function definition, not far away from it at the top of the file.
// MDE: Define "effectively import".  Maybe you mean, "The compiler treats the file as if the source code contained: ...".
/* Defines a function which reads a file, parses it, and adds every single
 * type definition into the crate being instrumented. This is used to effectively
 * import a file at compile time, providing access to the necessary definitions to
 * perform ATI.
*/
use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str, parser::ForceCollect};
use rustc_span::FileName;

// FIXME: should I make this an actual module import?? might lead to slightly cleaner code?

// MDE: What is a "required" struct def?
/// `file` must be a path to a .rs file containing required struct defs,
/// enum defs, and thier associated impl blocks, to be added to the target
/// program. Also handles use statements!
pub fn define_types_from_file(file: &std::path::Path, psess: &ParseSess, krate: &mut ast::Crate) {
    let code: String = std::fs::read_to_string(file).unwrap();

    let mut parser = new_parser_from_source_str(
        psess,
        FileName::anon_source_code(&code),
        code.to_string(),
        StripTokens::Nothing,
    )
    .unwrap();

    let mut imports = Vec::new();
    let mut items = Vec::new();

    loop {
        match parser.parse_item(ForceCollect::No) {
            Ok(Some(item)) => {
                if matches!(item.kind, ast::ItemKind::Use(_)) {
                    imports.push(item);
                } else {
                    items.push(item);
                }
            }
            Ok(None) => break, // no more items
            Err(diag) => {
                diag.emit();
                // MDE: I realize that rustc uses "item" to mean "AST node", but use of it here is confusing to users.  Also, an AST node is post-parsing, so "item" is misleading.  What cannot be parsed is a top-level construct, such as a function or variable definition.
                panic!("Failed to parse item from analysis.rs");
            }
        }
    }

    // actually add the stuff we've collected to the crate
    // placing imports before all other items
    let items = imports.into_iter().chain(items.into_iter());
    for (i, item) in items.enumerate() {
        krate.items.insert(i, item);
    }
}
