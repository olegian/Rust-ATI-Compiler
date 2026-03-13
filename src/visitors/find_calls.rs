/* This file defines a visitor which can be used to discover the code locations
 * where untracked function calls are being made. For each call location, we will also
 * store information about the returned type, to later make decisions about whether
 * to bring the return values into our "tracked" context. After performing the visitor
 * pass, self.fbs will be mutated to include all of this information.
*/

use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

use crate::types::ati_info::FunctionBoundaries;

/// Visitor that finds all invocations of untracked functions,
/// making sure to record those locations in the contained self.fbs.
pub struct FindUntrackedCallsVisitor<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub fbs: &'a mut FunctionBoundaries,
}

impl<'tcx, 'a> Visitor<'tcx> for FindUntrackedCallsVisitor<'tcx, 'a> {
    type NestedFilter = nested_filter::All;

    /// Combined with above NestedFilter, defines how the visitor
    /// is going to traverse the tree. This configuration will have
    /// this visitor visit all nested expressions.
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    /// Called on each expression.
    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            // we've found a call to a function...
            hir::ExprKind::Call(func, _args) => {
                if let hir::ExprKind::Path(ref qpath) = func.kind {
                    let ldid = expr.hir_id.owner.def_id;

                    let typeck = self.tcx.typeck(ldid);
                    if let Res::Def(_kind, def_id) = typeck.qpath_res(qpath, func.hir_id) {
                        // ... and we have type information for it, with a def
                        if !self.fbs.is_fn_def_id_tracked(&def_id) {
                            // .. and the function is untracked.

                            // this function call might need to have it's inputs
                            // untupled, and it's output tupled, depending on the type signature.
                            // store all this information in FunctionBoundaries.
                            let span = func.span;
                            let ret_ty = typeck.expr_ty(expr);
                            self.fbs.observe_untracked_fn_call(span, ret_ty);
                        }
                    }
                } else {
                    // TODO: could an instrumented call have a non-path kind?
                }
            }

            hir::ExprKind::AddrOf(borrow_kind, mutability, inner_expr) => {
                // let res =  self.tcx.type_of(inner_expr.hir_id.owner.def_id);
                // let res = typeck.expr_ty(expr);
                let ldid = inner_expr.hir_id.owner.def_id;
                let typeck = self.tcx.typeck(ldid);

                // NB (2): This type doesn’t provide type parameter args; e.g., if you ask for the type of id in id(3), it will return fn(&isize) -> isize instead of fn(ty) -> T with T = isize.
                let current_ty = typeck.expr_ty(expr);
                let coerced_ty = typeck.expr_ty_adjusted(expr);

                // TODO: not sure if this is the correct condition,
                // we might just apply this transformation to all unsized types
                // using res.is_sized;
                if current_ty != coerced_ty {
                    self.fbs.observe_slice_coercion(expr.span);
                }
            }
            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}
