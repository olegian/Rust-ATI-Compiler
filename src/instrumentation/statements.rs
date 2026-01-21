use std::collections::HashSet;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::token;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str};
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, FileName, Ident, Symbol};

use crate::instrumentation::common;

pub struct ATIVisitor<'psess, 'modfuncs> {
    psess: &'psess ParseSess,
    // used to determine whether a function invocation requires passing a TaggedValue or just the raw value
    modified_funcs: &'modfuncs HashSet<String>,
}

impl<'psess, 'modfuncs> MutVisitor for ATIVisitor<'psess, 'modfuncs> {
    fn visit_item(&mut self, item: &mut ast::Item) {
        if let ast::ItemKind::Fn(box ast::Fn {
            ref mut body,
            ref ident,
            ref mut sig,
            ..
        }) = item.kind
        {
            if let Some(block) = body {
                let fn_type = common::function_type(ident, &item.attrs);
                if matches!(fn_type, common::AtiFnType::Untracked) {
                    // could be useful to perform any stub management stuff
                    return;
                }

                let mut return_locations: Vec<usize> = Vec::new();
                for (i, stmt) in block.stmts.iter_mut().enumerate() {
                    // TODO: there are a bunch of ways to bind variables
                    // handle all of them, not just `let x = 10` type statements.
                    match stmt.kind {
                        ast::StmtKind::Let(box ast::Local {
                            pat:
                                box ast::Pat {
                                    kind: ast::PatKind::Ident(_, ref var_ident, _),
                                    ..
                                },
                            kind: ast::LocalKind::Init(box ref mut expr),
                            ..
                        }) => {
                            if common::is_expr_tupled(&expr.kind) {
                                *expr = self.create_let_site_bind(var_ident, expr);
                            }
                        }

                        // this currently catches too many returns... idk what will
                        // happen when closures are introduced / other places where you can return
                        // like match statements.
                        ast::StmtKind::Semi(box ast::Expr {
                            kind: ast::ExprKind::Ret(_),
                            ..
                        })
                        | ast::StmtKind::Expr(_) => {
                            return_locations.push(i);
                        }

                        _ => {}
                    }
                }

                let (prelude, epilogue) = match fn_type {
                    common::AtiFnType::Main => {
                        (self.create_main_prelude(), self.create_main_epilogue())
                    }
                    common::AtiFnType::Tracked => (
                        self.create_prelude(ident.as_str(), &sig.decl.inputs),
                        self.create_epilogue(),
                    ),
                    common::AtiFnType::Untracked => {
                        unreachable!();
                    }
                };

                // add epilogue before every return statement
                if return_locations.is_empty() {
                } else {
                    for return_loc in return_locations.into_iter().rev() {
                        // TODO: 
                        // if let ast::StmtKind::Semi(box ast::Expr {
                        //     kind: ast::ExprKind::Ret(Some(ret_expr)),
                        //     ..
                        // })
                        // | ast::StmtKind::Expr(ret_expr) = &block.stmts[return_loc].kind
                        // {
                        //     self.split_return(ret_expr);
                        // }

                        block
                            .stmts
                            .splice(return_loc..return_loc, epilogue.clone().into_iter());
                    }
                }

                // add prolouge at start (important to do this last)
                block.stmts.splice(0..0, prelude.into_iter());
            } else {
                // function with no body?
            }
        }

        mut_visit::walk_item(self, item);
    }

    // Converts all literals into TaggedValue<T>'s
    // and makes sure those values are correctly passed
    // between the tracked/untracked boundary.
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        mut_visit::walk_expr(self, expr);

        match expr.kind {
            ast::ExprKind::Lit(_) => {
                *expr = self.tupleify_expr(expr);
            }

            ast::ExprKind::Call(ref func, ref mut args) => {
                if let ast::ExprKind::Path(None, path) = &func.kind {
                    // TODO: not sure if this works with complex function invocations
                    // that involve use statements and renames. might have to construct
                    // down paths from crate::. Temporary workaround below
                    // prolly need to change Ident keys later.
                    // let full_name = path.segments.iter().map(|seg| seg.ident.as_str()).collect::<Vec<_>>().join("::");

                    if let Some(last_segment) = path.segments.last() {
                        if !self.modified_funcs.contains(last_segment.ident.as_str()) {
                            for arg_expr in args.iter_mut() {
                                arg_expr.kind = self.unbind_tupled_expr(arg_expr);
                            }

                            *expr = self.tupleify_expr(expr);
                        }
                    }
                }
            }

            ast::ExprKind::MacCall(box ast::MacCall {
                ref mut path,
                ref mut args,
            }) => {
                // TODO: handle macro invocations
            }

            // TODO: handle method calls
            _ => {}
        }
    }
}

impl<'psess, 'modfuncs> ATIVisitor<'psess, 'modfuncs> {
    pub fn new(psess: &'psess ParseSess, modified_funcs: &'modfuncs HashSet<String>) -> Self {
        ATIVisitor {
            psess,
            modified_funcs,
        }
    }

