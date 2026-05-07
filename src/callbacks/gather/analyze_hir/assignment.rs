//! Defines how the [`AnalyzeHirVisitor`] records information about assignment expressions.
//!
//! See the top-level comment in [crate::callbacks::gather::analyze_hir] for more information as to
//! why this is necessary.

use crate::{callbacks::gather::analyze_hir::AnalyzeHirVisitor, callbacks::types::CanBeTupled};

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    /// If the assignment happens to a dereferenced mutable reference that refers to some
    /// tuplable type, then mark this expression as requiring the assignment to write
    /// both the value and the tag.
    pub fn observe_assignment(&mut self, expr: &rustc_hir::Expr) {
        let (rustc_hir::ExprKind::Assign(lhs, _, _) | rustc_hir::ExprKind::AssignOp(_, lhs, _)) =
            expr.kind
        else {
            panic!(
                "Invoked observe_assignment with non-assign or assign-op expr: {:?}",
                expr
            );
        };

        if let rustc_hir::ExprKind::Unary(rustc_hir::UnOp::Deref, inner) = lhs.kind {
            let ldid = expr.hir_id.owner.def_id;
            let typeck = self.tcx.typeck(ldid);
            let inner_ty = typeck.expr_ty(inner);
            if let rustc_middle::ty::Ref(_, referent, mutbl) = *inner_ty.kind() {
                if mutbl.is_mut() && referent.can_be_tupled() {
                    self.first_pass
                        .assign_through_tagged_ref_mut
                        .mark(expr.span, self.tcx.sess.source_map());
                }
            }
        }
    }
}
