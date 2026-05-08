//! Defines a function to transform a single Array/Repeat AST expression.
//!
//! This expression requires utilizing the runtime library's `ATI::track(<array>)` function,
//! to assign the length a dynamic Id.

use crate::callbacks::instrument::{expr::common, instrument::InstrumentingVisitor};

/// Invoked whenever the visitor runs into a ExprKind::Array
/// or ExprKind::Repeat. In this case, we have already transformed
/// the inner type, therefore at this point we just transform from
/// `[a, b, c] --> ATI::track([a, b, c])`. The expr type is changed from 
/// `[T; N]  -->  Tagged<[Tag(T); N]>`.
pub fn transform_array(_visitor: &mut InstrumentingVisitor, array_expr: &mut rustc_ast::Expr) {
    // Before, array tracking was a much more complicated operation.
    // It has been simplified to just this line, but I am going to leave the 
    // overall function just in case I ever need to increase the complexity again.

    common::tuple(array_expr);
}