    /// Gets statements associated with main prelude, i.e.
    /// creating the new site
    fn create_main_prelude(&self) -> Vec<ast::Stmt> {
        let code = r#"
            let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main));
        "#;
        self.parse_code(code)
    }

    /// Gets statements associated with main epilogue, i.e.
    /// outputing everything
    fn create_main_epilogue(&self) -> Vec<ast::Stmt> {
        // TODO: modify .report() to cleanly output to file.
        let code = r#"
            let mut ati_locked = ATI_ANALYSIS.lock().unwrap();
            ati_locked.update_site(site_exit);
            ati_locked.report();
        "#;
        self.parse_code(code)
    }

    /// Prelude for all tracked functions other than main
    /// Creates two sites, one for exit and one for enter
    fn create_prelude(&self, func_name: &str, params: &[ast::Param]) -> Vec<ast::Stmt> {
        let param_binds: String = params
            .iter()
            .filter(|param| common::is_type_tupled(&param.ty))
            .map(|param| {
                if let ast::PatKind::Ident(_, ref ident, _) = param.pat.kind {
                    let param_name = ident.as_str();
                    format!(
                        r#"
                        site_enter.bind(stringify!({param_name}), {param_name});
                        site_exit.bind(stringify!({param_name}), {param_name});
                    "#
                    )
                } else {
                    panic!();
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let code = format!(
            r#"
            let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({func_name}::ENTER));
            let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({func_name}::EXIT));
            {param_binds}
            ATI_ANALYSIS.lock().unwrap().update_site(site_enter);
        "#
        );

        self.parse_code(&code)
    }

    /// Creates epilogue used for all functions but main
    fn create_epilogue(&self) -> Vec<ast::Stmt> {
        let code = r#"
            ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
        "#;
        self.parse_code(code)
    }

    fn split_return(&self, expr: &ast::Expr) -> Vec<ast::Stmt> {
        let code = r#"
            let ret = RETURN_PLACEHOLDER;
            return ret;
        "#;

        let block_ast = self.parse_code(code);
        println!("{:?}", block_ast);
        todo!();
    }

    /// Takes a local variable assignment and adds the variable to the site
    /// expr is already the rhs of a `let x = ...` statement
    fn create_let_site_bind(&self, var_ident: &Ident, expr: &ast::Expr) -> ast::Expr {
        // TODO: There has to be easier ways to construct these kinds of nodes,
        // like some sort of helper function. Doing this manually sucks.
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
                            segments: [ast::PathSegment {
                                ident: Ident::new(Symbol::intern("site_exit"), DUMMY_SP),
                                id: ast::DUMMY_NODE_ID,
                                args: None,
                            }]
                            .into(),
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
                        kind: ast::ExprKind::MacCall(Box::new(ast::MacCall {
                            path: ast::Path {
                                span: DUMMY_SP,
                                segments: [ast::PathSegment {
                                    ident: Ident::new(Symbol::intern("stringify"), DUMMY_SP),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                }]
                                .into(),
                                tokens: None,
                            },
                            args: Box::new(ast::DelimArgs {
                                dspan: ast::tokenstream::DelimSpan {
                                    open: DUMMY_SP,
                                    close: DUMMY_SP,
                                },
                                delim: token::Delimiter::Parenthesis,
                                tokens: ast::tokenstream::TokenStream::new(
                                    [ast::tokenstream::TokenTree::token_joint_hidden(
                                        token::TokenKind::Ident(
                                            Symbol::intern(var_ident.as_str()),
                                            token::IdentIsRaw::No,
                                        ),
                                        DUMMY_SP,
                                    )]
                                    .into(),
                                ),
                            }),
                        })),
                        span: DUMMY_SP,
                        attrs: [].into(),
                        tokens: None,
                    }),
                    Box::new(expr.clone()),
                ]
                .into(),
                span: DUMMY_SP,
            })),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        }
    }

    fn tupleify_expr(&self, expr: &ast::Expr) -> ast::Expr {
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
                                    ident: Ident::new(Symbol::intern("ATI"), DUMMY_SP),
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
                [Box::new(expr.clone())].into(),
            ),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        };

        new_expr
    }

    fn unbind_tupled_expr(&self, expr: &mut ast::Expr) -> ast::ExprKind {
        ast::ExprKind::Field(
            Box::new(expr.clone()),
            Ident::new(Symbol::intern("0"), DUMMY_SP),
        )

        // alternative way of doing the above thing
        // ast::ExprKind::MethodCall(
        //     Box::new(ast::MethodCall {
        //         seg: ast::PathSegment {
        //             ident: Ident::new(Symbol::intern("unbind"), DUMMY_SP),
        //             id: ast::DUMMY_NODE_ID,
        //             args: None,
        //         },
        //         receiver: Box::new(expr.clone()),
        //         args: [].into(),
        //         span: DUMMY_SP,
        //     })
        // )
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
