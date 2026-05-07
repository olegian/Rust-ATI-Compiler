//! Defines a function to transform a single Array/Repeat AST expression.
//!
//! This expression requires utilizing the runtime library's `ATI::track_array(<array>)` function,
//! to assign the length a dynamic Id.

use crate::callbacks::instrument::instrument::InstrumentingVisitor;

/// Invoked whenever the visitor runs into a ExprKind::Array
/// or ExprKind::Repeat. In this case, we have already transformed
/// the inner type, therefore at this point we just transform from
/// [a, b, c] --> ATI::track_array([a, b, c])
/// Expr type:  [T; N]  -->  Tagged<[Tag(T); N]>
pub fn transform_array(_visitor: &mut InstrumentingVisitor, array_expr: &mut rustc_ast::Expr) {
    let mut receiver_expr = rustc_ast::Expr::dummy();
    receiver_expr.kind = rustc_ast::ExprKind::Path(
        None,
        rustc_ast::Path {
            segments: [
                rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("ATI")),
                rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("track_array")),
            ]
            .into(),
            tokens: None,
            span: rustc_span::DUMMY_SP,
        },
    );

    let mut res = rustc_ast::Expr::dummy();
    res.kind = rustc_ast::ExprKind::Call(
        Box::new(receiver_expr),
        [Box::new(array_expr.clone())].into(),
    );

    *array_expr = res;
}
