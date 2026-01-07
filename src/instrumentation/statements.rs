use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str};
use rustc_session::parse::ParseSess;
use rustc_span::FileName;

use crate::instrumentation::common;

pub struct ATIVisitor<'psess> {
    psess: &'psess ParseSess,
}

impl<'psess> MutVisitor for ATIVisitor<'psess> {
    fn visit_item(&mut self, item: &mut ast::Item) {
        // println!("{:?}", item);
        if let ast::ItemKind::Fn(box ast::Fn { ref mut body, ref ident, .. }) = item.kind {
            if let Some(block) = body {
                if !common::is_function_skipped(ident, &item.attrs) {
                    // params used by this wont be available in skipped functions
                    let print_stmts = self.create_print_statement();
                    for (i, stmt) in print_stmts.into_iter().enumerate() {
                        block.stmts.insert(i, stmt);
                    }
                    
                    // TODO: on next rewrite change above functions to accepts &mut stmts and
                    //       directly modify
                    let prelude = self.create_prelude();
                    let epilogue = self.create_epilogue();
                    
                    // TODO: figure out a better way of doing this kind of fenceposting
                    for (i, stmt) in prelude.into_iter().enumerate() {
                        block.stmts.insert(i, stmt);
                    }
                    
                    let len = block.stmts.len();
                    for (i, stmt) in epilogue.into_iter().enumerate() {
                        block.stmts.insert(len + i, stmt);
                    }
                }
            }
        }

        mut_visit::walk_item(self, item);
    }
}

impl<'psess> ATIVisitor<'psess> {
    pub fn new(psess: &'psess ParseSess) -> Self {
        ATIVisitor { psess }
    }

    fn create_print_statement(&self) -> Vec<ast::Stmt> {
        let code = r#"
            println!("From compiler: {}", added_by_compiler);
        "#;
        self.parse_code(code)
    }

    fn create_prelude(&self) -> Vec<ast::Stmt> {
        let code = r#"
            let a: u32 = 10;
            let t = Tag::new(&a);
        "#;
        self.parse_code(code)
    }

    fn create_epilogue(&self) -> Vec<ast::Stmt> {
        let code = r#"
            println!("{:?}", t);
        "#;
        self.parse_code(code)
    }

    fn create_var_bind() -> ast::Stmt {
        todo!();
    }

    fn parse_code(&self, code: &str) -> Vec<ast::Stmt> {
        let block = format!("{{ {} }}", code);
        let mut parser = new_parser_from_source_str(
            self.psess,
            FileName::anon_source_code(&block),
            block,
            StripTokens::Nothing,
        )
        .unwrap();

        match parser.parse_block() {
            Ok(block) => block.stmts.into_iter().collect(),
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse code block");
            }
        }
    }
}

/* Manually creating node to insert into AST
    fn create_print_statement(&self, fn_name: &str) -> ast::Stmt {
        let macro_name = Symbol::intern("println");

        let format_str = format!("Entering function: {}", fn_name);
        let format_token = TokenTree::Token(
            token::Token::new(
                TokenKind::Literal(token::Lit {
                    kind: token::LitKind::Str,
                    symbol: Symbol::intern(&format_str),
                    suffix: None,
                }),
                DUMMY_SP,
            ),
            Spacing::Alone,
        );

        let tts = TokenStream::new(vec![format_token]);

        let mac = ast::MacCall {
            path: ast::Path::from_ident(Ident::new(macro_name, DUMMY_SP)),
            args: Box::new(ast::DelimArgs {
                dspan: ast::tokenstream::DelimSpan::dummy(),
                delim: token::Delimiter::Parenthesis,
                tokens: tts,
            }),
        };

        let mac_stmt_style = ast::MacStmtStyle::Semicolon;

        ast::Stmt {
            id: ast::DUMMY_NODE_ID,
            kind: ast::StmtKind::MacCall(Box::new(ast::MacCallStmt {
                mac: Box::new(mac),
                style: mac_stmt_style,
                attrs: ast::AttrVec::new(),
                tokens: None,
            })),
            span: DUMMY_SP,
        }
    }
*/

/* Single stmt parse
parser
    .parse_stmt_without_recovery(false, ForceCollect::No, false)
    .unwrap_or_else(|diag| {
            diag.emit();
            panic!("Failed to parse statement: {}", code)
        }
    ).expect("No statement found in code")
*/
