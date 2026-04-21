/* Provides helper functions that are used throughout this entire project.
 * Namely, this includes determining the set of types that are considered
 * able to be tagged, as well as moving between different representations of types.
*/
use rustc_ast::token::{Lit, LitKind};
use rustc_ast::{self as ast};
use rustc_middle as mir;
use rustc_span::sym;

/// Determines whether a type is a tracked primitive that can be wrapped in `Tagged<T>`.
/// Defines as a trait so that it can be shared between both MIR and AST types
// IMPORTANT: THE BELOW IMPLS NEED TO BE KEPT IN SYNC.
pub trait CanBeTupled {
    fn can_be_tupled(&self) -> bool;
}

impl CanBeTupled for ast::Ty {
    fn can_be_tupled(&self) -> bool {
        let ty = self.peel_refs();
        let Some(ty_sym) = ty.kind.is_simple_path() else {
            return false;
        };

        matches!(
            ty_sym,
            sym::i8
                | sym::i16
                | sym::i32
                | sym::i64
                | sym::i128
                | sym::u8
                | sym::u16
                | sym::u32
                | sym::u64
                | sym::u128
                | sym::f16
                | sym::f32
                | sym::f64
                | sym::f128
                | sym::isize
                | sym::usize
                | sym::bool
                | sym::char
        )
    }
}

impl CanBeTupled for mir::ty::Ty<'_> {
    fn can_be_tupled(&self) -> bool {
        self.is_integral() || self.is_floating_point() || self.is_bool() || self.is_char()
    }
}

impl CanBeTupled for Lit {
    fn can_be_tupled(&self) -> bool {
        match self.kind {
            LitKind::Integer | LitKind::Float | LitKind::Bool | LitKind::Char => true,
            _ => false,
        }
    }
}
