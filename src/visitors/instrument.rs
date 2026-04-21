/* Defines a visitor which performs all AST-to-AST transformations needed for ATI.
 * This visitor handles both expression instrumentation and type wrapping in a single
 * pass. It does not handle inserting any stub code into the crate, nor renaming any idents.
*/
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::{self as ast, BinOpKind, DUMMY_NODE_ID, GenericArgs};
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, Ident};
use smallvec::{SmallVec, smallvec};

use crate::common::{self, CanBeTupled, DatirConfig};
use crate::types::ati_info::FirstPassInfo;

/// Enumerates the different types of operations that can be observed.
enum OpKind {
    Logical,
    Comparison,
    Arithmetic,
}

pub struct TransformVisitor<'a> {
    datir_config: &'a DatirConfig,
    first_pass: &'a FirstPassInfo,
    psess: &'a ParseSess,
    /// Unique suffix for synthesized `__ati_hoist_N` locals produced by the
    /// let-hoist pass. Shared across the whole crate traversal.
    hoist_counter: u32,
}

impl<'a> MutVisitor for TransformVisitor<'a> {
    // define to stop visitor from modifying any expressions used as types
    fn visit_param(&mut self, _node: &mut ast::Param) {}

    // define to stop visitor from modifying anon consts which are required to be of the original type.
    // usually this is for things like array lengths. During runtime, these constants
    // will receive tags when the data structure is actually created.
    fn visit_anon_const(&mut self, _node: &mut rustc_ast::AnonConst) {}

    /// Updates type annotations on `let` bindings so that primitives like `let x: u32`
    /// become `let x: Tagged<u32>` in sync with the rest of the instrumentation.
    fn visit_local(&mut self, local: &mut ast::Local) {
        if let Some(ty) = &mut local.ty {
            self.recursively_tuple_type(ty);
        }
        mut_visit::walk_local(self, local);
    }

    /// After the default walk rewrites exprs, scan the resulting let init for every
    /// .as_tagged_ref(_mut?)() method call whose receiver is not a place.
    /// This is so we avoid creating a temporary value, then returning and storing a
    /// reference to it, without ever binding the value with it's own let statement.
    ///
    /// The expr-rewriter replaces &<rvalue> with <rvalue>.as_tagged_ref(), but temp-lifetime
    /// extension does not reach through a method-call receiver. The temp dies at
    /// statement end while the returned TaggedRef may need to live to block end.
    ///
    /// Each non-place receiver becomes __ati_hoist_N (with a unique N), the statement is
    /// places after a `let __ati_hoist_N = <recv>;`, so the TaggedRef
    /// borrows from a block-scoped local instead of a dead expression.
    fn flat_map_stmt(&mut self, stmt: ast::Stmt) -> SmallVec<[ast::Stmt; 1]> {
        let stmts = mut_visit::walk_flat_map_stmt(self, stmt);
        if stmts.len() != 1 {
            return stmts;
        }
        let mut iter = stmts.into_iter();
        let stmt = iter.next().unwrap();
        self.maybe_hoist_ref_binding(stmt)
    }

    /// Performs expression instrumentation (literals, binary ops, calls, etc.)
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        // Assigning through a TaggedRefMut must require a rewrite BEFORE the default
        // walk, because we do not want to rewrite the LHS, just replace it with a
        // lhs.assign(rhs) statement.
        if self.first_pass.is_assign_through_tagged_ref_mut(&expr.span) {
            if let ast::ExprKind::Assign(lhs, rhs, _) = &mut expr.kind {
                if let ast::ExprKind::Unary(ast::UnOp::Deref, inner) = &mut lhs.kind {
                    mut_visit::walk_expr(self, inner);
                    mut_visit::walk_expr(self, rhs);

                    let code = format!(
                        "{}.assign({})",
                        pprust::expr_to_string(inner),
                        pprust::expr_to_string(rhs),
                    );
                    *expr = common::parse_expr(self.psess, code);
                    return;
                }
            }
        }

        mut_visit::walk_expr(self, expr);

