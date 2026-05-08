//! Defines a function to transform a single function or method call AST expression.
//!
//! If the first pass determined that this expression is an invocation of an untracked function,
//! then all inputs need to be untupled, and the return value (might) need tupling.
//!
//! The Path which identifies the function being invoked could also have generic types within
//! it, which require tupleing as well.

use crate::callbacks::instrument::{expr::common, instrument_visitor::InstrumentingVisitor, types};

/// Invoked whenever the visitor runs into a ExprKind::Call.
///
/// Updates turbofish generics (`f::<u32>` -> `f::<Tagged<u32>>`).
/// If pass 1 marked this as an untracked call, untuples each argument
/// (`x` -> `x.1`) in place and, if the return is tupleable, wraps the
/// call in `ATI::track(...)`.
pub fn transform_call(visitor: &mut InstrumentingVisitor, call_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Call(func, args) = &mut call_expr.kind else {
        return;
    };

    let rustc_ast::ExprKind::Path(_, path) = &mut func.kind else {
        return;
    };

    for segment in path.segments.iter_mut() {
        tuple_generic_args_in_segment(segment);
    }

    let Some(call) = visitor
        .first_pass
        .untracked_fn_calls
        .get(func.span, visitor.psess.source_map())
    else {
        return;
    };
    let ret_tupleable = call.ret_is_tupleable;

    for arg_expr in args.iter_mut() {
        common::untuple(arg_expr);
    }

    // FIXME: again, this is a bit wrong. We are currently ignoring the tracked/untracked
    // boundary, but you can imagine that an untracked func call returns some struct, which
    // itself contains values that need to be converted into Tagged<T>s. Right now, that
    // case is entirely ignored, this works properly if the returned value is a simple
    // primitive.
    if ret_tupleable {
        common::tuple(call_expr);
    }
}

/// Invoked whenever the visitor runs into ExprKind::MethodCall.
///
/// Updates turbofish generics on the method segment.
pub fn transform_method_call(
    _visitor: &mut InstrumentingVisitor,
    method_expr: &mut rustc_ast::Expr,
) {
    let rustc_ast::ExprKind::MethodCall(box rustc_ast::MethodCall { seg, .. }) =
        &mut method_expr.kind
    else {
        return;
    };
    tuple_generic_args_in_segment(seg);
}

/// Recursively transforms all type generic arguments in a path segment.
fn tuple_generic_args_in_segment(segment: &mut rustc_ast::PathSegment) {
    let Some(boxed_args) = &mut segment.args else {
        return;
    };
    let rustc_ast::GenericArgs::AngleBracketed(rustc_ast::AngleBracketedArgs {
        ref mut args, ..
    }) = **boxed_args
    else {
        return;
    };
    for arg in args.iter_mut() {
        if let rustc_ast::AngleBracketedArg::Arg(rustc_ast::GenericArg::Type(ty)) = arg {
            types::recursively_transform_ast_type(ty);
        }
    }
}
