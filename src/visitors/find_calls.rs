/* This file defines a visitor which is used during the first compiler invocation, to:
 * 1. Find all places where a non-user-defined function was called.
 *    Calls to functions which are not known by self.first_pass are considered
 *    to be untracked function calls, which require special handling later on.
 * 2. Find all places where a reference is constructed to some tuplable type.
 * 3. Find all places where a range is used as an index into some collection.
 * 4. Find all places where a mutable reference is assigned to.
 * 5. Find all places where a unary deref operation is used on a TaggedRef(Mut?) and the
 *    result needs to net a Tagged<T> as opposed to a T which is offered by the standard
 *    deref implementation.
*/

use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

use crate::common::CanBeTupled;
use crate::types::ati_info::FirstPassInfo;

/// Lang items for every range struct that is valid as an indexing argument and
/// produces a slice result (Range, RangeFrom, RangeFull, RangeInclusive,
/// RangeTo, RangeToInclusive, and their *Copy variants).
const RANGE_LANG_ITEMS: &[rustc_hir::LangItem] = &[
    rustc_hir::LangItem::Range,
    rustc_hir::LangItem::RangeFrom,
    rustc_hir::LangItem::RangeFull,
    rustc_hir::LangItem::RangeInclusiveStruct,
    rustc_hir::LangItem::RangeTo,
    rustc_hir::LangItem::RangeToInclusive,
    rustc_hir::LangItem::RangeCopy,
    rustc_hir::LangItem::RangeFromCopy,
    rustc_hir::LangItem::RangeInclusiveCopy,
    rustc_hir::LangItem::RangeToInclusiveCopy,
];

fn is_range_lang_item<'tcx>(tcx: TyCtxt<'tcx>, did: DefId) -> bool {
    RANGE_LANG_ITEMS
        .iter()
        .any(|&lang| tcx.is_lang_item(did, lang))
}

/// Visitor that finds code spans of interest (listed at the top of this file).
/// Updates self.first_pass to include this information after running.
pub struct AnalyzeHirVisitor<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub first_pass: &'a mut FirstPassInfo,
}

