use crate::callbacks::{gather::analyze_hir::AnalyzeHirVisitor, types::CanBeTupled};

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    pub fn observe_match(&mut self, expr: &rustc_hir::Expr) {
        let rustc_hir::ExprKind::Match(target, _arms, _match_kind) = expr.kind else {
            panic!("Invoked observe_match with non-match expr: {:?}", expr);
        };

        let ldid = target.hir_id.owner.def_id;
        let typeck = self.tcx.typeck(ldid);
        let inner_ty = typeck.expr_ty(target);
        if inner_ty.peel_refs().can_be_tupled() {
            self.first_pass
                .match_on_tagged
                .mark(target.span, self.tcx.sess.source_map());
        }
    }
}
