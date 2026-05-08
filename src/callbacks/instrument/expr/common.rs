//! Defines helper functions utilized by all other files in `crate::callbacks::instrument::expr`.
//!
//! Namely this includes tupling and untupling operations, turning `expr` into `ATI::track(expr)`,
//! or `Tagged(Id, expr)` into `Tagged(Id, expr).1` to retrieve the expression.
//!
//! Further, this file defines a function to recursively make all bindings `mut` within patterns,
//! insert a reborrow operation on top of some expression, and determine whether some condition
//! expression contains a let-binding within it.

use crate::callbacks::instrument::instrument_visitor::InstrumentingVisitor;

/// Wraps an expression `e` of type `T` as `ATI::track(e)` of type `Tagged<T>` in place.
pub fn tuple(expr: &mut rustc_ast::Expr) {
    let mut ati_track = rustc_ast::Expr::dummy();
    ati_track.kind = rustc_ast::ExprKind::Path(
        None,
        rustc_ast::Path {
            segments: [
                rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("ATI")),
                rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("track")),
            ]
            .into(),
            tokens: None,
            span: rustc_span::DUMMY_SP,
        },
    );

    let inner = std::mem::replace(expr, rustc_ast::Expr::dummy());
    expr.kind = rustc_ast::ExprKind::Call(Box::new(ati_track), [Box::new(inner)].into());
}

/// Takes a `Tagged<T>` expression and unwraps it to just `T` via `.1` field access in place.
pub fn untuple(expr: &mut rustc_ast::Expr) {
    let inner = std::mem::replace(expr, rustc_ast::Expr::dummy());
    expr.kind = rustc_ast::ExprKind::Field(Box::new(inner), rustc_span::Ident::from_str("1"));
}

/// If `expr`'s span was marked by pass 1 as a `&mut T` (`T` tupleable), i.e.
/// post-instrumentation it is a `TaggedRefMut<T>`, rewrite it in place to
/// `(expr).reborrow()` so any consumption (move into a binding, into
/// emitted args, etc.) doesn't invalidate the original source binding.
/// `TaggedRefMut` is move-only.
pub fn reborrow_if_ref_mut(
    visitor: &crate::callbacks::instrument::instrument_visitor::InstrumentingVisitor,
    expr: &mut rustc_ast::Expr,
) {
    if !visitor
        .first_pass
        .ref_mut_to_tupleable
        .contains(expr.span, visitor.psess.source_map())
    {
        return;
    }

    let inner = std::mem::replace(expr, rustc_ast::Expr::dummy());
    expr.kind = rustc_ast::ExprKind::MethodCall(Box::new(rustc_ast::MethodCall {
        seg: rustc_ast::PathSegment::from_ident(rustc_span::Ident::from_str("reborrow")),
        receiver: Box::new(inner),
        args: [].into(),
        span: rustc_span::DUMMY_SP,
    }));
}

/// Walk a pattern and force every `Ident` binding's mutability to `mut`.
pub fn pat_force_mut_bindings(pat: &mut rustc_ast::Pat) {
    use rustc_ast::PatKind;
    match &mut pat.kind {
        PatKind::Ident(mode, _, sub) => {
            mode.1 = rustc_ast::Mutability::Mut;
            if let Some(sub) = sub {
                pat_force_mut_bindings(sub);
            }
        }
        PatKind::Tuple(elems)
        | PatKind::TupleStruct(_, _, elems)
        | PatKind::Or(elems)
        | PatKind::Slice(elems) => {
            for p in elems {
                pat_force_mut_bindings(p);
            }
        }
        PatKind::Struct(_, _, fields, _) => {
            for f in fields {
                pat_force_mut_bindings(&mut f.pat);
            }
        }
        PatKind::Box(inner)
        | PatKind::Deref(inner)
        | PatKind::Ref(inner, _, _)
        | PatKind::Paren(inner)
        | PatKind::Guard(inner, _) => {
            pat_force_mut_bindings(inner);
        }
        _ => {}
    }
}

