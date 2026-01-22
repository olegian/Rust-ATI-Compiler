use std::collections::HashMap;

use rustc_ast as ast;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str, parser::ForceCollect};
use rustc_session::parse::ParseSess;
use rustc_span::FileName;

use crate::instrumentation::common::FnInfo;


/// Uses previously discovered modified function information to define new "stub functions"
/// which dynamically create *::ENTER and *::EXIT sites, and then invoke the "unstubbed"
/// functions. Note that function stubs retain the original name of the function,
/// so that any uses of that function automatically invoke our stub instead.
pub fn create_stubs<'a>(
    krate: &mut ast::Crate,
    psess: &ParseSess,
    modified_functions: &'a HashMap<String, FnInfo>,
) {
    for (name, fn_info) in modified_functions.iter() {
        let code = fn_info.create_fn_stub(name);
        let mut parser = new_parser_from_source_str(
            psess,
            FileName::anon_source_code(&code),
            code,
            StripTokens::Nothing,
        )
        .unwrap();

        loop {
            // add all function stubs to crate
            match parser.parse_item(ForceCollect::No) {
                Ok(Some(item)) => {
                    krate.items.insert(0, item);
                }
                Ok(None) => {
                    break;
                }
                Err(diag) => {
                    diag.emit();
                    panic!("Failed to parse item from stubs.rs");
                }
            }
        }
    }
}
