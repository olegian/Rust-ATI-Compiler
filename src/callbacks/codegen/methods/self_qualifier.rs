//! Qualifies all associated type references via a visitor to include the name of the trait
//! which contains the associated type.
//!
//! See [crate::callbacks::codegen::methods] for more information on method shims.

/// In-place mut visitor that rewrites every `Self::X` path in
/// signature types, body expressions, and body patterns into the
/// fully-qualified form `<Self as Trait>::X`.
pub struct SelfPathQualifier<'a> {
    pub trait_segs: &'a [rustc_ast::PathSegment],
}

impl<'a> SelfPathQualifier<'a> {
    /// Rewrites a path segment, if it accesses an associated type via `Self`
    fn maybe_rewrite(&self, qself: &mut Option<Box<rustc_ast::QSelf>>, path: &mut rustc_ast::Path) {
        if qself.is_some() || path.segments.len() < 2 {
            return;
        }
        if path.segments[0].ident.name != rustc_span::symbol::kw::SelfUpper {
            return;
        }

        let tail: thin_vec::ThinVec<rustc_ast::PathSegment> =
            path.segments.iter().skip(1).cloned().collect();

        let mut new_segs: thin_vec::ThinVec<rustc_ast::PathSegment> =
            self.trait_segs.iter().cloned().collect();

        let position = new_segs.len();
        new_segs.extend(tail);
        path.segments = new_segs;

        let self_ty = Box::new(rustc_ast::Ty {
            id: rustc_ast::DUMMY_NODE_ID,
            kind: rustc_ast::TyKind::Path(
                None,
                rustc_ast::Path {
                    span: rustc_span::DUMMY_SP,
                    segments: thin_vec::thin_vec![rustc_ast::PathSegment {
                        ident: rustc_span::Ident::with_dummy_span(
                            rustc_span::symbol::kw::SelfUpper
                        ),
                        id: rustc_ast::DUMMY_NODE_ID,
                        args: None,
                    }],
                    tokens: None,
                },
            ),
            span: rustc_span::DUMMY_SP,
            tokens: None,
        });
        *qself = Some(Box::new(rustc_ast::QSelf {
            ty: self_ty,
            path_span: rustc_span::DUMMY_SP,
            position,
        }));
    }
}

/// Self paths could be in types, expressions, or patterns.
/// Make sure to visit all of them.
impl<'a> rustc_ast::mut_visit::MutVisitor for SelfPathQualifier<'a> {
    /// Rewrites `Self` types.
    fn visit_ty(&mut self, ty: &mut rustc_ast::Ty) {
        if let rustc_ast::TyKind::Path(qself, path) = &mut ty.kind {
            self.maybe_rewrite(qself, path);
        }
        rustc_ast::mut_visit::walk_ty(self, ty);
    }

    /// Rewrites `Self` type paths in exprs.
    fn visit_expr(&mut self, expr: &mut rustc_ast::Expr) {
        if let rustc_ast::ExprKind::Path(qself, path) = &mut expr.kind {
            self.maybe_rewrite(qself, path);
        }
        rustc_ast::mut_visit::walk_expr(self, expr);
    }

    /// Rewrites `Self` type paths in patterns.
    fn visit_pat(&mut self, pat: &mut rustc_ast::Pat) {
        if let rustc_ast::PatKind::Path(qself, path) = &mut pat.kind {
            self.maybe_rewrite(qself, path);
        }
        rustc_ast::mut_visit::walk_pat(self, pat);
    }
}
