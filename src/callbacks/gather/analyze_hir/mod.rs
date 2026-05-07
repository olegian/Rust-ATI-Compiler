//! Defines a visitor which walks the HIR, and gathers expression-level information used by the
//! second instrument compilation.
//!
//! More specifically, this visitor collects (and adds to [`FirstPassInfo`] code locations where:
//! - A call to an uninstrumented function is made. These are places where any tupled
//!   values passed in as input to this function need to be untupled, and a return value
//!   potentially needs to be tupled.
//!
//! - A reference is created to some tuplable value (e.g. `let x = &mut 10`). These are
//!   places where a [`TaggedRef<T>`] or [`TaggedRefMut<T>`] needs to be constructed
//!   from a [`Tagged<T>`]. Note that for non-tuplable types (like compound types),
//!   we continue to use a regular reference, requiring no AST transformation.
//!
//! - A reference to tuplable value is dereferenced (e.g. `*x` with above `x`). The
//!   runtime library defines [`TaggedRef::deref`] to a type `T`, to allow calling
//!   methods defined on `T` on `TaggedRef<T>`. However, these places, where an explicit
//!   derefernce is used, need to convert the [`TaggedRef<T>`] to a `[Tagged<T>]` instead,
//!   so that the tag is not stripped off. Record these locations, so that we can
//!   reconstruct the `Tagged<T>` where necessary.
//!
//! - A mutable reference to a tuplable value is used as a place value in an assignment
//!   (e.g. `*x = 20`, with above `x`). Pre-transformation, this assignment was writing
//!   just a single value to a place. Once transformed, this assignment has to write both
//!   the value and the value's tag, requiring special casing.
//!
//! - A range is used as an index into some collection (using the SliceIndex operator).
//!   Ranges require special casing as the SliceIndex trait is sealed, and cannot be
//!   implemented outside of rustc. When used as an index, they require a slightly
//!   different operation to be emitted to push the range index operation down into the
//!   `TaggedRef<[T]>` or `Tagged<[T; N]>` it acts on. See
//!   [crate::callbacks::instrument::expr::addr_of] for more information.
//!   Whenever the standard library is instrumented, it's possible this could be removed.
//!
//! As of 3/29/26, we are choosing to ignore uninstrumented libraries, meaning that
//! the first bullet is really an unnecessary step. The code is still left, as a proof-of-concept.

use crate::{callbacks::gather::first_pass_info::FirstPassInfo, callbacks::types::CanBeTupled};

mod assignment;
mod call;
mod deref;
mod index;
mod references;

/// Visitor that finds code spans of interest (listed at the top of this file).
/// Updates self.first_pass to include this information.
pub struct AnalyzeHirVisitor<'tcx, 'a> {
    pub tcx: rustc_middle::ty::TyCtxt<'tcx>,
    pub first_pass: &'a mut FirstPassInfo,
}

impl<'tcx, 'a> rustc_hir::intravisit::Visitor<'tcx> for AnalyzeHirVisitor<'tcx, 'a> {
    type NestedFilter = rustc_middle::hir::nested_filter::All;

    /// Combined with above NestedFilter, defining this method defines how the visitor
    /// is going to traverse the tree. This configuration will have
    /// this visitor visit all nested expressions, as in we are doing
    /// a "deep" traversal, visiting every single expression as opposed
    /// to doing a "shallow" traversal, visiting only the top-level exprs.
    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    /// Anon consts (array lengths, const generics, inline consts) live in
    /// their own owner with no typeck results, and have no values for us to
    /// instrument. Skip the entire subtree when encountered.
    fn visit_anon_const(&mut self, _: &'tcx rustc_hir::AnonConst) {}

    /// Called on each expression.
    fn visit_expr(&mut self, expr: &'tcx rustc_hir::Expr<'tcx>) {
        // Skip subtrees whose owner has no typeck results (e.g. struct/enum
        // item bodies reached via nested-filter walks, inline consts).
        let ldid = expr.hir_id.owner.def_id;
        if !self.tcx.has_typeck_results(ldid) {
            return;
        }

        // Regardless of the expr kind, record any expression whose adjusted type is
        // `&mut T` with a tupleable `T`. Pass 2 must explicitly reborrow such operands
        // before consuming them, as &mut T does not transfer ownership when moved, but a
        // TaggedRefMut<T> = (&mut Id, &mut T) will. Reborrowing allows the [`TaggedRefMut`]
        // to act in a semantically identical way to the uninstrumented `&mut T`.
        // See [`FirstPassInfo::ref_mut_to_tupleable_locs`].
        let typeck = self.tcx.typeck(ldid);
        let expr_ty = typeck.expr_ty(expr);
        if let rustc_middle::ty::Ref(_, referent, mutbl) = *expr_ty.kind() {
            if mutbl.is_mut() && referent.can_be_tupled() {
                self.first_pass
                    .ref_mut_to_tupleable
                    .mark(expr.span, self.tcx.sess.source_map());
            }
        }

        match expr.kind {
            // A call to a function might require us to untuple the arguments,
            // and then tuple back the return value, if it is a call to a function
            // which we are not going to be instrumenting.
            rustc_hir::ExprKind::Call(..) => {
                self.observe_call(expr);
            }

            // we are taking a reference to some sort of expression. If the
            // reference is to some type which is tuplable (e.g. &u32, or &mut &f64)
            // then during instrumentation we need to create a TaggedRef<T> from the Tagged<T>.
            rustc_hir::ExprKind::AddrOf(..) => {
                self.observe_ref(expr);
            }

            // Unary * on an instrumented &T / &mut T with tupleable T
            // strips the tag post-instrumentation (TaggedRef::deref -> T). Record
            // the span so pass 2 can rebuild a Tagged<T> from the borrowed fields,
            // and as a result have *&TaggedRef<T> net a Tagged<T>.
            // FIXME: In the future, this has to be fixed up to allow for all smart pointers.
            // For now, smart pointers (Box) stay using a plain `*`.
            rustc_hir::ExprKind::Unary(rustc_hir::UnOp::Deref, _) => {
                self.observe_deref(expr);
            }

            // Assignment (or compound assign) whose LHS is *expr where expr
            // is &mut T with tupleable T. Post-instrumentation the LHS is a
            // TaggedRefMut<T>; a plain *lhs = rhs goes through DerefMut and
            // only touches the value field (.1), leaving the old id (.0) behind.
            // Record the span so pass 2 rewrites it to expr.assign(rhs), which writes both fields.
            // .assign is defined in the runtime library, on the TaggedRefMut type.
            rustc_hir::ExprKind::Assign(..) | rustc_hir::ExprKind::AssignOp(..) => {
                self.observe_assignment(expr);
            }

            // Indexing is usually handled via traits defined on the Tagged* types in the
            // runtime library. Ranges are special cased however, and SliceIndex cannot be
            // overloaded in the way that the Index operation can. Therefore, we have to
            // record places where a range is used as an index, to correctly transform it
            // to the appropriate subslice operation in the next pass.
            rustc_hir::ExprKind::Index(..) => {
                self.observe_range(expr);
            }

            _ => {}
        }

        rustc_hir::intravisit::walk_expr(self, expr);
    }
}
