//! Defines how arrays are transformed amidst the recursive tupling operation.
//!
//! See [`recursively_transform_ast_type`].

use rustc_ast_pretty::pprust;

use crate::callbacks::instrument::types::recursively_transform_ast_type;

/// Converts an array type [T; N] --> Tagged<[Tag(T); N]>.
pub fn transform_array(target_ty: &mut rustc_ast::Ty) {
    let rustc_ast::TyKind::Array(ty, _) = &mut target_ty.kind else {
        panic!(
            "Invoked transform_array with non-array type as input: {:?}",
            pprust::ty_to_string(target_ty)
        );
    };

    recursively_transform_ast_type(ty);

    let mut tagged_array =
        rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("Tagged"));
    tagged_array.args = Some(Box::new(rustc_ast::GenericArgs::AngleBracketed(
        rustc_ast::AngleBracketedArgs {
            span: rustc_span::DUMMY_SP,
            args: [rustc_ast::AngleBracketedArg::Arg(
                rustc_ast::GenericArg::Type(Box::new(target_ty.clone())),
            )]
            .into(),
        },
    )));

    target_ty.kind = rustc_ast::TyKind::Path(
        None,
        rustc_ast::Path {
            span: rustc_span::DUMMY_SP,
            segments: [tagged_array].into(),
            tokens: None,
        },
    );
}
