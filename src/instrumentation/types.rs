use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str, parser::ForceCollect};
use rustc_span::FileName;

/// `file` must be a path to a .rs file containing required structs,
/// enums, and thier associated impl blocks, to be added to the target
/// program. This is essentially a module import.
// TODO: should I make this an actual module import?? might lead to slightly cleaner code
pub fn get_types_in(
    psess: &ParseSess,
    file: &std::path::Path,
) -> impl Iterator<Item = Box<ast::Item>> {
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

    // i assume here that the file only has `use`, `struct`, and `impl` statements
    // but `enums` and other top-level definitions should work?
    loop {
        match parser.parse_item(ForceCollect::No) {
            Ok(Some(item)) => {
                if matches!(item.kind, ast::ItemKind::Use(_)) {
                    imports.push(item);
                } else {
                    items.push(item);
                }
            }
            Ok(None) => break, // No more items
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse item from analysis.rs");
            }
        }

        // TODO: is this necessary?
        // Check if we've reached EOF
        if parser.token.kind == rustc_ast::token::TokenKind::Eof {
            break;
        }
    }

    imports.into_iter().chain(items.into_iter())
}
