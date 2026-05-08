//! Defines functions to transform binary and unary operation expressions.
//!
//! In terms of value interaction tracking, there are three types of binary operations:
//! 1. Logical operators (`||`, `&&`). These operators produce no interactions. If the operation is
//!    an `&&`, then the lhs and rhs could evaluate to either a `Tagged<bool>` or raw `bool`,
//!    depending on whether or not the lhs or rhs expression contains a let chain. If a let chain
//!    exists in either side, then we must be in the top level of either an `If` or `While`
//!    condition. This means it is safe to untuple any `Tagged<bool>`, as the overall binary
//!    expression evaluation will not interact with any value.
//! 2. Comparison operators (`==`, `>`, `<=`, etc...). These operators produce an interaction
//!    between the lhs and rhs, and the resulting boolean is a new value which receives a new Id.
//! 3. Arithmetic operators (`+`, `&`, etc...). These operators produce an interaction between the
//!    lhs, rhs, and the output. These operators rely on `std::ops` trait implementations to perform
//!    both tag merging and value computation.
//!
//! For unary operators:
//! 1. Deref could require reconstruction of a `Tagged<T>` from a `TaggedRef(Mut?)<T>`, if the first
//!    pass determined it was necessary (see `./deref.rs` in
//!    `crate::callbacks::gather::analyze_hir`).
//! 2. Negation and logical not both just get pushed down into the contained value within the
//!    `Tagged<T>`.

use rustc_ast::BinOpKind;
use rustc_ast_pretty::pprust;

use crate::{
    callbacks::instrument::{expr::common as expr_common, instrument::InstrumentingVisitor},
    callbacks::parsing,
};

enum OpKind {
    Logical,
    Comparison,
    Arithmetic,
}

/// Invoked whenever the visitor runs into an `ExprKind::Binary`.
///
/// Transforms `lhs op rhs` (both `Tagged<T>`) into a block that
/// explicitly calls `ATI_ANALYSIS` to record the interaction and
/// constructs the result.
pub fn transform_binary(visitor: &mut InstrumentingVisitor, binary_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Binary(op, lhs, rhs) = &binary_expr.kind else {
        return;
    };

    // Let-chain: `<expr> && let PAT = ...` (possibly nested in `&&` chains).
    // Keep the Binary intact so the Let stays in a syntactically valid slot
    // and the chain types out as raw `bool`. Untuple any non-let-chain
    // operand (a Tagged<bool>) so `&&` sees `bool` on both sides.
    if op.node == rustc_ast::BinOpKind::And
        && (expr_common::contains_let_chain(lhs) || expr_common::contains_let_chain(rhs))
    {
        let rustc_ast::ExprKind::Binary(_, lhs, rhs) = &mut binary_expr.kind else {
            unreachable!();
        };
        if !expr_common::contains_let_chain(lhs) {
            expr_common::untuple(&mut **lhs);
        }
        if !expr_common::contains_let_chain(rhs) {
            expr_common::untuple(&mut **rhs);
        }
        return;
    }

    // The block we emit binds lhs/rhs into locals, which moves them.
    // For TaggedRefMut operands (move-only), reborrow first so any later
    // use of the same source binding still compiles.
    let rustc_ast::ExprKind::Binary(_, lhs, rhs) = &mut binary_expr.kind else {
        unreachable!();
    };
    expr_common::reborrow_if_ref_mut(visitor, &mut **lhs);
    expr_common::reborrow_if_ref_mut(visitor, &mut **rhs);

    let rustc_ast::ExprKind::Binary(op, lhs, rhs) = &binary_expr.kind else {
        unreachable!();
    };
    let lhs_str = pprust::expr_to_string(lhs);
    let rhs_str = pprust::expr_to_string(rhs);
    let op_str = op.node.as_str();

    // FIXME: Kind of stupid to go from lhs op rhs to lhs op rhs in arithmetic case
    let block_str = match op_type(op.node) {
        // Comparisons interact the two operands but not the result.
        OpKind::Comparison => format!(
            r#"{{
                let __ati_lhs = {lhs_str};
                let __ati_rhs = {rhs_str};
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&__ati_lhs.0, &__ati_rhs.0);
                let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                Tagged(__ati_id, __ati_lhs.1 {op_str} __ati_rhs.1)
            }}"#
        ),
        // Logical &&/|| are not interactions, simply unwrap and assign a new id to result
        OpKind::Logical => format!(
            r#"{{
                let __ati_lhs = {lhs_str};
                let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                Tagged(__ati_id, __ati_lhs.1 {op_str} ({rhs_str}).1)
            }}"#
        ),
        // These will all interact through ops trait impls.
        OpKind::Arithmetic => format!(
            r#"{{
                ({lhs_str} {op_str} {rhs_str})
            }}"#
        ),
    };

    *binary_expr = parsing::parse_expr(visitor.psess, block_str);
}

/// Invoked whenever the visitor runs into an `ExprKind::Unary`.
///
/// Unary `*` on an instrumented `&T` / `&mut T` with tupleable `T`: post-
/// instrumentation the operand is a `TaggedRef(Mut?)<T>`, and a plain `*`
/// would strip the tag (`TaggedRef::deref` to `&T`). Rebuild a `Tagged<T>` from
/// the borrowed fields so the id travels with the value.
pub fn transform_unary(visitor: &mut InstrumentingVisitor, unary_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Unary(rustc_ast::UnOp::Deref, _) = &unary_expr.kind else {
        return;
    };

    if !visitor
        .first_pass
        .tag_stripping_deref
        .contains(unary_expr.span, visitor.psess.source_map())
    {
        return;
    }

    // The emitted block moves `inner` into __tr. Reborrow first if it's
    // a TaggedRefMut so subsequent uses of the same source binding survive.
    let rustc_ast::ExprKind::Unary(_, inner) = &mut unary_expr.kind else {
        unreachable!();
    };
    expr_common::reborrow_if_ref_mut(visitor, &mut **inner);

    let rustc_ast::ExprKind::Unary(_, inner) = &unary_expr.kind else {
        unreachable!();
    };
    let code = format!(
        "{{ let __tr = {}; Tagged(*__tr.0, *__tr.1) }}",
        pprust::expr_to_string(inner),
    );
    *unary_expr = parsing::parse_expr(visitor.psess, code);
}

fn op_type(op: BinOpKind) -> OpKind {
    match op {
        BinOpKind::Eq
        | BinOpKind::Ne
        | BinOpKind::Lt
        | BinOpKind::Gt
        | BinOpKind::Le
        | BinOpKind::Ge => OpKind::Comparison,

        BinOpKind::BitXor
        | BinOpKind::BitAnd
        | BinOpKind::BitOr
        | BinOpKind::Shl
        | BinOpKind::Shr
        | BinOpKind::Add
        | BinOpKind::Sub
        | BinOpKind::Mul
        | BinOpKind::Div
        | BinOpKind::Rem => OpKind::Arithmetic,

        BinOpKind::And | BinOpKind::Or => OpKind::Logical,
    }
}
