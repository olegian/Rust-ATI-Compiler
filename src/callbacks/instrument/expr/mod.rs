//! Defines the expression transformation, calling [`transform_expr`] will recursively modify
//! the expression in-place to have the compiled binary track value interactions.
//!
//! Expression instrumentation heavily relies on the runtime library, the high-level goal is
//! shaping the AST in such a way that allows using the runtime library to dynamically assign
//! id's to values ("tupling", "tagging") and then rely on std::ops trait definitions to dispatch
//! operations on the various Tagged types, which merge appropriate ids within an `ATI_ANALYSIS`
//! global.
//!
//! Some expressions must be special-cased, or utilize facts gathered by the first
//! compilation, stored within [crate::callbacks::gather::first_pass_info]. The facts are made available
//! through the `visitor` parameter.
//!
//! Some key points:
//! - Assign / AssignOp nodes: these nodes always evaluate to (). When assinging an owned value to
//!   a variable which owns it's old value, the assignment works out of the box post-transformation.
//!   When performing an assignment through a dereferenced mutable reference however, both the
//!   value and the Id needs to be assigned to the referenced location. The first compilation
//!   has already found all code locations where this kind of assignment happens, and the
//!   assign expression will turn into a runtime library defined `TaggedRefMut::assign` method
//!   call. This method will write to both. Further, the lhs of an assign operation is a "place"
//!   expression, which needs to be instrumented differently than normal expressions. See
//!   [`transform_lhs_place_expr`] below for more information.
//! - Literals of type T are turned into Tagged<T> by dynamically assigning them a tag, via
//!   the runtime libraries `ATI::track(<lit>)`.
//! - Arrays are tracked via the runtime libraries `ATI::track_array(<array>)`.
//! - References (to tuplable primitives) are converted to TaggedRef / TaggedRefMuts via the
//!   `.as_tagged_ref()` defined within the runtime library.
//! - Calls to uninstrumented method and function calls have thier inputs "untupled", and
//!   return value tupled, if the call was found to return a tuplable value by the Gather pass.
//! - If/While conditions are appropriately untupled, to have the condition evaluate to a boolean
//!   after performing any merges required by the evaluation of the boolean itself.

use crate::callbacks::instrument::{instrument::InstrumentingVisitor, item::data_types};

mod addr_of;
mod array;
mod assign;
mod call;
mod common;
mod control_flow;
mod index;
mod literal;
mod ops;
mod range;

