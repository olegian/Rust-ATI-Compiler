use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::token;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str};
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, FileName, Ident, Symbol};

use crate::instrumentation::common;

pub struct ATIVisitor<'psess> {
    psess: &'psess ParseSess,
}

// TODO: epilogues have to be inserted before all returns, not just at the end of the body
impl<'psess> MutVisitor for ATIVisitor<'psess> {
    fn visit_item(&mut self, item: &mut ast::Item) {
        if let ast::ItemKind::Fn(box ast::Fn {
            ref mut body,
            ref ident,
            ..
        }) = item.kind
        {
            if let Some(block) = body {
                if common::is_function_main(ident) {
                    for stmt in &mut block.stmts {
                        // TODO: there are a bunch of ways to bind variables
                        // handle all of them, not just `let x = 10` type statements.
                        if let ast::StmtKind::Let(box ast::Local {
                            pat: box ast::Pat {
                                kind: ast::PatKind::Ident(_, ref var_ident, _),
                                ..
                            },
                            kind: ast::LocalKind::Init(box ref mut expr),
                            ..
                        }) = stmt.kind
                        {
                            *expr = self.create_let_site_bind(var_ident, expr);
                            // println!("{:?}", expr);
                        }
                    }

                    let prelude = self.create_main_prelude();
                    for (i, stmt) in prelude.into_iter().enumerate() {
                        block.stmts.insert(i, stmt);
                    }

                    let epilogue = self.create_main_epilogue();
                    let len = block.stmts.len();
                    for (i, stmt) in epilogue.into_iter().enumerate() {
                        block.stmts.insert(len + i, stmt);
                    }
                }
            }
        }

        mut_visit::walk_item(self, item);
    }

    // Converts all literals into TaggedValue<T>'s
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        mut_visit::walk_expr(self, expr);

        if let ast::ExprKind::Lit(_) = expr.kind {
            // expression is a literal!
            // convert the expression into a TaggedValue
            *expr = self.tupleify_literal_expr(expr);
        }
    }
}

impl<'psess> ATIVisitor<'psess> {
    pub fn new(psess: &'psess ParseSess) -> Self {
        ATIVisitor { psess }
    }

    // TODO: I'm unsure if parse_code's additional block scope will move the analysis stuff out of scope
    // this wasn't a problem when I was trying to create a var in prelude and use in epilogue, so for now its fine
    fn create_main_prelude(&self) -> Vec<ast::Stmt> {
        let code = r#"
            let ATI_ANALYSIS: ATI = Rc::new(RefCell::new(AbstractTypeInference::new()));
            let mut site = ATI_ANALYSIS.borrow_mut().get_site(stringify!(main));
        "#;
        self.parse_code(code)
    }

    fn create_main_epilogue(&self) -> Vec<ast::Stmt> {
        // TODO: modify .report() to cleanly output to file.
        let code = r#"
            ATI_ANALYSIS.borrow_mut().update_site(site);
            ATI_ANALYSIS.borrow().report();
        "#;
        self.parse_code(code)
    }

    // expr is already the rhs of a `let x = ...` statement
    fn create_let_site_bind(&self, var_ident: &Ident, expr: &ast::Expr) -> ast::Expr {
        ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::MethodCall(Box::new(ast::MethodCall {
                seg: ast::PathSegment {
                    ident: Ident::new(Symbol::intern("bind"), DUMMY_SP),
                    id: ast::DUMMY_NODE_ID,
                    args: None,
                },
                receiver: Box::new(ast::Expr {
                    id: ast::DUMMY_NODE_ID,
                    kind: ast::ExprKind::Path(
                        None, 
                        ast::Path {
                            span: DUMMY_SP, 
                            segments: [
                                ast::PathSegment {
                                    ident: Ident::new(Symbol::intern("site"), DUMMY_SP),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                            ].into(),
                            tokens: None,                            
                        },
                    ),
                    span: DUMMY_SP,
                    attrs: [].into(),
                    tokens: None,
                }),
                args: [
                    Box::new(ast::Expr {
                        id: ast::DUMMY_NODE_ID,
                        kind: ast::ExprKind::MacCall(
                            Box::new(ast::MacCall {
                                path: ast::Path {
                                    span: DUMMY_SP,
                                    segments: [
                                        ast::PathSegment {
                                            ident: Ident::new(Symbol::intern("stringify"), DUMMY_SP),
                                            id: ast::DUMMY_NODE_ID,
                                            args: None,
                                        },
                                        ].into(),
                                        tokens: None,
                                },
                                args: Box::new(ast::DelimArgs {
                                    dspan: ast::tokenstream::DelimSpan {
                                        open: DUMMY_SP,
                                        close: DUMMY_SP,
                                    },
                                    delim: token::Delimiter::Parenthesis,
                                    tokens: ast::tokenstream::TokenStream::new( [
                                        ast::tokenstream::TokenTree::token_joint_hidden(
                                            token::TokenKind::Ident(Symbol::intern(var_ident.as_str()), token::IdentIsRaw::No),
                                            DUMMY_SP
                                        )
                                    ].into()),
                                })
                            })
                        ),
                        span: DUMMY_SP,
                        attrs: [].into(),
                        tokens: None,
                    }),
                    Box::new(expr.clone())
                ].into(),
                span: DUMMY_SP,
            })),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        }
    }

