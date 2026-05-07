//! Defines a function to transform a single Assign or AssignOp AST expression.
//!
//! If the first compilation determines that this is assignment is assigning a value by
//! derefencing a mutable reference, then the assignment needs to utilize the
//! `TaggedRefMut::assign` method call, to write both the value and the id.

use rustc_ast_pretty::pprust;

use crate::{callbacks::instrument::instrument::InstrumentingVisitor, callbacks::parsing};

/// Invoked whenever the visitor runs into a ExprKind::Assign.
///
/// Assigning through a TaggedRefMut requires rewriting `*lhs = rhs` to
/// `lhs.assign(rhs)` so that both the id and the value are written.
/// Both `lhs` and `rhs` have already been instrumented by the caller
/// (the LHS via place-walk, which dispatches `transform_expr` on the
/// inner of any `Deref` it finds; the RHS via the normal value walk).
pub fn transform_assign(visitor: &mut InstrumentingVisitor, assign_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Assign(lhs, rhs, _) = &mut assign_expr.kind else {
        panic!(
            "Invoked transform_assign with non-assign expr: {:?}",
            pprust::expr_to_string(assign_expr)
        );
    };

    if !visitor
        .first_pass
        .assign_through_tagged_ref_mut
        .contains(assign_expr.span, visitor.psess.source_map())
    {
        return;
    }

    let rustc_ast::ExprKind::Unary(rustc_ast::UnOp::Deref, inner) = &mut lhs.kind else {
        return;
    };

    let code = format!(
        "{}.assign({})",
        pprust::expr_to_string(inner),
        pprust::expr_to_string(rhs),
    );
    *assign_expr = parsing::parse_expr(visitor.psess, code);
}

/// Invoked whenever the visitor runs into ExprKind::AssignOp.
///
/// Compound assignment through a TaggedRefMut (`*lhs OP= rhs`).
/// Plain DerefMut would only update the value field and leave the
/// id stale, so rewrite to read the current Tagged via field
/// projection, apply the binary form of the op, then write both
/// id and value back through `.assign()`.
pub fn transform_assign_op(
    visitor: &mut InstrumentingVisitor,
    assign_op_expr: &mut rustc_ast::Expr,
) {
    let rustc_ast::ExprKind::AssignOp(op, lhs, rhs) = &mut assign_op_expr.kind else {
        panic!(
            "Invoked transform_assign_op with non-assign-op expr: {:?}",
            pprust::expr_to_string(assign_op_expr)
        );
    };

    if !visitor
        .first_pass
        .assign_through_tagged_ref_mut
        .contains(assign_op_expr.span, visitor.psess.source_map())
    {
        return;
    }

    let rustc_ast::ExprKind::Unary(rustc_ast::UnOp::Deref, inner) = &mut lhs.kind else {
        return;
    };

    let bin_op: rustc_ast::BinOpKind = op.node.into();
    let code = format!(
        "{{ let mut __ati_lhs = {}; __ati_lhs.assign(Tagged(*__ati_lhs.0, *__ati_lhs.1) {} {}); }}",
        pprust::expr_to_string(inner),
        bin_op.as_str(),
        pprust::expr_to_string(rhs),
    );
    *assign_op_expr = parsing::parse_expr(visitor.psess, code);
}
