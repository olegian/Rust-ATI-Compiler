//! Defines functions to transform control flow expressions: `if`, `while`, `for`, `loop`, and `match`.
//!
//! Conditions must be untupled before being consumed by the statement, as long as the condition
//! evaluates to a `Tagged<bool>`. If the condition contains a pattern-matching let-binding, then
//! the condition will already be a simple `bool`.

use rustc_ast_pretty::pprust;

use crate::callbacks::{
    instrument::{
        expr::common::{self, lift_lit_pats},
        instrument_visitor::InstrumentingVisitor,
    },
    parsing,
};

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

/// Pass 2's response to a `match` expression. Two structurally distinct
/// rewrites, controlled by what pass 1 marked (see
/// `crate::callbacks::gather::analyze_hir::match_expr` for the analysis):
///
/// 1. If the target was tagged in `match_on_tagged`, the whole target became
///    `Tagged<T>` / `TaggedRef<T>` / `TaggedRefMut<T>` after instrumentation.
///    Untuple it via `.1` so the existing arm patterns continue to match
///    against the underlying `T`.
///
/// 2. Otherwise the target is a compound type, but individual arm
///    sub-patterns may have been marked in `tagged_lit_pat`: literal/range
///    patterns whose position became `Tagged<T>` (e.g. `MyEnum::V2(10)` once
///    `V2`'s field is `Tagged<usize>`). For each such sub-pattern, replace it
///    with a fresh `ref __ati_pat_N` binding and append a guard fragment
///    `matches!(**__ati_pat_N, <orig>)` that re-checks the
///    original pattern against the dereferenced inner value. Guard fragments
///    are AND-combined with each other and with any pre-existing arm guard.
pub fn transform_match(visitor: &mut InstrumentingVisitor, match_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Match(target, arms, _kind) = &mut match_expr.kind else {
        return;
    };

    if visitor
        .first_pass
        .match_on_tagged
        .contains(target.span, visitor.psess.source_map())
    {
        common::untuple(target);
        return;
    }

    for arm in arms {
        let mut counter: usize = 0;
        let mut frags: Vec<String> = Vec::new();
        lift_lit_pats(visitor, &mut arm.pat, &mut counter, &mut frags);
        if frags.is_empty() {
            continue;
        }

        let combined = frags.join(" && ");
        let new_cond_str = match &arm.guard {
            Some(g) => format!("({}) && ({})", pprust::expr_to_string(&g.cond), combined),
            None => combined,
        };
        let new_cond = parsing::parse_expr(visitor.psess, new_cond_str);
        arm.guard = Some(Box::new(rustc_ast::Guard {
            cond: new_cond,
            span_with_leading_if: rustc_span::DUMMY_SP,
        }));
    }
}

// Handled in transform
pub fn transform_let_condition(
    _visitor: &mut InstrumentingVisitor,
    _let_condition: &mut rustc_ast::Expr,
) {
}