        match &mut expr.kind {
            // Convert all literals into Tagged<T>, if necessary
            ast::ExprKind::Lit(lit) => {
                if lit.can_be_tupled() {
                    *expr = self.tuplify_expr(expr);
                }
            }

            ast::ExprKind::AddrOf(_kind, mutbl, inner_expr) => {
                // SLICING WITH RANGES: indexing a slice/array by
                // a Tagged<Range<..>> has no native SliceIndex impl. The
                // borrow would need to live over an unsized result. Rewrite to use
                // ATI::track_subslice(_mut?) which consumes the tagged
                // range (merging ids into the collection's AT) and returns a
                // `TaggedRef(Mut)?<[T]>` in one shot.

                // Must run before the default AddrOf -> `.as_tagged_ref()` path because that path
                // calls `.as_tagged_ref()` on a place that doesn't type-check.
                if let ast::ExprKind::Index(idx_recv, idx_expr, _) = &inner_expr.kind {
                    if self.first_pass.is_span_index_by_range(&inner_expr.span) {
                        let mut_str = if mutbl.is_mut() { "_mut" } else { "" };

                        let recv_src = pprust::expr_to_string(idx_recv);
                        let idx_src = pprust::expr_to_string(idx_expr);
                        let code = format!("{recv_src}.subslice{mut_str}({idx_src})");
                        *expr = common::parse_expr(self.psess, code);
                        return;
                    }
                }

                // Uniformly rewrite &x / &mut x into a TaggedRef /
                // TaggedRefMut when pass 1 confirmed the operand is not
                // already a reference. Outer & over a ref (&&u32,
                // &&mut u32) carries no Id and stays as a plain ref. For
                // ref-typed args to untracked functions, the subsequent .1
                // field access in the Call arm turns x.as_tagged_ref() into
                // &T - the shape the callee expects.
                if self.first_pass.is_span_ref_to_tupleable_ty(&expr.span) {
                    self.transform_to_tagged_ref(expr);
                }
            }

            // If this expression constructs an array, create a TaggedArray by using the runtime
            // library ATI::track_array call. This call will also merge all contained value tags
            // into the same AT.
            ast::ExprKind::Array(_) | ast::ExprKind::Repeat(_, _) => {
                *expr = self.tuplify_array(expr);
            }

            // Convert all invocations of untracked functions
            // to use the un-tupled values, then bringing the return
            // back into a TaggedValue if necessary.
            ast::ExprKind::Call(func, args) => {
                if let ast::ExprKind::Path(_, path) = &mut func.kind {
                    // Update turbofish generics: f::<u32> -> f::<Tagged<u32>>
                    for segment in path.segments.iter_mut() {
                        self.tuple_generic_args_in_segment(segment);
                    }

                    if let Some(is_tupleable) =
                        self.first_pass.is_untracked_call_ret_tupleable(&func.span)
                    {
                        for arg_expr in args.iter_mut() {
                            *arg_expr = self.untuple(arg_expr.clone());
                        }

                        // FIXME: again, this is a bit wrong. We are currently ignoring the tracked/untracked boundary,
                        // but you can imagine that an untracked func call returns some struct, which itself contains
                        // values that need to be converted into Tagged<T>s. Right now, that case is entirely ignored,
                        // this works properly if the returned value is a simple primitive.
                        if is_tupleable {
                            *expr = self.tuplify_expr(expr);
                        }
                    }
                }
            }

            // Update turbofish generics on method calls
            // at some point this should include more tracked/untracked boundary logic?
            ast::ExprKind::MethodCall(box ast::MethodCall { seg, .. }) => {
                self.tuple_generic_args_in_segment(seg);
            }

            // Transform binary ops to include ATI_ANALYSIS calls to merge tags.
            ast::ExprKind::Binary(op, lhs, rhs) => {
                *expr = self.transform_binary_op(&lhs, op.node, &rhs);
            }

            // Transform similar to function transformation, tagging input / output types
            ast::ExprKind::Closure(box ast::Closure { fn_decl, .. }) => {
                for input in fn_decl.inputs.iter_mut() {
                    self.recursively_tuple_type(&mut input.ty)
                }

                if let ast::FnRetTy::Ty(ty) = &mut fn_decl.output {
                    self.recursively_tuple_type(ty);
                }
            }

            // After Binary transformation, comparison conditions produce Tagged<bool>.
            // Unwrap to raw bool so the if/while condition compiles.
            ast::ExprKind::If(cond, _, _) => {
                *cond = self.untuple(cond.clone());
            }

            ast::ExprKind::While(cond, _, _) => {
                *cond = self.untuple(cond.clone());
            }

            // Unary * on an instrumented &T/&mut T with tupleable T.
            // Post-instrumentation the operand is a TaggedRef(Mut?)<T>, and a
            // plain * would strip the tag (TaggedRef::deref to &T). Rebuild
            // a Tagged<T> from the borrowed fields so the id travels with the
            // value.
            ast::ExprKind::Unary(ast::UnOp::Deref, inner)
                if self.first_pass.is_tag_stripping_deref(&expr.span) =>
            {
                let code = format!(
                    "{{ let __tr = {}; Tagged(*__tr.0, *__tr.1) }}",
                    pprust::expr_to_string(inner),
                );
                *expr = common::parse_expr(self.psess, code);
            }

            // Transform range construction into a tracked-range constructor call.
            // By this point walk_expr has already instrumented the endpoints (so
            // literals/vars are Tagged<T>).
            ast::ExprKind::Range(lo, hi, limits) => {
                let is_inclusive = matches!(limits, ast::RangeLimits::Closed);
                let code = match (lo.as_ref(), hi.as_ref(), is_inclusive) {
                    (Some(lo), Some(hi), false) => format!(
                        "ATI::track_range({}, {})",
                        pprust::expr_to_string(lo),
                        pprust::expr_to_string(hi),
                    ),
                    (Some(lo), Some(hi), true) => format!(
                        "ATI::track_range_inclusive({}, {})",
                        pprust::expr_to_string(lo),
                        pprust::expr_to_string(hi),
                    ),
                    (Some(lo), None, _) => {
                        format!("ATI::track_range_from({})", pprust::expr_to_string(lo),)
                    }
                    (None, Some(hi), false) => {
                        format!("ATI::track_range_to({})", pprust::expr_to_string(hi),)
                    }
                    (None, Some(hi), true) => format!(
                        "ATI::track_range_to_inclusive({})",
                        pprust::expr_to_string(hi),
                    ),
                    (None, None, _) => "ATI::track_range_full()".to_string(),
                };
                *expr = common::parse_expr(self.psess, code);
            }

            _ => {}
        }
    }

    /// Wraps types in function signatures, struct definitions, and enum definitions.
    /// For functions, walks the body first (triggering visit_expr for expression
    /// instrumentation), then modifies the signature types.
    fn visit_item(&mut self, item: &mut ast::Item) {
        match &mut item.kind {
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                sig: ast::FnSig { decl, .. },
                body,
                ..
            }) => {
                if !self.first_pass.is_fn_ident_tracked(ident) {
                    return;
                }

                // instrument the actual code
                if let Some(body) = body {
                    mut_visit::walk_block(self, body);
                }

                // wrap parameter types in Tagged<T>, as necessary
                for param in &mut decl.inputs {
                    if matches!(
                        param.ty.kind,
                        ast::TyKind::Ref(
                            _,
                            ast::MutTy {
                                mutbl: ast::Mutability::Mut,
                                ..
                            }
                        )
                    ) {
                        let ast::PatKind::Ident(mode, _, _) = &mut param.pat.kind else {
                            panic!("AAAAAAAAAAAAAA");
                        };
                        mode.1 = ast::Mutability::Mut;
                    }

                    self.recursively_tuple_type(&mut param.ty);
                }

                // wrap return type in Tagged<T>
                if let ast::FnRetTy::Ty(return_type) = &mut decl.output {
                    self.recursively_tuple_type(return_type);
                }
            }

            // Tuple all tupleable types in all struct definitions
            ast::ItemKind::Struct(_, _, ast::VariantData::Struct { fields, .. })
            | ast::ItemKind::Struct(_, _, ast::VariantData::Tuple(fields, ..)) => {
                for field_def in fields.iter_mut() {
                    self.recursively_tuple_type(&mut field_def.ty);
                }
            }

            // Tuple all tupleable types in ever variant of all enum definitions
            ast::ItemKind::Enum(_ident, _, ast::EnumDef { variants }) => {
                for variant in variants.iter_mut() {
                    match &mut variant.data {
                        ast::VariantData::Struct { fields, .. } => {
                            for field in fields.iter_mut() {
                                self.recursively_tuple_type(&mut field.ty);
                            }
                        }
                        ast::VariantData::Tuple(fields, _) => {
                            for field in fields.iter_mut() {
                                self.recursively_tuple_type(&mut field.ty);
                            }
                        }
                        ast::VariantData::Unit(_) => {}
                    }
                }
            }

            // Instrument all methods
            ast::ItemKind::Impl(ast::Impl { items, .. }) => {
                for assoc_item in items.iter_mut() {
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        sig: ast::FnSig { decl, .. },
                        body,
                        ..
                    }) = &mut assoc_item.kind
                    else {
                        continue;
                    };

                    if !self.first_pass.is_fn_ident_tracked(ident) {
                        continue;
                    }

                    // instrument method body
                    if let Some(body) = body {
                        mut_visit::walk_block(self, body);
                    }

                    // tag all non-self parameter types
                    for param in &mut decl.inputs {
                        if !matches!(param.ty.peel_refs().kind, ast::TyKind::ImplicitSelf) {
                            self.recursively_tuple_type(&mut param.ty);
                        }
                    }

                    // wrap return type
                    if let ast::FnRetTy::Ty(ret_ty) = &mut decl.output {
                        self.recursively_tuple_type(ret_ty);
                    }
                }
            }

            _ => {}
        }
    }
}

