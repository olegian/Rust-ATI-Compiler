use std::collections::HashSet;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::token;
use rustc_parse::lexer::StripTokens;
use rustc_parse::new_parser_from_source_str;
use rustc_session::parse::ParseSess;
use rustc_span::FileName;
use rustc_span::{DUMMY_SP, Ident, Symbol};

use crate::instrumentation::common;

pub struct ModifyParamsVisitor<'a> {
    psess: &'a ParseSess,
    modified_functions: HashSet<Ident>,
}

impl<'a> MutVisitor for ModifyParamsVisitor<'a> {
    fn visit_item(&mut self, item: &mut ast::Item) {
        match item.kind {
            // To all non-skipped function definitions, push on a u32
            ast::ItemKind::Fn(box ast::Fn {
                ref mut ident,
                ref mut sig,
                ..
            }) => {
                if !common::is_function_skipped(ident, &item.attrs) {
                    // TODO: not sure if this works with complex function invocations
                    // that use some mod::submod::func_name() thing, might need to preserve
                    // the entire path as an identifier of the function, and use that in
                    // the modified_functions set.
                    self.modified_functions.insert(*ident);

                    self.push_param(&mut sig.decl, "added_by_compiler", "u32");
                }
            }

            _ => {}
        }

        mut_visit::walk_item(self, item);
    }
}

impl<'a> ModifyParamsVisitor<'a> {
    pub fn new(psess: &'a ParseSess) -> Self {
        Self {
            psess,
            modified_functions: HashSet::new(),
        }
    }

    pub fn get_modified_funcs(&self) -> &HashSet<Ident> {
        &self.modified_functions
    }

    /// Parse a type string into an ast::Ty
    fn parse_type(&self, type_str: &str) -> Box<ast::Ty> {
        let mut parser = new_parser_from_source_str(
            self.psess,
            FileName::anon_source_code(type_str),
            type_str.to_string(),
            StripTokens::Nothing,
        )
        .unwrap();

        match parser.parse_ty() {
            Ok(ty) => ty,
            Err(diag) => {
                diag.emit();
                panic!("Failed to parse type: {}", type_str)
            }
        }
    }

    /// Creates a new parameter: `_my_struct: &mut MyStruct`
    fn create_param(&self, param_name: &str, param_type: &str) -> ast::Param {
        let ty = self.parse_type(param_type);
        let param_name = Symbol::intern(param_name);

        let ident = Ident::new(param_name, DUMMY_SP);

        let pat = Box::new(ast::Pat {
            id: ast::DUMMY_NODE_ID,
            kind: ast::PatKind::Ident(ast::BindingMode::NONE, ident, None),
            span: DUMMY_SP,
            tokens: None,
        });

        ast::Param {
            attrs: ast::AttrVec::new(),
            ty,
            pat,
            id: ast::DUMMY_NODE_ID,
            span: DUMMY_SP,
            is_placeholder: false,
        }
    }

    /// Modify function declaration to add the new parameter
    fn push_param(&self, fn_decl: &mut Box<ast::FnDecl>, param_name: &str, param_type: &str) {
        let new_param = self.create_param(param_name, param_type);
        fn_decl.inputs.push(new_param);
    }
}

pub struct UpdateInvocationsVisitor<'a> {
    // functions which ModifyParamVisitor modified to include the extra params
    modified_functions: &'a HashSet<Ident>,
}

impl<'a> MutVisitor for UpdateInvocationsVisitor<'a> {
    /// adds extra parameter to each function invocation which isn't skipped
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        if let ast::ExprKind::Call(ref func, ref mut args) = expr.kind {
            if let ast::ExprKind::Path(None, path) = &func.kind {
                // TODO: not sure if this works with complex function invocations
                // that use some mod::submod::func_name() thing, might need to preserve
                // the entire path as an identifier of the function, and use that in
                // the modified_functions set.
                if let Some(last_segment) = path.segments.last() {
                    if self.modified_functions.contains(&last_segment.ident) {
                        let arg = self.create_arg();
                        args.push(arg);
                    }
                }
            }
        }

        // continue visiting nested expressions
        mut_visit::walk_expr(self, expr);
    }
}

impl<'a> UpdateInvocationsVisitor<'a> {
    pub fn new(modified_functions: &'a HashSet<Ident>) -> Self {
        Self { modified_functions }
    }

    /// Parse a type string into an ast::Ty
    fn create_arg(&self) -> Box<ast::Expr> {
        // technically, we will be passing a variable so...
        // let ident = Ident::new(Symbol::intern("var_name"), DUMMY_SP);

        // Box::new(ast::Expr {
        //     id: ast::DUMMY_NODE_ID,
        //     kind: ast::ExprKind::Path(
        //         None,
        //         ast::Path::from_ident(ident),
        //     ),
        //     span: DUMMY_SP,
        //     attrs: ast::AttrVec::new(),
        //     tokens: None,
        // })

        // ... just passing a literal for now
        Box::new(ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::Lit(token::Lit {
                kind: token::LitKind::Integer, // specifies literal type
                symbol: Symbol::intern("100"),
                suffix: None,
            }),
            span: DUMMY_SP,
            attrs: ast::AttrVec::new(),
            tokens: None,
        })
    }
}
