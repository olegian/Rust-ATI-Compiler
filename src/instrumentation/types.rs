use rustc_ast as ast;
use rustc_session::parse::ParseSess;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str, parser::ForceCollect};
use rustc_span::FileName;

/// `file` must be a path to a .rs file containing required structs,
/// enums, and thier associated impl blocks, to be added to the target
/// program. This is essentially a module import.
// TODO: should I make this an actual module import?? might lead to slightly cleaner code
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
            Ok(None) => break, // no more items
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse item from analysis.rs");
            }
        }

        // TODO: is this necessary? not sure if the above Ok(None) catches all EOFs
        // if parser.token.kind == rustc_ast::token::TokenKind::Eof {
        //     break;
        // }
    }

    let items = imports.into_iter().chain(items.into_iter());
    for (i, item) in items.enumerate() {
        krate.items.insert(i, item);
    }
}
