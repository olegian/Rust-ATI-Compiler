use std::collections::HashMap;

use rustc_ast as ast;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str, parser::ForceCollect};
use rustc_session::parse::ParseSess;
use rustc_span::FileName;

use crate::instrumentation::common::FnInfo;


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
