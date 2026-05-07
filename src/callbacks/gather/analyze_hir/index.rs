//! Defines how the [`AnalyzeHirVisitor`] records information about index expressions.
//!
//! See the top-level comment in [crate::callbacks::gather::analyze_hir] for more information as to
//! why this is necessary.

use crate::callbacks::gather::analyze_hir::AnalyzeHirVisitor;

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

/// Returns true if this `did` refers to some Range variant.
fn is_range_lang_item<'tcx>(
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    did: rustc_span::def_id::DefId,
) -> bool {
    RANGE_LANG_ITEMS
        .iter()
        .any(|&lang| tcx.is_lang_item(did, lang))
}

impl<'tcx, 'a> AnalyzeHirVisitor<'tcx, 'a> {
    /// If the index expression uses a range as the index into some collection,
    /// then record this expression as requiring a special `.subslice` call post-transformation
    /// which will construct an equivalent non-instrumented range object before applying the index
    /// operation to the collection.
    pub fn observe_range(&mut self, expr: &rustc_hir::Expr) {
        let rustc_hir::ExprKind::Index(_, idx, _) = expr.kind else {
            panic!("Invoked observe_range with non-range expr: {:?}", expr);
        };

        let ldid = expr.hir_id.owner.def_id;
        let typeck = self.tcx.typeck(ldid);
        let idx_ty = typeck.expr_ty(idx);
        if idx_ty
            .ty_adt_def()
            .map(|adt| is_range_lang_item(self.tcx, adt.did()))
            .unwrap_or(false)
        {
            self.first_pass
                .index_by_range
                .mark(expr.span, self.tcx.sess.source_map());
        }
    }
}
