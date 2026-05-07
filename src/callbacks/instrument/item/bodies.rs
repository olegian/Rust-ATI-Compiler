//! This file defines the transformation performed on each Item that has a body: Functions,
//! Methods, and Traits.
//!
//! Input and return types are recursively tupled, as defined by
//! [types::recursively_transform_ast_type].
//!
//! Bodies are walked and transformed, via the transformation defined in [crate::callbacks::instrument::expr].
//!
//! If the Instrument compilation encounters a function or method which the Gather pass determined
//! should not be instrumented (via [crate::callbacks::gather::first_pass_info]), then the function is skipped.
//!
//! Further, given that the Instrument compilation happens after the Gather compilation is able to
//! run all code analysis, we know that the compiled crate is semantically correct. Therefore, it
//! is safe to make every variable binding `mut`, which allows for assigning to values via
//! TaggedRefMut's. This is currently a patch solution, it is simple, works, and maintains
//! correctness.

use rustc_ast_pretty::pprust;

use crate::callbacks::gather::{first_pass_info::FnNamespace, type_key};
use crate::callbacks::instrument::{instrument::InstrumentingVisitor, types};

/// Walks the body, then wraps parameter and return types in `Tagged<T>`
/// for free functions that pass 1 observed.
pub fn transform_fn(visitor: &mut InstrumentingVisitor, fn_item: &mut rustc_ast::Item) {
    let rustc_ast::ItemKind::Fn(box rustc_ast::Fn {
        ident,
        sig: rustc_ast::FnSig { decl, .. },
        body,
        ..
    }) = &mut fn_item.kind
    else {
        return;
    };

    // skip functions that are considered untracked.
    if visitor
        .first_pass
        .fns
        .lookup(&visitor.mod_path, FnNamespace::Free, ident.as_str())
        .is_none()
    {
        return;
    }

    // instrument the function body
    if let Some(body) = body {
        rustc_ast::mut_visit::walk_block(visitor, body);
    }

    for param in &mut decl.inputs {
        // make every parameter binding mutable...
        if matches!(
            param.ty.kind,
            rustc_ast::TyKind::Ref(
                _,
                rustc_ast::MutTy {
                    mutbl: rustc_ast::Mutability::Mut,
                    ..
                }
            )
        ) {
            let rustc_ast::PatKind::Ident(mode, _, _) = &mut param.pat.kind else {
                panic!(
                    "Mut-ref parameter has non-ident pattern: {:?}",
                    pprust::pat_to_string(&param.pat)
                );
            };
            mode.1 = rustc_ast::Mutability::Mut;
        }

        // ... and recursively tuple the input types.
        types::recursively_transform_ast_type(&mut param.ty);
    }

    if let rustc_ast::FnRetTy::Ty(return_type) = &mut decl.output {
        // recursively tuple the return type, if it exists.
        types::recursively_transform_ast_type(return_type);
    }
}

/// Walks the body of every method that pass 1 observed in this impl,
/// then wraps parameter and return types. This function is very similar to the above
/// `transform_fn`, but requires a slightly different lookup as the method is defined
/// within the self type's namespace.
pub fn transform_impl(visitor: &mut InstrumentingVisitor, impl_item: &mut rustc_ast::Item) {
    let rustc_ast::ItemKind::Impl(rustc_ast::Impl {
        of_trait,
        self_ty,
        items,
        ..
    }) = &mut impl_item.kind
    else {
        return;
    };

    let type_key =
        type_key::TypeKey::try_from_ast(of_trait.as_deref().map(|h| &h.trait_ref), self_ty)
            .unwrap_or_else(|| {
                panic!(
                    "instrumentation could not derive TypeKey from impl self-type \
                 `{}` in module `{}`; only path self/trait types are supported",
                    pprust::ty_to_string(self_ty),
                    visitor.mod_path,
                )
            });

    for assoc_item in items.iter_mut() {
        let rustc_ast::AssocItemKind::Fn(box rustc_ast::Fn {
            ident,
            sig: rustc_ast::FnSig { decl, .. },
            body,
            ..
        }) = &mut assoc_item.kind
        else {
            continue;
        };

        if visitor
            .first_pass
            .fns
            .lookup(
                &visitor.mod_path,
                FnNamespace::Method(&type_key),
                ident.as_str(),
            )
            .is_none()
        {
            continue;
        }

        if let Some(body) = body {
            rustc_ast::mut_visit::walk_block(visitor, body);
        }

        for param in &mut decl.inputs {
            if !matches!(param.ty.peel_refs().kind, rustc_ast::TyKind::ImplicitSelf) {
                types::recursively_transform_ast_type(&mut param.ty);
            }
        }

        if let rustc_ast::FnRetTy::Ty(ret_ty) = &mut decl.output {
            types::recursively_transform_ast_type(ret_ty);
        }
    }
}

/// Transforms a trait definition.
pub fn transform_trait(_visitor: &mut InstrumentingVisitor, _trait_item: &mut rustc_ast::Item) {
    // TODO: trait items aren't instrumented yet.
}
