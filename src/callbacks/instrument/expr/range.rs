//! Defines a function to transform a range to DATIR's range representation, using the
//! runtime library's `ATI::track_range*` calls.

use rustc_ast_pretty::pprust;

use crate::{callbacks::instrument::instrument::InstrumentingVisitor, callbacks::parsing};

/// Invoked whenever the visitor runs into an `ExprKind::Range`.
///
/// Transform range construction into a tracked-range constructor call.
/// By this point `walk_expr` has already instrumented the endpoints (so
/// literals/vars are `Tagged<T>`).
pub fn transform_range(visitor: &mut InstrumentingVisitor, range_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::Range(lo, hi, limits) = &range_expr.kind else {
        return;
    };

    let is_inclusive = matches!(limits, rustc_ast::RangeLimits::Closed);
    let code = match (lo.as_ref(), hi.as_ref(), is_inclusive) {
        (Some(lo), Some(hi), false) => format!(
            "ATI::track_range({}, {})",
            pprust::expr_to_string(lo),
            pprust::expr_to_string(hi),
        ),
        (Some(lo), Some(hi), true) => format!(
            "ATI::track_range_inclusive({}, {})",
            pprust::expr_to_string(lo),
            pprust::expr_to_string(hi),
        ),
        (Some(lo), None, _) => {
            format!("ATI::track_range_from({})", pprust::expr_to_string(lo))
        }
        (None, Some(hi), false) => {
            format!("ATI::track_range_to({})", pprust::expr_to_string(hi))
        }
        (None, Some(hi), true) => format!(
            "ATI::track_range_to_inclusive({})",
            pprust::expr_to_string(hi),
        ),
        (None, None, _) => "ATI::track_range_full()".to_string(),
    };
    *range_expr = parsing::parse_expr(visitor.psess, code);
}
