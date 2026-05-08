//! Provides helper functions to construct rustc expressions/items/etc from Strings, used by
//! [crate::callbacks::codegen], [crate::callbacks::instrument].

/// Construct a rustc parser within the current parse session.
fn create_parser<'a>(
    psess: &'a rustc_session::parse::ParseSess,
    contents: String,
    file_path: Option<&std::path::Path>,
) -> rustc_parse::parser::Parser<'a> {
    // use this isntead?  more so matches what rustc does...
    // psess.source_map().path_mapping().to_real_filename(working_dir, path)
    rustc_parse::new_parser_from_source_str(
        psess,
        match file_path {
            Some(path) => {
                rustc_span::FileName::Real(rustc_span::RealFileName::from_virtual_path(path))
            }
            None => rustc_span::FileName::anon_source_code(&contents),
        },
        contents,
        rustc_parse::lexer::StripTokens::Nothing,
    )
    .unwrap()
}

/// Parses a string `contents` into a vector of rustc_ast::Items.
pub fn parse_items(
    psess: &rustc_session::parse::ParseSess,
    contents: String,
    file_path: Option<&std::path::Path>,
) -> Vec<Box<rustc_ast::Item>> {
    let mut parser = create_parser(psess, contents, file_path);

    let mut res = Vec::new();
    loop {
        match parser.parse_item(
            rustc_parse::parser::ForceCollect::No,
            rustc_parse::parser::AllowConstBlockItems::No,
        ) {
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

/// Parses a string `contents` into a rustc_ast::Expr.
pub fn parse_expr(psess: &rustc_session::parse::ParseSess, contents: String) -> rustc_ast::Expr {
    let mut parser = create_parser(psess, contents, None);

    match parser.parse_expr() {
        Ok(expr) => *expr,
        Err(diag) => {
            diag.emit();
            panic!("Unable to parse expression!")
        }
    }
}

/// Parses a string `contents` into top-level inner attributes.
pub fn parse_single_unstable_compiler_attribute(
    psess: &rustc_session::parse::ParseSess,
    contents: String,
    file_path: Option<&std::path::Path>,
) -> rustc_ast::Attribute {
    let mut parser = create_parser(psess, contents, file_path);

    parser
        .parse_inner_attributes()
        .expect(&format!("Unable to parse Attribute"))
        .into_iter()
        .next()
        .expect("Attribute list has zero elements")
}

/// Parses a string `contents` into a rustc_ast::Stmt.
pub fn parse_stmt(psess: &rustc_session::parse::ParseSess, contents: String) -> rustc_ast::Stmt {
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
        rustc_ast::ExprKind::Block(block, _) => {
            let mut block = *block;
            block.stmts.pop().expect("called common::parse_stmt with a multi-statements codeblock. Use common::parse_stmts instead.")
        }
        _ => panic!("Expected a block when parsing stmts"),
    }
}

/// Parses a string `contents`, into an ast-represented Crate.
pub fn parse_crate(
    psess: &rustc_session::parse::ParseSess,
    contents: String,
    file_path: Option<&std::path::Path>,
) -> rustc_ast::Crate {
    let mut parser = create_parser(psess, contents, file_path);

    match parser.parse_crate_mod() {
        Ok(krate) => krate,
        Err(diag) => {
            diag.emit();
            panic!("Failed to parse crate!")
        }
    }
}