/// Mutates the input expression in place, to track value interactions during runtime.
pub fn transform_expr<'session>(
    visitor: &mut InstrumentingVisitor<'session>,
    expr: &mut rustc_ast::Expr,
) {
    // Assign / AssignOp are special-cased, the LHS is a *place* expression. Running the normal
    // value-expression instrumentation on it (e.g. rewriting `arr[i]` to `arr.subslice(..)`, etc.)
    // would turn the place into a value and break the assignment. So pre-walk, we instrument the
    // RHS as a value, but only walk the value-context subexpressions inside the LHS via
    // [`transform_lhs_place_expr`].
    if let rustc_ast::ExprKind::Assign(lhs, rhs, _) | rustc_ast::ExprKind::AssignOp(_, lhs, rhs) =
        &mut expr.kind
    {
        transform_lhs_place_expr(visitor, lhs);
        transform_expr(visitor, rhs);
        match &expr.kind {
            rustc_ast::ExprKind::Assign(..) => assign::transform_assign(visitor, expr),
            rustc_ast::ExprKind::AssignOp(..) => assign::transform_assign_op(visitor, expr),
            _ => unreachable!(),
        }
        return;
    }

    // instrument all other expressions in a post-fix order,
    // so that any inner expressions are transformed first.
    rustc_ast::mut_visit::walk_expr(visitor, expr);

    match &expr.kind {
        // Already handled above.
        rustc_ast::ExprKind::Assign(..) | rustc_ast::ExprKind::AssignOp(..) => unreachable!(),

        // <>
        rustc_ast::ExprKind::Lit(..) => {
            literal::transform_literal(visitor, expr);
        }

        // [ <>, <>, <> ] and [ <>; N ]
        rustc_ast::ExprKind::Array(..) | rustc_ast::ExprKind::Repeat(..) => {
            array::transform_array(visitor, expr);
        }

        // &<> and &mut <>
        rustc_ast::ExprKind::AddrOf(..) => {
            addr_of::transform_addr_of(visitor, expr);
        }

        // <func>(<>, <>, ...)
        rustc_ast::ExprKind::Call(..) => {
            call::transform_call(visitor, expr);
        }

        // <recv>.<method>(<>, <>, ...)
        rustc_ast::ExprKind::MethodCall(..) => {
            call::transform_method_call(visitor, expr);
        }

        // +, -, *, /, %, ||, &&, ^, &, |, <<, >>, ==, !=,  <, >, <=, >=
        rustc_ast::ExprKind::Binary(..) => {
            ops::transform_binary(visitor, expr);
        }

        // Deref, Not, Negation
        rustc_ast::ExprKind::Unary(_, _) => {
            ops::transform_unary(visitor, expr);
        }

        // if <> { <> }
        rustc_ast::ExprKind::If(..) => {
            control_flow::transform_if(visitor, expr);
        }

        // while <> { <> }
        rustc_ast::ExprKind::While(..) => {
            control_flow::transform_while(visitor, expr);
        }

        // expr in condition of if-let while-let
        rustc_ast::ExprKind::Let(..) => {
            control_flow::transform_let_condition(visitor, expr);
        }

        // for <> in <> { <> }
        rustc_ast::ExprKind::ForLoop { .. } => {
            control_flow::transform_for(visitor, expr);
        }

        // loop { <> }
        rustc_ast::ExprKind::Loop(..) => {
            control_flow::transform_loop(visitor, expr);
        }

        // match <> { <> => <> }
        rustc_ast::ExprKind::Match(..) => {
            control_flow::transform_match(visitor, expr);
        }

        // <>[<>]
        rustc_ast::ExprKind::Index(..) => {
            index::transform_index(visitor, expr);
        }

        // <>..<>
        rustc_ast::ExprKind::Range(..) => {
            range::transform_range(visitor, expr);
        }

        // |args| <body>
        rustc_ast::ExprKind::Closure(..) => {
            data_types::transform_closure(visitor, expr);
        }

        // No special transformation on the rest of these exprs
        rustc_ast::ExprKind::ConstBlock(..)
        | rustc_ast::ExprKind::Tup(..)
        | rustc_ast::ExprKind::Cast(..)
        | rustc_ast::ExprKind::Type(..)
        | rustc_ast::ExprKind::Block(..)
        | rustc_ast::ExprKind::Gen(..)
        | rustc_ast::ExprKind::Await(..)
        | rustc_ast::ExprKind::Use(..)
        | rustc_ast::ExprKind::TryBlock(..)
        | rustc_ast::ExprKind::Field(..)
        | rustc_ast::ExprKind::Underscore
        | rustc_ast::ExprKind::Path(..)
        | rustc_ast::ExprKind::Break(..)
        | rustc_ast::ExprKind::Continue(..)
        | rustc_ast::ExprKind::Ret(..)
        | rustc_ast::ExprKind::InlineAsm(..)
        | rustc_ast::ExprKind::OffsetOf(..)
        | rustc_ast::ExprKind::MacCall(..)
        | rustc_ast::ExprKind::Struct(..)
        | rustc_ast::ExprKind::Paren(..)
        | rustc_ast::ExprKind::Try(..)
        | rustc_ast::ExprKind::Yield(..)
        | rustc_ast::ExprKind::Yeet(..)
        | rustc_ast::ExprKind::Become(..)
        | rustc_ast::ExprKind::IncludedBytes(..)
        | rustc_ast::ExprKind::FormatArgs(..)
        | rustc_ast::ExprKind::UnsafeBinderCast(..)
        | rustc_ast::ExprKind::Err(..)
        | rustc_ast::ExprKind::Dummy => {}
    }
}

/// Walk a place expression (an Assign / AssignOp LHS) without value-instrumenting its
/// outer structure.
///
/// This entails recursing into value-context subexpressions, more concretely:
/// - `Index(base, idx, _)`: `base` is itself a place but `idx` is a value.
/// - `Unary(Deref, inner)`: `inner` is a value (the pointer being dereferenced).
/// - `Field(base, _)` / `Paren(inner)`: everything here is a place.
/// - `Tup(elems)`: tuple-destructuring assignment, everything here is a place.
/// - `Path(..)` / `Underscore`: No-op
///
/// This preserves place-ness so that the surrounding Assign / AssignOp remains
/// well-formed for the borrow checker, while still instrumenting any value-context
/// children (e.g. a Tagged<usize> index, or a `compute_ptr()` call inside `*..`).
fn transform_lhs_place_expr<'session>(
    visitor: &mut InstrumentingVisitor<'session>,
    lhs: &mut rustc_ast::Expr,
) {
    match &mut lhs.kind {
        rustc_ast::ExprKind::Index(base, idx, _) => {
            transform_lhs_place_expr(visitor, base);
            transform_expr(visitor, idx);
        }
        rustc_ast::ExprKind::Unary(rustc_ast::UnOp::Deref, inner) => {
            transform_lhs_place_expr(visitor, inner);
        }
        rustc_ast::ExprKind::Field(base, _) => {
            transform_lhs_place_expr(visitor, base);
        }
        rustc_ast::ExprKind::Paren(inner) => {
            transform_lhs_place_expr(visitor, inner);
        }
        rustc_ast::ExprKind::Tup(elems) => {
            for elem in elems {
                transform_lhs_place_expr(visitor, elem);
            }
        }
        // Leaf places / non-place LHS forms: leave alone.
        _ => {}
    }
}
