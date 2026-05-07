//! Defines how the [`AnalyzeHirVisitor`] records information about reference expressions.
//!
//! See the top-level comment in [crate::callbacks::gather::analyze_hir] for more information as to
//! why this is necessary.

use crate::{callbacks::gather::analyze_hir::AnalyzeHirVisitor, callbacks::types::CanBeTupled};

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    /// If the expression is a reference taken to some tuplable type or to an array or slice type,
    /// we need to record this expression a requiring a `.as_tagged_ref()` call added to it to
    /// convert it to a `TaggedRef<T>` rather than a `Tagged<T>`.
    pub fn observe_ref(&mut self, expr: &rustc_hir::Expr) {
        let rustc_hir::ExprKind::AddrOf(_, _, referant) = expr.kind else {
            panic!("Invoked observe_ref with non AddrOf expr {:?}", expr);
        };

        let ldid = expr.hir_id.owner.def_id;
        let typeck = self.tcx.typeck(ldid);
        let inner_ty = typeck.expr_ty(referant);

        // Note, its important to mirror `recursively_tuple_type` in instrument.rs here.
        // we want to call .as_tagged_ref() on types which are themselves tuplable, or on arrays
        // and slices which are represented in the semantic model by `TaggedRef<[T]>` (or
        // `TaggedRef<[T; N]>`) rather than by &Tagged<[T]> (or &Tagged<[T; N]>).
        let is_tagged_wrapped = inner_ty.can_be_tupled()
            || matches!(
                inner_ty.kind(),
                rustc_middle::ty::Array(..) | rustc_middle::ty::Slice(..)
            );

        if is_tagged_wrapped {
            self.first_pass
                .ref_to_tupleable_ty
                .mark(expr.span, self.tcx.sess.source_map());
        }
    }
}
