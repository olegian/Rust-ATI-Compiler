//! Defines how the [`AnalyzeHirVisitor`] records information about call expressions.
//!
//! See the top-level comment in [crate::callbacks::gather::analyze_hir] for more information as to
//! why this is necessary.

use crate::{
    callbacks::gather::{analyze_hir::AnalyzeHirVisitor, first_pass_info::UntrackedCall},
    callbacks::types::CanBeTupled,
};

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    /// If the call expression is to a non-instrumented function, mark this
    /// call as requiring argument untupling, and potentially return value tupling.
    pub fn observe_call(&mut self, expr: &rustc_hir::Expr) {
        let rustc_hir::ExprKind::Call(func, _args) = expr.kind else {
            panic!("Called observe_call with non-call expression.");
        };

        if let rustc_hir::ExprKind::Path(ref qpath) = func.kind {
            let ldid = expr.hir_id.owner.def_id;
            let typeck = self.tcx.typeck(ldid);
            if let rustc_hir::def::Res::Def(kind, def_id) = typeck.qpath_res(qpath, func.hir_id) {
                // Tuple struct constructors are parsed as calls. Skip them.
                let is_constructor = matches!(kind, rustc_hir::def::DefKind::Ctor(_, _));
                if !is_constructor && !self.first_pass.fns.contains(&def_id) {
                    // We found a function that is untracked, as self.first_pass never had
                    // the appropriate defid registered for it.

                    // this function call might need to have it's inputs
                    // untupled, and it's output tupled, depending on the type signature.
                    // store all this information in FirstPassInfo.
                    let span = func.span;
                    let ret_ty = typeck.expr_ty(expr);
                    self.first_pass.untracked_fn_calls.record(
                        span,
                        self.tcx.sess.source_map(),
                        UntrackedCall {
                            ret_is_tupleable: ret_ty.can_be_tupled(),
                        },
                    );
                }
            }
        } else {
            // FIXME: could an instrumented call have a non-path kind?
            // yes? closures? ignoring for now...
        }
    }
}
