use std::collections::HashSet;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_span::{DUMMY_SP, Ident, Symbol};

use crate::instrumentation::common::{self};

pub struct ModifyParamsVisitor {
    modified_functions: HashSet<String>,
}

impl MutVisitor for ModifyParamsVisitor {
    /// Converts all function signatures and top level type definitions (structs)
    /// to thier tagged variants. Specifically modifies all parameter types to
    /// be TaggedValues if necessary, alongside returns.
    fn visit_item(&mut self, item: &mut ast::Item) {
        match item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ref mut ident,
                sig: ast::FnSig { ref mut decl, .. },
                ..
            }) => {
                match common::function_type(ident, &item.attrs) {
                    common::AtiFnType::Main | common::AtiFnType::Tracked => {
                        // TODO: not sure if this works with complex function invocations
                        // that use some mod::submod::func_name() thing, might need to preserve
                        // the entire path as an identifier of the function, and use that in
                        // the modified_functions set.

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

                        self.modified_functions.insert(ident.as_str().into());
                    }
                    common::AtiFnType::Untracked => {}
                }
            }

            ast::ItemKind::Struct(_, _, ast::VariantData::Struct { ref mut fields, .. }) => {
                for field_def in fields {
                    if common::can_type_be_tupled(&*field_def.ty) {
                        field_def.ty.kind = self.tuple_type(&field_def.ty);
                    }
                }
            }

            // TODO: method defs etc...
            // TODO: create stubs for untracked functions?
            _ => {}
        }

        mut_visit::walk_item(self, item);
    }
}

impl ModifyParamsVisitor {
    pub fn new() -> Self {
        Self {
            modified_functions: HashSet::new(),
        }
    }

    /// Extract the set of functions this visitor has discovered and considered tracked
    pub fn get_modified_funcs(&self) -> &HashSet<String> {
        &self.modified_functions
    }

    /// Converts a type T into a TaggedValue<T>
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
}
