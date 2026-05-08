//! Defines functions to transform control flow expressions: `if`, `while`, `for`, `loop`, and `match`.
//!
//! Conditions must be untupled before being consumed by the statement, as long as the condition
//! evaluates to a `Tagged<bool>`. If the condition contains a pattern-matching let-binding, then
//! the condition will already be a simple `bool`.

use crate::callbacks::instrument::{expr::common, instrument_visitor::InstrumentingVisitor};

/// Invoked whenever the visitor runs into an `ExprKind::If`.
///
/// After Binary instrumentation, comparison conditions produce `Tagged<bool>`.
/// Unwrap to a raw `bool` so the if-condition compiles. Skip when the
/// condition is `if let` or a `&&` let-chain (already raw `bool` post-binary).
pub fn transform_if(_visitor: &mut InstrumentingVisitor, if_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::If(cond, _, _) = &mut if_expr.kind else {
        return;
    };
    if common::contains_let_chain(cond) {
        return;
    }
    common::untuple(cond);
}

/// Same as [`transform_if`], but for while loops.
pub fn transform_while(_visitor: &mut InstrumentingVisitor, while_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::While(cond, _, _) = &mut while_expr.kind else {
        return;
    };
    if common::contains_let_chain(cond) {
        return;
    }
    common::untuple(cond);
}

/// Force every ident binding in the for-loop pattern to `mut`. The deref /
/// reborrow rewrites that fire inside the loop body need a mutable binding
/// on `TaggedRefMut` operands, and we don't have type info at this stage to
/// tell which iterators yield mut refs. Over-marking is a warning at worst;
/// pass 2's output already silences `unused_mut` crate-wide.
pub fn transform_for(_visitor: &mut InstrumentingVisitor, for_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::ForLoop { pat, .. } = &mut for_expr.kind else {
        return;
    };
    common::pat_force_mut_bindings(pat);
}
pub fn transform_loop(_visitor: &mut InstrumentingVisitor, _loop_expr: &mut rustc_ast::Expr) {}
pub fn transform_match(_visitor: &mut InstrumentingVisitor, _match_expr: &mut rustc_ast::Expr) {}

// Handled in transform
pub fn transform_let_condition(
    _visitor: &mut InstrumentingVisitor,
    _let_condition: &mut rustc_ast::Expr,
) {
}
