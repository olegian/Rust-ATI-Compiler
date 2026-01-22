use std::collections::HashMap;

use rustc_ast as ast;
use rustc_ast::mut_visit::MutVisitor;
use rustc_span::{DUMMY_SP, Ident, Symbol};

use crate::instrumentation::common::{self, FnInfo};

pub struct UpdateFnDeclsVisitor {
    modified_functions: HashMap<String, FnInfo>,
}

impl MutVisitor for UpdateFnDeclsVisitor {
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
                let mut params = Vec::new();
                // go through parameters of function...
                for param in &mut decl.inputs {
                    let ty = &mut param.ty;
                    if common::can_type_be_tupled(ty) {
                        // ... if type can be tupled, we need to convert the type to be
                        // a TaggedValue<ty> to carry tracking info
                        ty.kind = self.tuple_type(ty);
                    }

                    params.push(Box::new(param.clone()));
                }

                if let ast::FnRetTy::Ty(ref mut return_type) = decl.output {
                    if common::can_type_be_tupled(return_type) {
                        // if return type exists and should also be tupled
                        return_type.kind = self.tuple_type(return_type);
                    }
                }

                // store required info so that we can later make a stub version of this later
                self.modified_functions.insert(
                    ident.as_str().into(),
                    FnInfo {
                        params,
                        return_ty: Box::new(decl.output.clone()),
                    },
                );

                // rename the function to be *_unstubbed so the stub can call it
                let old_ident = ident.as_str();
                *ident = Ident::from_str(&format!("{old_ident}_unstubbed"));
            }

            // Tags all values in struct that can be tupled
            ast::ItemKind::Struct(_, _, ast::VariantData::Struct { ref mut fields, .. }) => {
                for field_def in fields {
                    if common::can_type_be_tupled(&*field_def.ty) {
                        field_def.ty.kind = self.tuple_type(&field_def.ty);
                    }
                }
            }

            // TODO: method defs in impl blocks?
            _ => {}
        }
    }
}

impl UpdateFnDeclsVisitor {
    pub fn new() -> Self {
        Self {
            modified_functions: HashMap::new(),
        }
    }

    /// Extract the set of functions this visitor has discovered and considered tracked
    pub fn get_modified_funcs(&self) -> &HashMap<String, FnInfo> {
        &self.modified_functions
    }

    /// Converts a type T into a TaggedValue<T>
    fn tuple_type(&self, old_type: &ast::Ty) -> ast::TyKind {
        ast::TyKind::Path(
            None,
            ast::Path {
                segments: [ast::PathSegment {
                    ident: Ident::from_str("TaggedValue"),
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