impl<'a> TransformVisitor<'a> {
    /// Consutrctor
    pub fn new(
        datir_config: &'a DatirConfig,
        first_pass: &'a FirstPassInfo,
        psess: &'a ParseSess,
    ) -> Self {
        Self {
            datir_config,
            first_pass,
            psess,
            hoist_counter: 0,
        }
    }

    /// Walk the let's init expression post-order, hoisting every
    /// .as_tagged_ref(_mut?)() / .subslice(_mut?)() call whose receiver is a non-place rvalue.
    /// Each hoisted receiver becomes a preceding let __ati_hoist_N = <recv>; and the
    /// method call's receiver is replaced with a path to that local.
    fn maybe_hoist_ref_binding(&mut self, mut stmt: ast::Stmt) -> SmallVec<[ast::Stmt; 1]> {
        // ignore all statements that aren't a let with some init value
        let ast::StmtKind::Let(local) = &mut stmt.kind else {
            return smallvec![stmt];
        };
        let ast::LocalKind::Init(init) = &mut local.kind else {
            return smallvec![stmt];
        };

        let mut hoists: Vec<(String, ast::Expr)> = Vec::new();
        self.collect_as_tagged_ref_hoists(init, &mut hoists);
        if hoists.is_empty() {
            // nothing to hoist!
            return smallvec![stmt];
        }

        // Construct new list of statements that will replace original
        let mut out: SmallVec<[ast::Stmt; 1]> = SmallVec::with_capacity(hoists.len() + 1);
        for (name, recv) in hoists {
            // mut will sometimes be unnecessary... but always including it is safe
            // and avoids needing to determine whether a mutable reference is being
            // taken to this value.
            let code = format!("let mut {name} = {};", pprust::expr_to_string(&recv));
            let hoist_stmts = common::parse_stmts(self.psess, code);
            out.extend(hoist_stmts);
        }
        out.push(stmt);
        out
    }

