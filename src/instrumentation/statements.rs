use std::collections::{HashSet};

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::token;

use rustc_parse::{lexer::StripTokens, new_parser_from_source_str};
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, FileName, Ident, Symbol};

use crate::instrumentation::common;

pub struct ATIVisitor<'psess, 'modfuncs> {
    psess: &'psess ParseSess,
    // used to determine whether a funtion invocation requires passing a TaggedValue or just the raw value
    modified_funcs: &'modfuncs HashSet<Ident>,
}

// TODO: epilogues have to be inserted before all returns, not just at the end of the body
impl<'psess, 'modfuncs> MutVisitor for ATIVisitor<'psess, 'modfuncs> {
    fn visit_item(&mut self, item: &mut ast::Item) {
        if let ast::ItemKind::Fn(box ast::Fn {
            ref mut body,
            ref ident,
            ref mut sig,
            ..
        }) = item.kind {
            if let Some(block) = body {
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
                    }
                }

                if common::is_function_main(ident) {
                    // main function has a slightly different prelude/epilogue
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

                if !common::is_function_skipped(ident, &item.attrs) {
                    let param_names: Vec<_> = sig.decl.inputs.iter().map(|param| {
                        if let ast::PatKind::Ident(_, ref ident, _) = param.pat.kind {
                            ident.as_str()
                        } else {
                            panic!();
                        }
                    }).collect();

                    let prelude = self.create_prelude(ident.as_str(), &param_names);
                    for (i, stmt) in prelude.into_iter().enumerate() {
                        block.stmts.insert(i, stmt);
                    }

                    let epilogue = self.create_epilogue();
                    // TODO: dirty way of inserting before end
                    let len = block.stmts.len() - 1;
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
            *expr = self.tupleify_expr(expr);
        } else if let ast::ExprKind::Call(ref func, ref mut args) = expr.kind {
            if let ast::ExprKind::Path(None, path) = &func.kind {
                // TODO: not sure if this works with complex function invocations
                // that use some mod::submod::func_name() thing, might need to preserve
                // the entire path as an identifier of the function, and use that in
                // the modified_functions set.
                if let Some(last_segment) = path.segments.last() {
                    if self.modified_funcs.contains(&last_segment.ident) {
                        // args are being passed to a tracked function, retain tuplings if necessary
                        // TODO:  something i assume

                    } else {
                        // args are being passed to an untracked function
                        for arg_expr in args.iter_mut() {
                            // TODO: include check for non tupled args, so we don't accidentally unbind
                            arg_expr.kind = self.unbind_tupled_expr(arg_expr);
                        }

                        *expr = self.tupleify_expr(expr);
                    }
                }
            }
        } else if let ast::ExprKind::MacCall(box ast::MacCall {
            ref mut path,
            ref mut args,
        }) = expr.kind {
            // TODO: handle macro invocations, also handle methods at some point holy shit
        }
    }
}

impl<'psess, 'modfuncs> ATIVisitor<'psess, 'modfuncs> {
    pub fn new(psess: &'psess ParseSess, modified_funcs: &'modfuncs HashSet<Ident>) -> Self {
        ATIVisitor { psess, modified_funcs }
    }

    // TODO: I'm unsure if parse_code's additional block scope will move the analysis stuff out of scope
    // this wasn't a problem when I was trying to create a var in prelude and use in epilogue, so for now its fine
    fn create_main_prelude(&self) -> Vec<ast::Stmt> {
        let code = r#"
            let mut site = ATI_ANALYSIS.lock().unwrap().get_site(stringify!(main));
        "#;
        self.parse_code(code)
    }

    fn create_main_epilogue(&self) -> Vec<ast::Stmt> {
        // TODO: modify .report() to cleanly output to file.
        let code = r#"
            let mut ati_locked = ATI_ANALYSIS.lock().unwrap();
            ati_locked.update_site(site);
            ati_locked.report();
        "#;
        self.parse_code(code)
    }

    fn create_prelude(&self, func_name: &str, param_names: &[&str]) -> Vec<ast::Stmt> {
        let site = format!(r#"
            let mut site = ATI_ANALYSIS.lock().unwrap().get_site(stringify!({func_name}));
        "#);
        let param_binds: String = param_names.iter().map(|param_name| {
            format!(r#"site.bind(stringify!({param_name}), {param_name});"#)
        }).collect::<Vec<_>>().join("");

        let code = format!(r#"
            {site}
            {param_binds}
        "#);

        self.parse_code(&code)
    }

    fn create_epilogue(&self) -> Vec<ast::Stmt> {
        let code = r#"
            ATI_ANALYSIS.lock().unwrap().update_site(site);
        "#;
        self.parse_code(code)
    }

    // expr is already the rhs of a `let x = ...` statement
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
                                    ident: Ident::new(
                                        Symbol::intern("ATI"),
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
