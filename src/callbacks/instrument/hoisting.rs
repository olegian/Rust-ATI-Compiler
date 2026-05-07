//! Defines functions used by the [`InstrumentingVisitor`] to hoist statements, avoiding dropping
//! temporary values while holding references to them.
//!
//! Because instrumentation inserts method calls on Tagged<T>s, some calls consume a
//! temporary value and return an reference to it. This requires the temporary value to be bound
//! to a variable before the reference is constructed, to avoid dropping the value on exit from the
//! method and creating a dangling reference.
//!
//! Invoking [`maybe_hoist_binding`] will take a statement, recurse into it to find all places
//! a temporary is created before being passed to a ATI library function, and create multiple
//! statements out of the one, binding each temporary to a variable with a unique name.

use crate::{callbacks::instrument::instrument::InstrumentingVisitor, callbacks::parsing};

/// Hoist any method invocations to ATI library functions within this statement
/// by creating temporary let-bindings.
pub fn maybe_hoist_binding(
    visitor: &mut InstrumentingVisitor,
    mut stmt: rustc_ast::Stmt,
) -> smallvec::SmallVec<[rustc_ast::Stmt; 1]> {
    let rustc_ast::StmtKind::Let(box rustc_ast::Local {
        kind: rustc_ast::LocalKind::Init(box expr),
        ..
    }) = &mut stmt.kind
    else {
        return smallvec::smallvec![stmt];
    };

    let mut hoists = Vec::new();
    collect_hoists(visitor, expr, &mut hoists, &mut 0);
    hoists
        .into_iter()
        .map(|(name, recv)| {
            let code = format!(
                "let mut {name} = {};",
                rustc_ast_pretty::pprust::expr_to_string(&recv)
            );
            parsing::parse_stmt(visitor.psess, code)
        })
        .chain(std::iter::once(stmt))
        .collect()
}

/// Recurse into a statement to find all places where a hoist is necessary,
/// populating `hoists` with tuples of unique variable name strings and the corresponding
/// expressions that are going to be bound to it.
fn collect_hoists(
    visitor: &mut InstrumentingVisitor,
    expr: &mut rustc_ast::Expr,
    hoists: &mut Vec<(String, rustc_ast::Expr)>,
    hoist_counter: &mut u64,
) {
    // Recurse into all exprs that could contain expressions that require hoisting
    match &mut expr.kind {
        // single inner expression
        rustc_ast::ExprKind::AddrOf(_, _, inner)
        | rustc_ast::ExprKind::Unary(_, inner)
        | rustc_ast::ExprKind::Field(inner, _)
        | rustc_ast::ExprKind::Paren(inner)
        | rustc_ast::ExprKind::Cast(inner, _)
        | rustc_ast::ExprKind::Repeat(inner, _) => {
            collect_hoists(visitor, inner, hoists, hoist_counter);
        }

        // multiple exprs to recurse into
        rustc_ast::ExprKind::Tup(elems) | rustc_ast::ExprKind::Array(elems) => {
            for e in elems {
                collect_hoists(visitor, e, hoists, hoist_counter);
            }
        }
        rustc_ast::ExprKind::Call(f, args) => {
            collect_hoists(visitor, f, hoists, hoist_counter);
            for a in args {
                collect_hoists(visitor, a, hoists, hoist_counter);
            }
        }
        rustc_ast::ExprKind::MethodCall(mc) => {
            collect_hoists(visitor, &mut mc.receiver, hoists, hoist_counter);
            for a in &mut mc.args {
                collect_hoists(visitor, a, hoists, hoist_counter);
            }
        }
        rustc_ast::ExprKind::Index(base, idx, _) => {
            collect_hoists(visitor, base, hoists, hoist_counter);
            collect_hoists(visitor, idx, hoists, hoist_counter);
        }
        rustc_ast::ExprKind::Binary(op, lhs, rhs) => {
            if !matches!(
                op.node,
                rustc_ast::BinOpKind::And | rustc_ast::BinOpKind::Or
            ) {
                collect_hoists(visitor, lhs, hoists, hoist_counter);
                collect_hoists(visitor, rhs, hoists, hoist_counter);
            }
        }
        rustc_ast::ExprKind::Range(s, e, _) => {
            if let Some(s) = s {
                collect_hoists(visitor, s, hoists, hoist_counter);
            }
            if let Some(e) = e {
                collect_hoists(visitor, e, hoists, hoist_counter);
            }
        }
        rustc_ast::ExprKind::Struct(se) => {
            for field in &mut se.fields {
                collect_hoists(visitor, &mut field.expr, hoists, hoist_counter);
            }
        }
        _ => {}
    }

    // double check that this is in fact an expression
    // that should be hoisted. Only method call invocations
    // should be hoisted (as we added, for instance, a
    // `expr.as_tagged_ref()` call).
    if !requires_hoist(expr) {
        return;
    }

    // minor fixme: should remove this unreachable by changing requires_hoist return
    let rustc_ast::ExprKind::MethodCall(mc) = &mut expr.kind else {
        unreachable!();
    };

    let id = *hoist_counter;
    *hoist_counter += 1;

    let name = format!("__ati_hoist_{id}");
    let new_recv = parsing::parse_expr(visitor.psess, name.clone());
    let old_recv = std::mem::replace(&mut *mc.receiver, new_recv);
    hoists.push((name, old_recv));
}

/// Returns true if this expression requires a hoist to be included.
/// Expressions require a hoist if it is a method call to one of the library functions,
/// and the expression is already not bound to a place.
fn requires_hoist(expr: &rustc_ast::Expr) -> bool {
    matches!(
        &expr.kind,
        rustc_ast::ExprKind::MethodCall(mc)
        if matches!(
            mc.seg.ident.name.as_str(),
            "as_tagged_ref"
            | "as_tagged_ref_mut"
            | "subslice"
            | "subslice_mut"
        ) && !is_place_expr(&mc.receiver)
    )
}

/// Returns true if this expression is already in a place context, in which case the expression
/// is already bound and does not need a hoist.
fn is_place_expr(expr: &rustc_ast::Expr) -> bool {
    matches!(
        expr.kind,
        rustc_ast::ExprKind::Path(..)
            | rustc_ast::ExprKind::Field(..)
            | rustc_ast::ExprKind::Index(..)
            | rustc_ast::ExprKind::Unary(rustc_ast::UnOp::Deref, _)
    )
}