impl<'tcx, 'a> Visitor<'tcx> for AnalyzeHirVisitor<'tcx, 'a> {
    type NestedFilter = nested_filter::All;

    /// Combined with above NestedFilter, defines how the visitor
    /// is going to traverse the tree. This configuration will have
    /// this visitor visit all nested expressions, as in we are doing
    /// a "deep" traversal, visiting every single expression as opposed
    /// to doing a "shallow" traversal, visiting only the top-level exprs
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
                    if let Res::Def(kind, def_id) = typeck.qpath_res(qpath, func.hir_id) {
                        // ... and we have type information for it ...

                        // FIXME: I have low confidence in this, but for now this resolved a problem with
                        // enum and struct tuple constructors which appear as function calls.
                        // Given that we are currently ignoring the tracked/untracked boundary,
                        // I think this is fine for now. Is there anything different about constructing these
                        // types as opposed to calling a function from the perspective of the ATI analysis?
                        let is_constructor = matches!(kind, rustc_hir::def::DefKind::Ctor(_, _));
                        if !is_constructor && !self.first_pass.is_fn_def_id_tracked(&def_id) {
                            // ... and the function is untracked as self.first_pass never had
                            // the appropriate defid registered for it.

                            // this function call might need to have it's inputs
                            // untupled, and it's output tupled, depending on the type signature.
                            // store all this information in FirstPassInfo.
                            let span = func.span;
                            let ret_ty = typeck.expr_ty(expr);
                            self.first_pass.observe_untracked_fn_call(span, ret_ty);
                        }
                    }
                } else {
                    // TODO: could an instrumented call have a non-path kind?
                    // yes? closures?
                }
            }

            // we are taking a reference to some sort of expression. If the 
            // reference is to some type which is tuplable (e.g. &u32, or &mut &f64)
            // then during instrumentation we need to create a TaggedRef<T> from the Tagged<T>.
            // Store this location, to perform the transformation in the next pass.
            //
            // A TaggedRef(Mut?)<T> == (&(mut?) Id, &(mut?) T). Nested references will only
            // turn the innermost reference into a TaggedRef. In other words, for some tuplable T,
            // &&&T becomes &&&Tagged<T> which becomes &&TaggedRef<T>, to avoid dragging around an 
            // owned Id. If T is a not a tuplable type (i.e. a user-defined compound type), then 
            // &T remains &T.
            hir::ExprKind::AddrOf(_, _, inner) => {
                let ldid = expr.hir_id.owner.def_id;
                let typeck = self.tcx.typeck(ldid);

                // This is no longer necessary, as we utilize the coerce_unsized and unsize 
                // features, to allow a TaggedRef<[T; N]> to automatically coerce to a 
                // TaggedRef<[T]>.
                // let adjustments = typeck.expr_adjustments(expr);
                // if adjustments.iter().any(|adjustment| {
                //     matches!(adjustment.kind, Adjust::Pointer(PointerCoercion::Unsize))
                // }) {
                //     self.first_pass.observe_unsized_ref_coercion(expr.span)
                // }

                let inner_ty = typeck.expr_ty(inner);
                let elem_tupleable = |ty: rustc_middle::ty::Ty<'tcx>| ty.can_be_tupled();
                let is_tagged_wrapped = inner_ty.can_be_tupled()
                    || matches!(inner_ty.kind(), rustc_middle::ty::Array(elem, _) if elem_tupleable(*elem))
                    || matches!(inner_ty.kind(), rustc_middle::ty::Slice(elem) if elem_tupleable(*elem));
                if is_tagged_wrapped {
                    self.first_pass.observe_ref_to_tupleable_ty(expr.span);
                }
            }

            // Unary * on an instrumented &T / &mut T with tupleable T
            // strips the tag post-instrumentation (TaggedRef::deref -> T). Record
            // the span so pass 2 can rebuild a Tagged<T> from the borrowed fields.
            // FIXME: In the future, this has to be fixed up to allow for all smart pointers.
            // For now, smart pointers (Box) stay using a plain `*`.
            hir::ExprKind::Unary(hir::UnOp::Deref, inner) => {
                let ldid = expr.hir_id.owner.def_id;
                let typeck = self.tcx.typeck(ldid);
                let inner_ty = typeck.expr_ty(inner);
                if let rustc_middle::ty::Ref(_, referent, _) = *inner_ty.kind() {
                    if referent.can_be_tupled() {
                        self.first_pass.observe_tag_stripping_deref(expr.span);
                    }
                }
            }

            // Assignment (or compound assign) whose LHS is *expr where expr
            // is &mut T with tupleable T. Post-instrumentation the LHS is a
            // TaggedRefMut<T>; a plain *lhs = rhs goes through DerefMut and
            // only touches the value field (.1), leaving the old id (.0) behind. 
            // Record the span so pass 2 rewrites it to expr.assign(rhs), which writes both fields.
            // .assign is defined in the runtime library, on the TaggedRefMut type.
            hir::ExprKind::Assign(lhs, _, _) | hir::ExprKind::AssignOp(_, lhs, _) => {
                if let hir::ExprKind::Unary(hir::UnOp::Deref, inner) = lhs.kind {
                    let ldid = expr.hir_id.owner.def_id;
                    let typeck = self.tcx.typeck(ldid);
                    let inner_ty = typeck.expr_ty(inner);
                    if let rustc_middle::ty::Ref(_, referent, mutbl) = *inner_ty.kind() {
                        if mutbl.is_mut() && referent.can_be_tupled() {
                            self.first_pass.observe_assign_through_tagged_ref_mut(expr.span);
                        }
                    }
                }
            }

            // Indexing is usually handled via traits defined on the Tagged* types in the 
            // runtime library. Ranges are special cased however, and SliceIndex cannot be 
            // overloaded in the way that the Index operation can. Therefore, we have to 
            // record places where a range is used as an index, to correctly transform it
            // to the appropriate subslice operation in the next pass.
            hir::ExprKind::Index(_, idx, _) => {
                let ldid = expr.hir_id.owner.def_id;
                let typeck = self.tcx.typeck(ldid);
                let idx_ty = typeck.expr_ty(idx);
                if idx_ty.ty_adt_def().map(|adt| is_range_lang_item(self.tcx, adt.did())).unwrap_or(false) {
                    self.first_pass.observe_index_by_range(expr.span);
                }
            }

            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}