/// True if `expr` is a `let PAT = EXPR` or an `&&` chain that contains one.
///
/// `let` only appears in if/while cond positions, optionally inside `&&`
/// chains (let-chains). Such chains must stay structurally intact as
/// `Binary(And, ..)` so the `Let` keeps a syntactically legal slot, and they
/// evaluate to raw `bool` rather than `Tagged<bool>`, so callers (binary,
/// if, while) must also treat them specially.
pub fn contains_let_chain(expr: &rustc_ast::Expr) -> bool {
    match &expr.kind {
        rustc_ast::ExprKind::Let(..) => true,
        rustc_ast::ExprKind::Binary(op, lhs, rhs) if op.node == rustc_ast::BinOpKind::And => {
            contains_let_chain(lhs) || contains_let_chain(rhs)
        }
        _ => false,
    }
}

/// Walk an arm pattern, lifting any sub-pattern marked in `tagged_lit_pat`.
/// On lift, `pat` is replaced with a fresh `ref __ati_pat_N` binding and
/// `frags` gains a string-form guard fragment that re-asserts the original
/// pattern against the inner value.
pub fn lift_lit_pats(
    visitor: &InstrumentingVisitor,
    pat: &mut rustc_ast::Pat,
    counter: &mut usize,
    frags: &mut Vec<String>,
) {
    let pats_requiring_lift = &visitor.first_pass.tagged_lit_pat;
    let sm = visitor.psess.source_map();

    // Or-pattern of marked literal/range alternatives, lift the whole Or as
    // a single binding+guard. Lifting alternatives individually would force
    // the Or to bind different names per arm, which is illegal.
    if let rustc_ast::PatKind::Or(alts) = &pat.kind {
        let any_lit_marked = alts.iter().any(|a| {
            matches!(
                a.kind,
                rustc_ast::PatKind::Expr(_) | rustc_ast::PatKind::Range(..)
            ) && pats_requiring_lift.contains(a.span, sm)
        });
        if any_lit_marked {
            lift_node(visitor, pat, counter, frags);
            return;
        }
    }

    if pats_requiring_lift.contains(pat.span, sm) {
        lift_node(visitor, pat, counter, frags);
        return;
    }

    match &mut pat.kind {
        rustc_ast::PatKind::Ident(_, _, Some(sub_pat))
        | rustc_ast::PatKind::Box(sub_pat)
        | rustc_ast::PatKind::Deref(sub_pat)
        | rustc_ast::PatKind::Ref(sub_pat, _, _)
        | rustc_ast::PatKind::Paren(sub_pat)
        | rustc_ast::PatKind::Guard(sub_pat, _) => {
            lift_lit_pats(visitor, sub_pat, counter, frags);
        }
        rustc_ast::PatKind::Tuple(pats)
        | rustc_ast::PatKind::TupleStruct(_, _, pats)
        | rustc_ast::PatKind::Slice(pats)
        | rustc_ast::PatKind::Or(pats) => {
            for sub_pat in pats {
                lift_lit_pats(visitor, sub_pat, counter, frags);
            }
        }
        rustc_ast::PatKind::Struct(_, _, fields, _) => {
            for f in fields {
                lift_lit_pats(visitor, &mut f.pat, counter, frags);
            }
        }
        _ => {}
    }
}

/// Replace `pat` with a fresh `ref __ati_pat_N` binding, recording a guard
/// fragment that re-checks the original pattern against `**__ati_pat_N`.
/// `ref` keeps the binding shape uniform across owned vs. borrowed match
/// targets; `**` derefs both the resulting `&Tagged<T>` and `Tagged<T>`'s
/// `Deref<Target = T>` impl, leaving the inner primitive that the original
/// literal/range pattern was written against.
fn lift_node(
    visitor: &InstrumentingVisitor,
    pat: &mut rustc_ast::Pat,
    counter: &mut usize,
    frags: &mut Vec<String>,
) {
    let n = *counter;
    *counter += 1;
    let name = format!("__ati_pat_{n}");
    let orig_str = rustc_ast_pretty::pprust::pat_to_string(pat);
    frags.push(format!(
        "(matches!(**{name}, {orig_str}))"
        // match **{name} {{ {orig_str} => true, _ => false }})
    ));
    pat.kind = rustc_ast::PatKind::Ident(
        rustc_ast::BindingMode::REF,
        rustc_span::Ident::from_str(&name),
        None,
    );
}
