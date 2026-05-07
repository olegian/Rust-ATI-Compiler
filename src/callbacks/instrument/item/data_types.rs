//! Defines the transformation done to all user-defined compound type definitions.
//!
//! Structs and tuples have thier field types recursively tupled, as defined in
//! [crate::callbacks::instrument::types].

use crate::callbacks::instrument::{instrument::InstrumentingVisitor, types};

/// Tuples all tupleable types in the struct's fields.
pub fn transform_struct(_visitor: &mut InstrumentingVisitor, struct_item: &mut rustc_ast::Item) {
    let rustc_ast::ItemKind::Struct(_, _, variant_data) = &mut struct_item.kind else {
        return;
    };

    let fields = match variant_data {
        rustc_ast::VariantData::Struct { fields, .. } => fields,
        rustc_ast::VariantData::Tuple(fields, _) => fields,
        rustc_ast::VariantData::Unit(_) => return,
    };

    for field_def in fields.iter_mut() {
        types::recursively_transform_ast_type(&mut field_def.ty);
    }
}

/// Tuples all tupleable types in every variant of the enum.
pub fn transform_enum(_visitor: &mut InstrumentingVisitor, enum_item: &mut rustc_ast::Item) {
    let rustc_ast::ItemKind::Enum(_ident, _, rustc_ast::EnumDef { variants }) = &mut enum_item.kind
    else {
        return;
    };

    for variant in variants.iter_mut() {
        match &mut variant.data {
            rustc_ast::VariantData::Struct { fields, .. } => {
                for field in fields.iter_mut() {
                    types::recursively_transform_ast_type(&mut field.ty);
                }
            }
            rustc_ast::VariantData::Tuple(fields, _) => {
                for field in fields.iter_mut() {
                    types::recursively_transform_ast_type(&mut field.ty);
                }
            }
            rustc_ast::VariantData::Unit(_) => {}
        }
    }
}

/// Tuples input/output types of a closure expression. The body has
/// already been walked by the default walk_expr; this only updates the
/// fn_decl signature.
pub fn transform_closure(_visitor: &mut InstrumentingVisitor, closure_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Closure(box rustc_ast::Closure { fn_decl, .. }) =
        &mut closure_expr.kind
    else {
        panic!(
            "Invoked transform_closure with non-closure expr: {:?}",
            rustc_ast_pretty::pprust::expr_to_string(closure_expr)
        );
    };

    for input in fn_decl.inputs.iter_mut() {
        types::recursively_transform_ast_type(&mut input.ty);
    }
    if let rustc_ast::FnRetTy::Ty(ty) = &mut fn_decl.output {
        types::recursively_transform_ast_type(ty);
    }
}