    fn tupleify_literal_expr(&self, expr: &ast::Expr) -> ast::Expr {
        let ati_clone = Box::new(ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::MethodCall(Box::new(ast::MethodCall {
                seg: ast::PathSegment {
                    ident: Ident::new(Symbol::intern("clone"), DUMMY_SP),
                    id: ast::DUMMY_NODE_ID,
                    args: None,
                },
                // The receiver, e.g. `x`.
                receiver: Box::new(ast::Expr {
                    id: ast::DUMMY_NODE_ID,
                    kind: ast::ExprKind::Path(
                        None,
                        ast::Path {
                            span: DUMMY_SP,
                            segments: [ast::PathSegment {
                                ident: Ident::new(Symbol::intern("ATI_ANALYSIS"), DUMMY_SP),
                                id: ast::DUMMY_NODE_ID,
                                args: None,
                            }]
                            .into(),
                            tokens: None,
                        },
                    ),
                    tokens: None,
                    attrs: [].into(),
                    span: DUMMY_SP,
                }),
                // The arguments, e.g. `a, b, c`.
                args: [].into(),
                span: DUMMY_SP,
            })),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        });

        let new_expr = ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::Call(
                Box::new(ast::Expr {
                    id: ast::DUMMY_NODE_ID,
                    kind: ast::ExprKind::Path(
                        None,
                        ast::Path {
                            span: DUMMY_SP,
                            segments: [
                                ast::PathSegment {
                                    ident: Ident::new(
                                        Symbol::intern("AbstractTypeInference"),
                                        DUMMY_SP,
                                    ),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                                ast::PathSegment {
                                    ident: Ident::new(Symbol::intern("track"), DUMMY_SP),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                            ]
                            .into(),
                            tokens: None,
                        },
                    ),
                    span: DUMMY_SP,
                    attrs: [].into(),
                    tokens: None,
                }),
                [ati_clone, Box::new(expr.clone())].into(),
            ),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        };

        new_expr
    }

    fn create_print_statement(&self) -> Vec<ast::Stmt> {
        let code = r#"
            println!("From compiler: {}", added_by_compiler);
        "#;
        self.parse_code(code)
    }

    fn create_prelude(&self) -> Vec<ast::Stmt> {
        let code = r#"
            let a: Id = 10;
            // let t = Tag::new(&a);
        "#;
        self.parse_code(code)
    }

    fn create_epilogue(&self) -> Vec<ast::Stmt> {
        let code = r#"
            println!("{:?}", a);
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

/*
else if !common::is_function_skipped(ident, &item.attrs) {
    // params used by this wont be available in skipped functions
    let print_stmts = self.create_print_statement();
    for (i, stmt) in print_stmts.into_iter().enumerate() {
        block.stmts.insert(i, stmt);
    }

    // FIXME: on next rewrite change above functions to accepts &mut stmts and directly modify
    // note: that'll require resolving the thin-vec version bullshit, so maybe not worth?
    let prelude = self.create_prelude();
    for (i, stmt) in prelude.into_iter().enumerate() {
        block.stmts.insert(i, stmt);
    }

    // TODO: only add epilogue before return statements
    // be careful, because not all functions have returns either
    // need to do more pattern matching here
    let epilogue = self.create_epilogue();
    let len = block.stmts.len() - 1;
    for (i, stmt) in epilogue.into_iter().enumerate() {
        block.stmts.insert(len + i, stmt);
    }
}
*/
