//! Defines functions to transform Index expressions.
//!
//! Currently, unused.

use crate::callbacks::instrument::instrument::InstrumentingVisitor;

/// Invoked whenever the visitor runs into a ExprKind::Index.
///
/// Index expressions don't need direct instrumentation. The
/// index-by-range case is handled at the surrounding AddrOf, since we
/// need to know whether the borrow is mutable to dispatch the right
/// `subslice(_mut)?` method. That might not be the best way to approach it though?
pub fn transform_index(_visitor: &mut InstrumentingVisitor, _index_expr: &mut rustc_ast::Expr) {}
