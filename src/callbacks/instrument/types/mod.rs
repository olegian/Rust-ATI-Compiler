//! Defines the type transformation function, [`recursively_transform_ast_type`], which
//! tuples all types within some AST type, in place.
//!
//! Atomic primitive types (specified by [`CanBeTupled`]) become Tagged<T>'s.
//! References to atomic primitive types become TaggedRef(Mut?)<T>s.
//! References to references, and references to compound types are untouched.
//! Arrays ([T; N]) become Tagged<[T; N]> (further recursively tupling the inner T).
//! Tuples and other aggregate types simply recurse the transformation into each inner T.
//!
//! Slices are special cased somewhat. Because !Sized types can only be constructed behind a
//! pointer, DATIR currently only supports slices that are stored behind references (as opposed
//! to other pointer types, like Box/Arc/etc). A reference to a slice (&[T]) becomes TaggedRef<[T]>,
//! after recursively tupling the inner type.

use crate::callbacks::types::CanBeTupled;

mod array;
mod path;
mod references;

/// Applies the recursive tupling transformations to `target_ty`, modifying it in-place.
pub fn recursively_transform_ast_type(target_ty: &mut rustc_ast::Ty) {
    // References must be checked before can_be_tupled: ast::Ty::can_be_tupled
    // peels refs, so `&u32` would otherwise be wrapped as Tagged<&u32>
    // instead of dispatched to the TaggedRef rewrite.
    if matches!(target_ty.kind, rustc_ast::TyKind::Ref(..)) {
        references::transform_reference(target_ty);
        return;
    }

    // we recursed down to a simple primitive!
    if target_ty.can_be_tupled() {
        transform_primitive(target_ty);
        return;
    }

    match &mut target_ty.kind {
        // Handled above.
        rustc_ast::TyKind::Ref(..) => unreachable!(),

        rustc_ast::TyKind::Array(..) => {
            array::transform_array(target_ty);
        }

        rustc_ast::TyKind::Slice(..) => {
            // at some point, have to implement other pointers pointing to a slice,
            // in which case this path might become relevant.
            unreachable!("Slice type should have been tupled during reference tupling.")
        }

        // [A, B, C] --> [Tag(A), Tag(B), Tag(C)]
        rustc_ast::TyKind::Tup(tys) => {
            for ty in tys {
                recursively_transform_ast_type(ty);
            }
        }

        rustc_ast::TyKind::Path(..) => {
            path::transform_path(target_ty);
        }

        // Explicit no-ops. There's nothing to be done here.
        rustc_ast::TyKind::Never
        | rustc_ast::TyKind::Infer
        | rustc_ast::TyKind::ImplicitSelf
        | rustc_ast::TyKind::Dummy
        | rustc_ast::TyKind::CVarArgs
        | rustc_ast::TyKind::Err(_) => {}

        // The following types have not been finished, but most likely,
        // they involve just pushing the operation down into any inner types.
        rustc_ast::TyKind::Ptr(rustc_ast::MutTy { ty: _, .. }) => {
            // e.g.
            // recursively_transform_ast_type(ty);
            unimplemented!();
        }
        rustc_ast::TyKind::PinnedRef(..) => unimplemented!(),
        rustc_ast::TyKind::FnPtr(..) => unimplemented!(),
        rustc_ast::TyKind::UnsafeBinder(..) => unimplemented!(),
        rustc_ast::TyKind::TraitObject(..) => unimplemented!(),
        rustc_ast::TyKind::ImplTrait(..) => unimplemented!(),
        rustc_ast::TyKind::Paren(..) => unimplemented!(),
        rustc_ast::TyKind::MacCall(..) => unimplemented!(),
        rustc_ast::TyKind::Pat(..) => unimplemented!(),
        rustc_ast::TyKind::FieldOf(..) => unimplemented!(),
    }
}

/// Converts an atomic primitive type T to a Tagged<T> inplace.
/// This is the base case op for the recursive tupling op.
pub(super) fn transform_primitive(ty: &mut rustc_ast::Ty) {
    ty.kind = rustc_ast::TyKind::Path(
        None,
        rustc_ast::Path {
            segments: [rustc_ast::PathSegment {
                ident: rustc_span::Ident::from_str("Tagged"),
                id: rustc_ast::DUMMY_NODE_ID,
                args: Some(Box::new(rustc_ast::AngleBracketed(
                    rustc_ast::AngleBracketedArgs {
                        span: rustc_span::DUMMY_SP,
                        args: [rustc_ast::AngleBracketedArg::Arg(
                            rustc_ast::GenericArg::Type(Box::new(ty.clone())),
                        )]
                        .into(),
                    },
                ))),
            }]
            .into(),
            span: rustc_span::DUMMY_SP,
            tokens: None,
        },
    );
}
