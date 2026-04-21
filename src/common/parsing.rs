/* Provides helper functions that are used throughout this entire project.
 * Namely, this includes determining the set of types that are considered
 * able to be tagged, as well as moving between different representations of types.
*/
use rustc_ast::{self as ast};
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_parse::parser::ForceCollect;
use rustc_session::parse::ParseSess;
use rustc_span::{FileName, RealFileName};

use std::path::Path;

fn create_parser<'a>(
    psess: &'a ParseSess,
    contents: String,
    file_path: Option<&Path>,
) -> rustc_parse::parser::Parser<'a> {
    new_parser_from_source_str(
        psess,
        match file_path {
            Some(path) => FileName::Real(RealFileName::from_virtual_path(path)),
            None => FileName::anon_source_code(&contents),
        },
        contents,
        StripTokens::Nothing,
    )
    .unwrap()
}

/// Parses a string `contents` into a vector of ast::Items that can then be inserted into
/// any crate.
pub fn parse_items(
    psess: &ParseSess,
    contents: String,
    file_path: Option<&Path>,
) -> Vec<Box<ast::Item>> {
    let mut parser = create_parser(psess, contents, file_path);

    let mut res = Vec::new();
    loop {
        match parser.parse_item(ForceCollect::No) {
            Ok(Some(item)) => {
                res.push(item);
            }
            Ok(None) => break, // no more items
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse item!");
            }
        }
    }

    res
}
/// Parses a string `contents` into a vector of ast::Items that can then be inserted into
/// any crate.
pub fn parse_expr(psess: &ParseSess, contents: String) -> ast::Expr {
    let mut parser = create_parser(psess, contents, None);

    match parser.parse_expr() {
        Ok(expr) => *expr,
        Err(diag) => {
            diag.emit();
            panic!("Unable to parse expression!")
        }
    }
}

pub fn parse_single_unstable_compiler_attribute(
    psess: &ParseSess,
    contents: String,
    file_path: Option<&Path>,
) -> ast::Attribute {
    let mut parser = create_parser(psess, contents, file_path);

    parser
        .parse_inner_attributes()
        .expect(&format!("Unable to parse Attribute"))
        .into_iter()
        .next()
        .expect("Attribute list has zero elements")
}

/// Parses a string `contents` as a sequence of statements. The source is
/// wrapped as the body of a dummy block so the parser accepts bare statements.
pub fn parse_stmts(psess: &ParseSess, contents: String) -> Vec<ast::Stmt> {
    let wrapped = format!("{{ {contents} }}");
    let mut parser = create_parser(psess, wrapped, None);
    let expr = match parser.parse_expr() {
        Ok(e) => *e,
        Err(diag) => {
            diag.emit();
            panic!("Failed to parse stmts block");
        }
    };
    match expr.kind {
        ast::ExprKind::Block(block, _) => {
            let block = *block;
            block.stmts.into_iter().collect()
        }
        _ => panic!("Expected a block when parsing stmts"),
    }
}

pub fn parse_crate(psess: &ParseSess, contents: String, file_path: Option<&Path>) -> ast::Crate {
    let mut parser = create_parser(psess, contents, file_path);

    match parser.parse_crate_mod() {
        Ok(krate) => krate,
        Err(diag) => {
            diag.emit();
            panic!("Failed to parse crate!")
        }
    }
}
