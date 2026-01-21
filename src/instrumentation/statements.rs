use std::collections::HashSet;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::token;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str};
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, FileName, Ident, Symbol};

use crate::instrumentation::common::{self, AtiFnType};

pub struct ATIVisitor<'psess, 'modfuncs> {
    psess: &'psess ParseSess,
    // used to determine whether a function invocation requires passing a TaggedValue or just the raw value
    modified_funcs: &'modfuncs HashSet<String>,
}

impl<'psess, 'modfuncs> MutVisitor for ATIVisitor<'psess, 'modfuncs> {
    // Modify each function's body to perform instrumentation
    // do this on block level?
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
                            // Uncomment to see output regarding all local vars
                            // if common::is_expr_tupled(&expr.kind) {
                            //     *expr = self.create_let_site_bind(var_ident, expr);
                            // }
                        }

                        // TODO: it's wierdly difficult to find return points
                        // this currently not enough returns... idk what will
                        // happen when closures are introduced too. We need to walk
                        // all expressions in this function, but retain info about
                        // the fn_type we are currently instrumenting.
                        ast::StmtKind::Semi(box ast::Expr {
                            kind: ast::ExprKind::Ret(_),
                            ..
                        }) => {
                            return_locations.push(i);
                        }

                        _ => {}
                    }
                }

                // handle last statement seperately, if it's a returned value
                if let Some(ast::Stmt {
                    kind: ast::StmtKind::Semi(_),
                    ..
                }) = block.stmts.iter_mut().last()
                {
                    // return_locations.push(block.stmts.len());
                }

                // add epilogue before every return point
                for return_loc in return_locations.into_iter().rev() {
                    let return_stmt = block.stmts.remove(return_loc);
                    let new_return_stmts = self.create_epilogue(fn_type, return_stmt);

                    block
                        .stmts
                        .splice((return_loc)..(return_loc), new_return_stmts.into_iter());
                }

                // add prolouge at start (important to do this last)
                let prelude = self.create_prelude(fn_type, ident.as_str(), &sig.decl.inputs);
                block.stmts.splice(0..0, prelude.into_iter());
            } else {
                // function with no body? does this ever need instrumentation?
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

            ast::ExprKind::Ret(_) => {
                // exit point from function? need epilogue which needs fn type...
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

    /// Prelude for all tracked functions other than main
    /// Creates two sites, one for exit and one for enter
    fn create_prelude(
        &self,
        fn_type: AtiFnType,
        func_name: &str,
        params: &[ast::Param],
    ) -> Vec<ast::Stmt> {
        let code = match fn_type {
            AtiFnType::Main => String::from(
                r#"
                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main));
                "#,
            ),
            AtiFnType::Tracked => {
                // for each parameter, if it's tupled add
                // statements to bind those parameters
                // to both enter and exit sites.
                let param_binds: String = params
                    .iter()
                    .filter(|param| common::is_type_tupled(&param.ty))
                    .map(|param| {
                        if let ast::PatKind::Ident(_, ref ident, _) = param.pat.kind {
                            let param_name = ident.as_str();
                            format!(r#"
                                site_enter.bind(stringify!({param_name}), {param_name});
                                site_exit.bind(stringify!({param_name}), {param_name});
                            "#)
                        } else {
                            unreachable!();
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("");

                format!(r#"
                    let mut site_enter = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({func_name}::ENTER));
                    let mut site_exit = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({func_name}::EXIT));
                    {param_binds}
                    ATI_ANALYSIS.lock().unwrap().update_site(site_enter);
                "#)
            }
            AtiFnType::Untracked => {
                unreachable!();
            }
        };

        self.parse_code(&code)
    }

    fn create_epilogue_without_ret(&self, fn_type: AtiFnType) -> Vec<ast::Stmt> {

        todo!();
    }

    /// Creates epilogue used for all functions but main
    fn create_epilogue(&self, fn_type: AtiFnType, ret_stmt: ast::Stmt) -> Vec<ast::Stmt> {
        // empty return has no extra work to do
        // beyond closing the exit site

        match ret_stmt.kind {
            ast::StmtKind::Semi(box ast::Expr {
                kind: ast::ExprKind::Ret(None),
                ..
            }) => {
                let mut code = String::from(
                    r#"
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                "#,
                );

                if let AtiFnType::Main = fn_type {
                    code = format!(
                        r#"
                        {code}
                        ATI_ANALYSIS.lock().unwrap().report();
                    "#
                    );
                }

                self.parse_code(&code)
            }

            ast::StmtKind::Semi(box ast::Expr {
                kind: ast::ExprKind::Ret(Some(ret_expr)),
                ..
            })
            | ast::StmtKind::Expr(ret_expr) => {
                let split = r#"
                    let tmp_ret = site_exit.bind(stringify!(RET), RETURN_PLACEHOLDER);
                    ATI_ANALYSIS.lock().unwrap().update_site(site_exit);
                    return tmp_ret;
                "#;
                let mut split_stmts = self.parse_code(split);

                // replace the RETURN_PLACEHOLDER with the actual returned value
                if let ast::StmtKind::Let(box ast::Local {
                    kind: ast::LocalKind::Init(box ast::Expr {
                        kind: ast::ExprKind::MethodCall(
                            box ast::MethodCall {
                                ref mut args,
                                ..
                            }
                        ),
                        ..
                    }),
                    ..
                }) = split_stmts[0].kind
                {
                    args[1] = ret_expr;
                } else {
                    // due to defined structure for code string above.
                    unreachable!();
                }

                split_stmts
            }

            _ => {
                unreachable!();
            }
        }
    }

    /// Takes a local variable assignment and adds the variable to the site
    /// expr is already the rhs of a `let x = ...` statement
    /// This is going to be removed soon.
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

    /// Takes an expression of type T and converts it to a TaggedValue<T>
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

    /// Takes a TaggedValue<T> expression and unwraps it to just T
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

    /// Helper to parse a code string into a set of statements
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
