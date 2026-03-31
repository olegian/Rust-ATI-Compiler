/* Defines a visitor which tags all primitives that can be tagged,
 * based on the common::can_literal_be_tupled function. Further,
 * finds uses of these values that require them to be "untupled"
 * within tracked functions (like when passed to an untracked function),
 * unbinding the tag from the value in that case (TaggedValue<T> -> T).
 *
 * Binary expressions between tagged values are transformed into explicit
 * blocks that call ATI_ANALYSIS methods directly, rather than relying on
 * operator overloads on Tagged<T>.
*/
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::{self as ast, BinOpKind, DUMMY_NODE_ID};
use rustc_ast_pretty::pprust;
use rustc_session::parse::ParseSess;
use rustc_span::{DUMMY_SP, Ident};

use crate::common;
use crate::types::ati_info::FirstPassInfo;

pub struct TupleLiteralsVisitor<'a> {
    first_pass: &'a FirstPassInfo,
    psess: &'a ParseSess,
}

impl<'a> MutVisitor for TupleLiteralsVisitor<'a> {
    // define to stop visitor from modifying any expressions used as types
    fn visit_param(&mut self, _node: &mut ast::Param) {}
    fn visit_anon_const(&mut self, _node: &mut rustc_ast::AnonConst) {}

    /// Converts all literals into TaggedValue<T>'s,
    /// transforms Binary nodes into explicit union+Tagged blocks,
    /// and unwraps Tagged<bool> conditions in If/While to raw bool.
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        mut_visit::walk_expr(self, expr);

        match &mut expr.kind {
            // Convert all literals into TaggedValues, if necessary
            ast::ExprKind::Lit(lit) => {
                if common::can_literal_be_tupled(&lit) {
                    *expr = self.tuplify_expr(expr);
                }
            }

            ast::ExprKind::AddrOf(borrow_kind, mutbl, inner_expr) => {
                if self.first_pass.is_span_ref_type_coercion(&expr.span) {
                    *expr = self.array_to_slice(expr);
                }
            }

            ast::ExprKind::Array(_) | ast::ExprKind::Repeat(_, _) => {
                *expr = self.tuplify_array(expr);
            }

            // Convert all invocations of untracked functions
            // to use the un-tupled values, then bringing the return
            // back into a TaggedValue
            ast::ExprKind::Call(func, args) => {
                if let ast::ExprKind::Path(_, _) = &func.kind {
                    if let Some(is_tupleable) = self.first_pass.is_untracked_call_ret_tupleable(&func.span)
                    {
                        for arg_expr in args.iter_mut() {
                            *arg_expr = self.untuple(arg_expr.clone());
                        }

                        if is_tupleable {
                            *expr = self.tuplify_expr(expr);
                        }
                    }
                }
            }

            // Transform binary ops into explicit ATI_ANALYSIS calls + Tagged construction.
            // For arithmetic/logical ops: both inputs and the output are in the same abstract type.
            // For comparison ops: inputs are merged, output gets a fresh independent id.
            ast::ExprKind::Binary(op, lhs, rhs) => {
                let op_node = op.node;
                let lhs_clone = lhs.as_ref().clone();
                let rhs_clone = rhs.as_ref().clone();
                *expr = self.transform_binary_op(&lhs_clone, op_node, &rhs_clone);
            }

            // Transform compound assignment ops (+=, -=, etc.) into explicit ATI_ANALYSIS
            // calls that mutate the LHS in place. The generated block evaluates to () to
            // match the normal semantics of an assignment expression.
            ast::ExprKind::AssignOp(op, lhs, rhs) => {
                let op_node = op.node.into();
                let lhs_clone = lhs.as_ref().clone();
                let rhs_clone = rhs.as_ref().clone();
                *expr = self.transform_assign_op(&lhs_clone, op_node, &rhs_clone);
            }

            // After Binary transformation, comparison conditions produce Tagged<bool>.
            // Unwrap to raw bool so the if/while condition compiles.
            ast::ExprKind::If(cond, _, _) | ast::ExprKind::While(cond, _, _) => {
                *cond = self.untuple(cond.clone());
            }

            ast::ExprKind::Index(receiver_expr, index_expr, _) => {
                // TODO: does this work for ranges? It should... but I'm skeptical.
                // if the returned value is a slice for instance, then shouldn't we create
                // a Tagged<[Tagged<T>]> type of return? what tag should the length take on?
                *receiver_expr = self.untuple(receiver_expr.clone());
                *index_expr = self.untuple(index_expr.clone());
            }

            ast::ExprKind::MacCall(box ast::MacCall { path: _, args: _ }) => {}

            ast::ExprKind::MethodCall(box ast::MethodCall {
                seg: _,
                receiver: _,
                args: _,
                span: _,
            }) => {}

            _ => {}
        }
    }
}

