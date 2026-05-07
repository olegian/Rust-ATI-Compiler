//! Defines how references are transformed amidst the recursive tupling operation.
//!
//! A reference to an atomic primitive (&'a T) must become a TaggedRef<'a, T> (mutable, if
//! necessary).
//!
//! A reference to a reference, or a reference to  a non-tuplable type T must remain unchanged.
//!
//! A reference to a slice must be converted to a TaggedRef<[T]>, with the inner type being
//! recursively tupled.
//!
//! See [`recursively_transform_ast_type`] for more information on reccursive tupling.

use rustc_ast_pretty::pprust;

use crate::{
    callbacks::instrument::types::recursively_transform_ast_type, callbacks::types::CanBeTupled,
};

/// Recursively transforms a reference type, taking &T -> TaggedRef<Tag(T)> when necessary,
/// alongside it's mutable counterpart.
pub fn transform_reference(target_ty: &mut rustc_ast::Ty) {
    let rustc_ast::TyKind::Ref(_lt, rustc_ast::MutTy { box ty, mutbl }) = &mut target_ty.kind
    else {
        panic!(
            "Invoked transform_reference with non-reference type as input: {:?}",
            pprust::ty_to_string(target_ty)
        );
    };

    // Nested refs (e.g. &&u32, &&[u32]): only the innermost &
    // carries an Id. Recurse down into the inner Ref.
    if matches!(
        ty.kind,
        rustc_ast::TyKind::Ref(..) | rustc_ast::TyKind::Ptr(..)
    ) {
        recursively_transform_ast_type(ty);
        return;
    }

    // target_ty = &prim | &[T] | &[T; N]: the outer & gets swallowed into a
    // TaggedRef(Mut)? wrapper. For slices and arrays, tuple the element
    // type in place first so the inner shape becomes [Tag(T)] / [Tag(T); N].
    let mutable = mutbl.is_mut();
    match ty.kind {
        rustc_ast::TyKind::Slice(ref mut elem_ty)
        | rustc_ast::TyKind::Array(ref mut elem_ty, _) => {
            // target_ty = &[T], convert to &[Tag(T)], then to TaggedRef<[Tag(T)]>
            recursively_transform_ast_type(elem_ty);
            wrap_ty_as_tagged_ref(target_ty, mutable);
        }
        _ if ty.can_be_tupled() => {
            // target_ty = &prim, convert to TaggedRef<prim>
            wrap_ty_as_tagged_ref(target_ty, mutable);
        }
        _ => {
            // target_ty = non-primitive, non-slice type.
            recursively_transform_ast_type(ty);
        }
    }
}

/// Modifies in place a type `T` into `TaggedRef(Mut?)<T>`.
/// The caller is responsible for having already
/// tupled any sub-element types (e.g. the element type of a slice/array);
/// this helper only wraps the outer shape.
fn wrap_ty_as_tagged_ref(outer_ty: &mut rustc_ast::Ty, mutable: bool) {
    // extract the referent and preserve the source
    // lifetime so `&'a T` becomes `TaggedRef<'a, T>`.
    let (lifetime, inner) = match &mut outer_ty.kind {
        rustc_ast::TyKind::Ref(lt, rustc_ast::MutTy { box ty, .. }) => (lt.clone(), ty.clone()),
        _ => panic!(
            "Trying to convert a non-ref type to a TaggedRef: {:?}",
            pprust::ty_to_string(outer_ty)
        ),
    };

    // Construct new name of wrapped type
    let name = if mutable { "TaggedRefMut" } else { "TaggedRef" };
    let mut seg = rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str(name));

    // Construct generic type parameters of TaggedRef
    let mut args: Vec<rustc_ast::AngleBracketedArg> = Vec::new();
    if let Some(lt) = lifetime {
        args.push(rustc_ast::AngleBracketedArg::Arg(
            rustc_ast::GenericArg::Lifetime(lt),
        ));
    }
    args.push(rustc_ast::AngleBracketedArg::Arg(
        rustc_ast::GenericArg::Type(Box::new(inner)),
    ));
    seg.args = Some(Box::new(rustc_ast::GenericArgs::AngleBracketed(
        rustc_ast::AngleBracketedArgs {
            span: rustc_span::DUMMY_SP,
            args: args.into(),
        },
    )));

    // Write transformed type into target
    outer_ty.kind = rustc_ast::TyKind::Path(
        None,
        rustc_ast::Path {
            span: rustc_span::DUMMY_SP,
            segments: [seg].into(),
            tokens: None,
        },
    );
}
