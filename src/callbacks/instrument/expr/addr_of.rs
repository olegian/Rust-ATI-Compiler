//! Defines a function to transform a single `AddrOf` AST expression.
//!
//! If the first pass determined that this reference is being taken to a slice or array type
//! that is being indexed by a range (`&array[a..b]`, `&slice[..]`, etc...) then use the
//! runtime library's `.subslice()` / `.subslice_mut()` method call, to push the indexing operation
//! down into the collection's representation (for instance, into a `TaggedRef<[T]>`).
//!
//! If the first pass determined that this reference is being taken to some tuplable type `T`,
//! then use the runtime library's `.as_tagged_ref()` / `.as_tagged_ref_mut()` methods to convert
//! the `Tagged<T>` into a `TaggedRef<T>` / `TaggedRefMut<T>`.

use crate::{callbacks::instrument::instrument_visitor::InstrumentingVisitor, callbacks::parsing};

/// Invoked whenever the visitor runs into an `ExprKind::AddrOf`.
pub fn transform_addr_of(visitor: &mut InstrumentingVisitor, addr_of_expr: &mut rustc_ast::Expr) {
    let rustc_ast::ExprKind::AddrOf(_, mutbl, referent) = &mut addr_of_expr.kind else {
        panic!(
            "Invoked transform_addr_of with non addr-of expression: {:?}",
            rustc_ast_pretty::pprust::expr_to_string(addr_of_expr)
        );
    };

    // This reference is taken after indexing a array/slice with a range.
    // It would be nice to do this on the Index expression itself,
    // however we need to know whether this is a mutable ref or a shared ref
    // to know which method to dispatch.
    if visitor
        .first_pass
        .index_by_range
        .contains(referent.span, visitor.psess.source_map())
    {
        let rustc_ast::ExprKind::Index(idx_recv, idx_expr, _) = &referent.kind else {
            panic!(
                "First pass identified {:?} as the span of a index-by-range, yet \
                 second pass found a non-index expression: {:?}",
                referent.span,
                rustc_ast_pretty::pprust::expr_to_string(referent)
            );
        };
        let mut_str = if mutbl.is_mut() { "_mut" } else { "" };
        let recv_src = rustc_ast_pretty::pprust::expr_to_string(idx_recv);
        let idx_src = rustc_ast_pretty::pprust::expr_to_string(idx_expr);
        let code = format!("{recv_src}.subslice{mut_str}({idx_src})");
        *addr_of_expr = parsing::parse_expr(visitor.psess, code);
        return;
    }

    // handle refernces to tupleable types.
    if visitor
        .first_pass
        .ref_to_tupleable_ty
        .contains(addr_of_expr.span, visitor.psess.source_map())
    {
        // need to transform to (addr_of).as_tagged_ref()
        let mut_str = if mutbl.is_mut() { "_mut" } else { "" };

        let old_kind = std::mem::replace(
            &mut addr_of_expr.kind,
            rustc_ast::ExprKind::Tup(thin_vec::ThinVec::new()),
        );
        let receiver = Box::new(rustc_ast::Expr {
            id: addr_of_expr.id,
            span: addr_of_expr.span,
            attrs: std::mem::take(&mut addr_of_expr.attrs),
            tokens: addr_of_expr.tokens.take(),
            kind: old_kind,
        });
        addr_of_expr.kind = rustc_ast::ExprKind::MethodCall(Box::new(rustc_ast::MethodCall {
            seg: rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str(&format!(
                "as_tagged_ref{mut_str}"
            ))),
            receiver,
            args: [].into(),
            span: rustc_span::DUMMY_SP,
        }));
    }
}