enum OpKind {
    Comparison,
    Arithmetic,
    Bitwise,
}

impl<'a> TupleLiteralsVisitor<'a> {
    pub fn new(first_pass: &'a FirstPassInfo, psess: &'a ParseSess) -> Self {
        Self { first_pass, psess }
    }

    /// Transforms `lhs op rhs` (where both operands are Tagged<T>) into a block that
    /// explicitly calls ATI_ANALYSIS to record the interaction and constructs the result.
    ///
    /// For arithmetic/logical ops the result id is the leader after union (same abstract type).
    /// For comparison ops the result gets a fresh id (independent abstract type for the bool).
    fn transform_binary_op(&self, lhs: &ast::Expr, op: BinOpKind, rhs: &ast::Expr) -> ast::Expr {
        let lhs_str = pprust::expr_to_string(lhs);
        let rhs_str = pprust::expr_to_string(rhs);
        let op_str = Self::binop_str(op);

        let block_str = match Self::op_type(op) {
            // in all of these branches, lhs and rhs need to be pulled out first to
            // not let lock acquire on the union_and_get_id call to overlap with any
            // lock acquires that happen when evaluating the lhs or rhs
            OpKind::Comparison => {
                format!(
                    r#"{{
                        let __ati_lhs = {lhs_str};
                        let __ati_rhs = {rhs_str};
                        ATI_ANALYSIS.lock().unwrap().union_and_get_id(&__ati_lhs.0, &__ati_rhs.0);
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                        Tagged(__ati_id, __ati_lhs.1 {op_str} __ati_rhs.1)
                    }}"#
                )
            }
            OpKind::Arithmetic => {
                format!(
                    r#"{{
                        let __ati_lhs = {lhs_str};
                        let __ati_rhs = {rhs_str};
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().union_and_get_id(&__ati_lhs.0, &__ati_rhs.0);
                        Tagged(__ati_id, __ati_lhs.1 {op_str} __ati_rhs.1)
                    }}"#
                )
            }
            OpKind::Bitwise => {
                format!(
                    r#"{{
                        let __ati_lhs = {lhs_str};
                        let __ati_rhs = {rhs_str};
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                        Tagged(__ati_id, __ati_lhs.1 {op_str} __ati_rhs.1)
                    }}"#
                )
            }
        };

        common::parse_expr(self.psess, block_str)
    }

    /// Transforms `lhs op= rhs` into a block that records the interaction and
    /// writes the result back to the LHS place expression. The block evaluates
    /// to `()`, matching the normal semantics of a compound assignment.
    ///
    /// For arithmetic ops the LHS and RHS end up in the same abstract type.
    /// For bitwise ops the result gets a fresh, independent id.
    /// Comparison ops cannot appear as compound assignments and are unreachable.
    fn transform_assign_op(&self, lhs: &ast::Expr, op: BinOpKind, rhs: &ast::Expr) -> ast::Expr {
        let lhs_str = pprust::expr_to_string(lhs);
        let rhs_str = pprust::expr_to_string(rhs);
        let op_str = Self::binop_str(op);

        // The RHS is captured first so that any lock acquired while evaluating it
        // does not overlap with the lock acquired inside union_and_get_id / make_id.
        let block_str = match Self::op_type(op) {
            OpKind::Arithmetic => {
                format!(
                    r#"{{
                        let __ati_rhs = {rhs_str};
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().union_and_get_id(&{lhs_str}.0, &__ati_rhs.0);
                        {lhs_str} = Tagged(__ati_id, {lhs_str}.1 {op_str} __ati_rhs.1);
                    }}"#
                )
            }
            OpKind::Bitwise => {
                format!(
                    r#"{{
                        let __ati_rhs = {rhs_str};
                        let __ati_id = ATI_ANALYSIS.lock().unwrap().make_id();
                        {lhs_str} = Tagged(__ati_id, {lhs_str}.1 {op_str} __ati_rhs.1);
                    }}"#
                )
            }
            OpKind::Comparison => {
                unreachable!("compound-assignment operators cannot be comparison operators")
            }
        };

        common::parse_expr(self.psess, block_str)
    }

    fn op_type(op: BinOpKind) -> OpKind {
        match op {
            BinOpKind::Eq
            | BinOpKind::Ne
            | BinOpKind::Lt
            | BinOpKind::Gt
            | BinOpKind::Le
            | BinOpKind::Ge => OpKind::Comparison,

            BinOpKind::Add
            | BinOpKind::Sub
            | BinOpKind::Mul
            | BinOpKind::Div
            | BinOpKind::Rem
            | BinOpKind::And
            | BinOpKind::Or => OpKind::Arithmetic,

            BinOpKind::BitXor
            | BinOpKind::BitAnd
            | BinOpKind::BitOr
            | BinOpKind::Shl
            | BinOpKind::Shr => OpKind::Bitwise,
        }
    }

    fn binop_str(op: BinOpKind) -> &'static str {
        match op {
            BinOpKind::Add => "+",
            BinOpKind::Sub => "-",
            BinOpKind::Mul => "*",
            BinOpKind::Div => "/",
            BinOpKind::Rem => "%",
            BinOpKind::And => "&&",
            BinOpKind::Or => "||",
            BinOpKind::Eq => "==",
            BinOpKind::Ne => "!=",
            BinOpKind::Lt => "<",
            BinOpKind::Gt => ">",
            BinOpKind::Le => "<=",
            BinOpKind::Ge => ">=",

            // BITWISE OPS DONT MAKE ANY SENSE TO TRANSFORM
            BinOpKind::BitXor => "^",
            BinOpKind::BitAnd => "&",
            BinOpKind::BitOr => "|",
            // AND NEITTHER DO THESE??
            BinOpKind::Shl => "<<",
            BinOpKind::Shr => ">>",
        }
    }

    fn array_to_slice(&self, expr: &mut ast::Expr) -> ast::Expr {
        let ast::ExprKind::AddrOf(borrow_kind, mutbl, inner_expr) = expr.kind.clone() else {
            unimplemented!(
                "Only reference-based fat pointers are supported for array -> slice coercion"
            );
        };

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
                        ident: Ident::from_str("track_slice"),
                        id: DUMMY_NODE_ID,
                        args: None,
                    },
                ]
                .into(),
                tokens: None,
            },
        );

        let mut call_expr = ast::Expr::dummy();
        call_expr.kind =
            ast::ExprKind::Call(Box::new(receiver_expr), [Box::new(expr.clone())].into());

        let mut new_expr = ast::Expr::dummy();
        new_expr.kind = ast::ExprKind::AddrOf(borrow_kind, mutbl, Box::new(call_expr));

        new_expr
    }

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

    /// Takes an expression of type T and converts it to an expression of TaggedValue<T>,
    /// by using the ATI::track function from ati.rs
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

    /// Takes a TaggedValue<T> expression and unwraps it to just T,
    /// by accessing the TaggedValue's 0'th field.
    fn untuple(&self, expr: Box<ast::Expr>) -> Box<ast::Expr> {
        let mut node = ast::Expr::dummy();
        node.kind = ast::ExprKind::Field(expr, Ident::from_str("1"));
        Box::new(node)
    }
}