    /// Post-order walk that collects hoists. For each .as_tagged_ref(_mut?)() /
    /// .subslice(_mut?)() call with a non-place receiver, swaps the receiver with a Path
    /// expression referencing a fresh __ati_hoist_N local and records the
    /// original receiver. Recurses into the original receiver first so nested
    /// chains (<t>.as_tagged_ref().as_tagged_ref()) produce a hoist per layer
    /// in dependency order (inner first).
    fn collect_as_tagged_ref_hoists(
        &mut self,
        expr: &mut ast::Expr,
        hoists: &mut Vec<(String, ast::Expr)>,
    ) {
        // Recurse into all exprs that could contain expressions that require hoisting
        match &mut expr.kind {
            ast::ExprKind::AddrOf(_, _, inner)
            | ast::ExprKind::Unary(_, inner)
            | ast::ExprKind::Field(inner, _)
            | ast::ExprKind::Paren(inner)
            | ast::ExprKind::Cast(inner, _)
            | ast::ExprKind::Repeat(inner, _) => {
                self.collect_as_tagged_ref_hoists(inner, hoists);
            }
            ast::ExprKind::Tup(elems) | ast::ExprKind::Array(elems) => {
                for e in elems {
                    self.collect_as_tagged_ref_hoists(e, hoists);
                }
            }
            ast::ExprKind::Call(f, args) => {
                self.collect_as_tagged_ref_hoists(f, hoists);
                for a in args {
                    self.collect_as_tagged_ref_hoists(a, hoists);
                }
            }
            ast::ExprKind::MethodCall(mc) => {
                self.collect_as_tagged_ref_hoists(&mut mc.receiver, hoists);
                for a in &mut mc.args {
                    self.collect_as_tagged_ref_hoists(a, hoists);
                }
            }
            ast::ExprKind::Index(base, idx, _) => {
                self.collect_as_tagged_ref_hoists(base, hoists);
                self.collect_as_tagged_ref_hoists(idx, hoists);
            }
            ast::ExprKind::Binary(op, lhs, rhs) => {
                if !matches!(op.node, ast::BinOpKind::And | ast::BinOpKind::Or) {
                    self.collect_as_tagged_ref_hoists(lhs, hoists);
                    self.collect_as_tagged_ref_hoists(rhs, hoists);
                }
            }
            ast::ExprKind::Range(s, e, _) => {
                if let Some(s) = s {
                    self.collect_as_tagged_ref_hoists(s, hoists);
                }
                if let Some(e) = e {
                    self.collect_as_tagged_ref_hoists(e, hoists);
                }
            }
            ast::ExprKind::Struct(se) => {
                for field in &mut se.fields {
                    self.collect_as_tagged_ref_hoists(&mut field.expr, hoists);
                }
            }
            _ => {}
        }

        let hoist_target = matches!(
            &expr.kind,
            ast::ExprKind::MethodCall(mc)
                if matches!(mc.seg.ident.name.as_str(), "as_tagged_ref" | "as_tagged_ref_mut" | "subslice" | "subslice_mut")
                    && !Self::is_place_expr(&mc.receiver)
        );
        if !hoist_target {
            return;
        }

        let ast::ExprKind::MethodCall(mc) = &mut expr.kind else {
            unreachable!();
        };
        let id = self.hoist_counter;
        self.hoist_counter += 1;
        let name = format!("__ati_hoist_{id}");

        let new_recv = common::parse_expr(self.psess, name.clone());
        let old_recv = std::mem::replace(&mut *mc.receiver, new_recv);
        hoists.push((name, old_recv));
    }

