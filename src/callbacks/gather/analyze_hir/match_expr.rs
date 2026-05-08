//! Defines how the [`AnalyzeHirVisitor`] records information about match expressions.
//!
//! See the top-level comment in [crate::callbacks::gather::analyze_hir] for more information as to
//! why this is necessary.

use crate::callbacks::{gather::analyze_hir::AnalyzeHirVisitor, types::CanBeTupled};

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    /// Records facts about a match expression for pass 2.
    ///
    /// Two distinct shapes can break a match after instrumentation:
    ///
    /// 1. The target itself is a tupleable type (a primitive, or a reference
    ///    to one). After instrumentation its type becomes `Tagged<T>` /
    ///    `TaggedRef<T>` / `TaggedRefMut<T>`, and the existing arm patterns
    ///    (which are `T`-shaped literals/ranges/etc.) can no longer match it.
    ///    Mark the target in `match_on_tagged` so pass 2 untuples it and
    ///    leaves the arms alone.
    ///
    /// 2. The target is a compound type whose shape is unchanged, but a
    ///    *primitive field* inside it gets promoted to `Tagged<T>`. Any
    ///    literal/range sub-pattern targeting that field then fails to
    ///    type-check (e.g. `MyEnum::V2(10)` where `V2`'s field is now
    ///    `Tagged<usize>`, but the pattern remains as just a `10: usize`). 
    ///    Mark each such sub-pattern in `tagged_lit_pat` so
    ///    pass 2 lifts it into a fresh binding plus a guard fragment.
    ///
    /// The two cases are disjoint, case 1 fires only when the
    /// target's peeled type is tupleable, case 2 only when it is not. This
    /// keeps the two pass-2 rewrites from stepping on each other.
    pub fn observe_match(&mut self, expr: &rustc_hir::Expr) {
        let rustc_hir::ExprKind::Match(target, arms, _match_kind) = expr.kind else {
            panic!("Invoked observe_match with non-match expr: {:?}", expr);
        };

        let ldid = target.hir_id.owner.def_id;
        let typeck = self.tcx.typeck(ldid);
        let target_ty = typeck.expr_ty(target);
        if target_ty.peel_refs().can_be_tupled() {
            self.first_pass
                .match_on_tagged
                .mark(target.span, self.tcx.sess.source_map());
            return;
        }

        // with a compound target, look for literal/range sub-patterns whose
        // position will become `Tagged<T>` post-transform.
        for arm in arms {
            arm.pat.walk(|p| {
                let kind_is_lit_or_range = matches!(
                    p.kind,
                    rustc_hir::PatKind::Expr(_) | rustc_hir::PatKind::Range(..)
                );
                if kind_is_lit_or_range && typeck.pat_ty(p).can_be_tupled() {
                    self.first_pass
                        .tagged_lit_pat
                        .mark(p.span, self.tcx.sess.source_map());
                }
                true
            });
        }
    }
}
