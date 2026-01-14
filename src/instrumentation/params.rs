use std::collections::HashSet;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
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
                sig: ast::FnSig { ref mut decl, .. },
                ..
            }) => {
                if !common::is_function_skipped(ident, &item.attrs) {
                    // TODO: not sure if this works with complex function invocations
                    // that use some mod::submod::func_name() thing, might need to preserve
                    // the entire path as an identifier of the function, and use that in
                    // the modified_functions set.
                    self.modified_functions.insert(*ident);

                    // go through parameters of function...
                    for ast::Param { ty, .. } in &mut decl.inputs {
                        if common::can_type_be_tupled(ty) {
                            // ... if type is tupled, we need to convert the type to be
                            // a TaggedValue<ty> to carry tracking info through fn boundary
                            ty.kind = self.tuple_type(ty);
                        }
                    }

                    if let ast::FnRetTy::Ty(ref mut return_type) = decl.output {
                        if common::can_type_be_tupled(return_type) {
                            // if return type exists and should also be tupled
                            return_type.kind = self.tuple_type(return_type);
                        }
                    }
                }
            },
            ast::ItemKind::Struct(_, _, ast::VariantData::Struct { ref mut fields, .. }) => {
                for field_def in fields {
                    if common::can_type_be_tupled(&*field_def.ty) {
                        field_def.ty.kind = self.tuple_type(&field_def.ty);
                    }
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

    fn tuple_type(&self, old_type: &ast::Ty) -> ast::TyKind {
        ast::TyKind::Path(
            None,
            ast::Path {
                segments: [ast::PathSegment {
                    ident: Ident::new(Symbol::intern("TaggedValue"), DUMMY_SP),
                    id: ast::DUMMY_NODE_ID,
                    args: Some(Box::new(ast::AngleBracketed(ast::AngleBracketedArgs {
                        span: DUMMY_SP,
                        args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                            Box::new(old_type.clone()),
                        ))]
                        .into(),
                    }))),
                }]
                .into(),
                span: DUMMY_SP,
                tokens: None,
            },
        )
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