    /// An expression is a place, if it is one of the following kinds of expr.
    /// If this expression is not a place, then using it as a recv in a MethodCall
    /// would create a temporary that gets dropped on return.
    fn is_place_expr(expr: &ast::Expr) -> bool {
        matches!(
            expr.kind,
            ast::ExprKind::Path(..)
                | ast::ExprKind::Field(..)
                | ast::ExprKind::Index(..)
                | ast::ExprKind::Unary(ast::UnOp::Deref, _)
        )
    }

    /// Converts an &(mut?)<inner> expression into a <inner>.as_tagged_ref(_mut?)()
    /// call, so that a reference to both the Id and inner value is obtained.
    fn transform_to_tagged_ref(&mut self, expr: &mut ast::Expr) {
        let ast::ExprKind::AddrOf(kind, mutbl, inner_expr) = &mut expr.kind else {
            panic!("Attempting to create a tagged ref out of a non-ref expr");
        };

        let mut_str = if mutbl.is_mut() { "_mut" } else { "" };
        let recv = inner_expr.clone();
        expr.kind = ast::ExprKind::MethodCall(Box::new(ast::MethodCall {
            seg: ast::PathSegment::from_ident(Ident::from_str(&format!("as_tagged_ref{mut_str}"))),
            receiver: recv,
            args: [].into(),
            span: DUMMY_SP,
        }));
    }

    /// Transforms `lhs op rhs` (where we expect both operands are Tagged<T>s) into a block that
    /// explicitly calls ATI_ANALYSIS functions to record the interaction and constructs the result.
    fn transform_binary_op(&self, lhs: &ast::Expr, op: BinOpKind, rhs: &ast::Expr) -> ast::Expr {
        let lhs_str = pprust::expr_to_string(lhs);
        let rhs_str = pprust::expr_to_string(rhs);
        let op_str = op.as_str();

        // FIXME: Kind of stupid to go from lhs op rhs to lhs op rhs in arithemtic case
        let block_str = match Self::op_type(op) {
            OpKind::Comparison => {
                // guaranteed that lhs op rhs will return a regular bool
                format!(
                    r#"{{
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                        Tagged(__ati_id, {lhs_str} {op_str} {rhs_str})
                    }}"#
                )
            }
            OpKind::Logical => {
                // guaranteed that lhs and rhs were bools, and are now Tagged<bool>s
                format!(
                    r#"{{
                        let __ati_lhs = {lhs_str};
                        let __ati_rhs = {rhs_str};
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().union_and_get_id(&__ati_lhs.0, &__ati_rhs.0);
                        Tagged(__ati_id, __ati_lhs.1 {op_str} __ati_rhs.1)
                    }}"#
                )
            }
            OpKind::Arithmetic => {
                // handled by op impls on Tagged<T>
                format!(
                    r#"{{
                        ({lhs_str} {op_str} {rhs_str})
                    }}"#
                )
            }
        };

        // self.datir_config.log("TMP", format!("======\n{block_str}"));

        common::parse_expr(self.psess, block_str)
    }

    /// Converts between rustc's BinOpKind type to DATIR's OpKind type
    fn op_type(op: BinOpKind) -> OpKind {
        match op {
            BinOpKind::Eq
            | BinOpKind::Ne
            | BinOpKind::Lt
            | BinOpKind::Gt
            | BinOpKind::Le
            | BinOpKind::Ge => OpKind::Comparison,

            BinOpKind::BitXor
            | BinOpKind::BitAnd
            | BinOpKind::BitOr
            | BinOpKind::Shl
            | BinOpKind::Shr
            | BinOpKind::Add
            | BinOpKind::Sub
            | BinOpKind::Mul
            | BinOpKind::Div
            | BinOpKind::Rem => OpKind::Arithmetic,

            BinOpKind::And | BinOpKind::Or => OpKind::Logical,
        }
    }

    /// Converts from a [T; N] to a Tagged<[T; N]>
    fn tuplify_array(&self, expr: &mut ast::Expr) -> ast::Expr {
        let mut receiver_expr = ast::Expr::dummy();
        receiver_expr.kind = ast::ExprKind::Path(
            None,
            ast::Path {
                span: DUMMY_SP,
                segments: [
                    ast::PathSegment {
                        ident: Ident::from_str("ATI"),
                        id: DUMMY_NODE_ID,
                        args: None,
                    },
                    ast::PathSegment {
                        ident: Ident::from_str("track_array"),
                        id: DUMMY_NODE_ID,
                        args: None,
                    },
                ]
                .into(),
                tokens: None,
            },
        );

        let mut new_expr = ast::Expr::dummy();
        new_expr.kind =
            ast::ExprKind::Call(Box::new(receiver_expr), [Box::new(expr.clone())].into());

        new_expr
    }

    /// Takes an expression of type T and converts it to an expression of Tagged<T>
    fn tuplify_expr(&self, expr: &ast::Expr) -> ast::Expr {
        ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::Call(
                Box::new(ast::Expr {
                    id: ast::DUMMY_NODE_ID,
                    kind: ast::ExprKind::Path(
                        None,
                        ast::Path {
                            span: DUMMY_SP,
                            segments: [
                                ast::PathSegment {
                                    ident: Ident::from_str("ATI"),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                                ast::PathSegment {
                                    ident: Ident::from_str("track"),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                            ]
                            .into(),
                            tokens: None,
                        },
                    ),
                    span: DUMMY_SP,
                    attrs: [].into(),
                    tokens: None,
                }),
                [Box::new(expr.clone())].into(),
            ),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        }
    }

    /// Takes a Tagged<T> expression and unwraps it to just T
    fn untuple(&self, expr: Box<ast::Expr>) -> Box<ast::Expr> {
        let mut node = ast::Expr::dummy();
        node.kind = ast::ExprKind::Field(expr, Ident::from_str("1"));
        Box::new(node)
    }

    ///////////////// Type Wrapping Helpers //////////////////////

    /// Directly modifies a type T into a Tagged<T> in place
    fn tuple_type(&self, old_type: &mut ast::Ty) {
        old_type.kind = ast::TyKind::Path(
            None,
            ast::Path {
                segments: [ast::PathSegment {
                    ident: Ident::from_str("Tagged"),
                    id: ast::DUMMY_NODE_ID,
                    args: Some(Box::new(ast::AngleBracketed(ast::AngleBracketedArgs {
                        span: DUMMY_SP,
                        args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                            Box::new(old_type.clone()),
                        ))]
                        .into(),
                    }))),
                }]
                .into(),
                span: DUMMY_SP,
                tokens: None,
            },
        );
    }

    /// Directly modifies a type `T` into `TaggedRef(Mut?)<T>`.
    /// The caller is responsible for having already
    /// tupled any sub-element types (e.g. the element type of a slice/array);
    /// this helper only wraps the outer shape.
    fn wrap_ty_as_tagged_ref(&self, outer_ty: &mut ast::Ty, mutable: bool) {
        // the wrapper should see the inner type (the Ref's referent).
        // extract the referent and preserve the source
        // lifetime so `&'a T` becomes `TaggedRef<'a, T>`.
        let (lifetime, inner) = match &mut outer_ty.kind {
            ast::TyKind::Ref(lt, ast::MutTy { box ty, .. }) => (lt.clone(), ty.clone()),
            _ => panic!("AAAAAA"),
        };

        let name = if mutable { "TaggedRefMut" } else { "TaggedRef" };
        let mut seg = ast::PathSegment::from_ident(Ident::from_str(name));

        let mut args: Vec<ast::AngleBracketedArg> = Vec::new();
        if let Some(lt) = lifetime {
            args.push(ast::AngleBracketedArg::Arg(ast::GenericArg::Lifetime(lt)));
        }
        args.push(ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
            Box::new(inner),
        )));

        seg.args = Some(Box::new(GenericArgs::AngleBracketed(
            ast::AngleBracketedArgs {
                span: DUMMY_SP,
                args: args.into(),
            },
        )));
        outer_ty.kind = ast::TyKind::Path(
            None,
            ast::Path {
                span: DUMMY_SP,
                segments: [seg].into(),
                tokens: None,
            },
        );
    }

    /// Converts a [T; N] into a Tagged<[T; N]>
    fn tuple_array(&self, array_ty: &mut ast::Ty) {
        let mut tagged_array = ast::PathSegment::from_ident(Ident::from_str("Tagged"));
        tagged_array.args = Some(Box::new(GenericArgs::AngleBracketed(
            ast::AngleBracketedArgs {
                span: DUMMY_SP,
                args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                    Box::new(array_ty.clone()),
                ))]
                .into(),
            },
        )));

        array_ty.kind = ast::TyKind::Path(
            None,
            ast::Path {
                span: DUMMY_SP,
                segments: [tagged_array].into(),
                tokens: None,
            },
        );
    }

    /// Recursively tuples all type generic arguments in a path segment
    fn tuple_generic_args_in_segment(&self, segment: &mut ast::PathSegment) {
        let Some(ref mut boxed_args) = segment.args else {
            return;
        };
        let ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs { ref mut args, .. }) =
            **boxed_args
        else {
            return;
        };
        for arg in args.iter_mut() {
            if let ast::AngleBracketedArg::Arg(ast::GenericArg::Type(ty)) = arg {
                self.recursively_tuple_type(ty);
            }
        }
    }

    /// Searches through type `ty` to find and tuple all atomic types
    /// that should be tupled. Modifies the type in place.
    fn recursively_tuple_type(&self, outer_ty: &mut ast::Ty) {
        // Reference shapes are the interesting case: &T, &[T], &[T; N],
        // and nested refs each map to a different instrumented target.
        if let ast::TyKind::Ref(_lt, ast::MutTy { box ty, mutbl }) = &mut outer_ty.kind {
            let mutable = mutbl.is_mut();

            // Nested refs (e.g. &&u32, &&[u32]): only the innermost &
            // carries an Id. Recurse into the inner Ref.
            if matches!(ty.kind, ast::TyKind::Ref(..) | ast::TyKind::Ptr(..)) {
                self.recursively_tuple_type(ty);
                return;
            }

            // &prim, &[T], &[T; N]: the outer & gets swallowed into a
            // TaggedRef(Mut)? wrapper. For slices and arrays, tuple the element
            // type in place first so the inner shape becomes [Tag(T)] / [Tag(T); N].
            let collapsible = ty.can_be_tupled()
                || matches!(ty.kind, ast::TyKind::Slice(_) | ast::TyKind::Array(_, _));
            if collapsible {
                match &mut ty.kind {
                    ast::TyKind::Slice(elem) => self.recursively_tuple_type(elem),
                    ast::TyKind::Array(elem, _) => self.recursively_tuple_type(elem),
                    _ => {} // atomic primitive.
                }
                self.wrap_ty_as_tagged_ref(outer_ty, mutable);
                return;
            }

            // otherwise, keep ref as just &
            self.recursively_tuple_type(ty);
            return;
        }

        // Owned primitive - wrap as `Tagged<prim>`.
        if outer_ty.can_be_tupled() {
            self.tuple_type(outer_ty);
            return;
        }

        match &mut outer_ty.kind {
            // Standalone [T] shouldn't appear - slice types only exist behind
            // a pointer. If we hit one anyway, tuple the element and leave the
            // outer shape alone rather than panicking?
            rustc_ast::TyKind::Slice(inner_ty) => {
                self.datir_config.log("Warning", format!("Found a slice type not behind a reference!"));
                self.recursively_tuple_type(inner_ty);
            }

            // Owned array [T; N] - tuple the element, wrap in Tagged<>.
            rustc_ast::TyKind::Array(inner_ty, _) => {
                self.recursively_tuple_type(inner_ty);
                self.tuple_array(outer_ty);
            }

            rustc_ast::TyKind::Ptr(ast::MutTy { box ty, .. }) => {
                self.recursively_tuple_type(ty);
            }

            // Ref is handled at the top of this function before the match.
            rustc_ast::TyKind::Ref(..) => unreachable!(),

            rustc_ast::TyKind::FnPtr(box ast::FnPtrTy {
                generic_params,
                decl: box ast::FnDecl { inputs, output },
                ..
            }) => {
                for generic in generic_params {
                    match &mut generic.kind {
                        rustc_ast::GenericParamKind::Const { ty, default, .. } => {
                            self.recursively_tuple_type(ty);
                            // FIXME: handle the default value. An AnonConst isn't computed
                            // at runtime though, so how are we associating an Id with it?
                        }
                        _ => {} // Pretty certain we want to leave generics alone
                                // rustc_ast::GenericParamKind::Type { default } => {
                                //     if let Some(ty) = default {
                                //         self.recursively_tuple_type(ty);
                                //     }
                                // }
                                // rustc_ast::GenericParamKind::Lifetime => {}
                    }
                }

                for input in inputs {
                    self.recursively_tuple_type(&mut input.ty)
                }

                if let ast::FnRetTy::Ty(box ty) = output {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Tup(tys) => {
                for ty in tys {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Path(_, ast::Path { segments, .. }) => {
                for segment in segments.iter_mut() {
                    if let Some(box arg) = &mut segment.args {
                        match arg {
                            rustc_ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                                args,
                                ..
                            }) => {
                                for arg in args.iter_mut() {
                                    match arg {
                                        rustc_ast::AngleBracketedArg::Arg(generic_arg) => {
                                            match generic_arg {
                                                rustc_ast::GenericArg::Type(ty) => {
                                                    self.recursively_tuple_type(ty);
                                                }
                                                rustc_ast::GenericArg::Const(_)
                                                | rustc_ast::GenericArg::Lifetime(_) => {}
                                            }
                                        }
                                        rustc_ast::AngleBracketedArg::Constraint(_) => {
                                            todo!("Constraint is a trait?")
                                        }
                                    }
                                }
                            }
                            rustc_ast::GenericArgs::Parenthesized(ast::ParenthesizedArgs {
                                inputs,
                                output,
                                ..
                            }) => {
                                for input in inputs {
                                    self.recursively_tuple_type(input);
                                }

                                if let ast::FnRetTy::Ty(box ty) = output {
                                    self.recursively_tuple_type(ty);
                                }
                            }
                            rustc_ast::GenericArgs::ParenthesizedElided(_span) => {
                                panic!("this panic is probably fine to remove")
                            }
                        }
                    }
                }

                // FIXME: this is not resilient to custom types that are called "range".
                // If this path refers to a std range type, wrap the whole
                // type in Tagged so `std::ops::Range<usize>` becomes
                // `Tagged<std::ops::Range<Tagged<usize>>>`.
                if let Some(last) = segments.last() {
                    let name = last.ident.name.as_str();
                    if matches!(
                        name,
                        "Range"
                            | "RangeInclusive"
                            | "RangeFrom"
                            | "RangeTo"
                            | "RangeToInclusive"
                            | "RangeFull"
                    ) {
                        self.tuple_type(outer_ty);
                    }
                }
            }

            // maybe impl later
            rustc_ast::TyKind::PinnedRef(_, _) => todo!(),
            rustc_ast::TyKind::Pat(_, _) => todo!(),

            // probably left untouched
            rustc_ast::TyKind::Infer => {}
            rustc_ast::TyKind::TraitObject(_, _) => panic!(),
            rustc_ast::TyKind::Paren(_) => panic!(),
            rustc_ast::TyKind::UnsafeBinder(_) => panic!(),
            rustc_ast::TyKind::Never => panic!(),
            rustc_ast::TyKind::ImplTrait(_, _) => panic!(),
            rustc_ast::TyKind::ImplicitSelf => panic!(),
            rustc_ast::TyKind::MacCall(_) => panic!(),
            rustc_ast::TyKind::CVarArgs => panic!(),
            rustc_ast::TyKind::Dummy => panic!(),
            rustc_ast::TyKind::Err(_) => panic!(),
            rustc_ast::TyKind::FieldOf(_, _, _) => panic!(),
        };
    }
}
