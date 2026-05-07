//! Provides helper functions which determine whether or not a particular type can be tupled.
//!
//! Throughout DATIR, there are many places where we ask "is this type tuplable?". Note that types
//! within rustc use different representations within different IRs. [`CanBeTupled`] is a trait
//! which allows these different type representations to have a single method defined which
//! answers the question.
//!
//! IMPORTANT NOTE: The below implementations need to remain in sync for consistency.

/// Determines whether a type is a tracked primitive that can be wrapped in `Tagged<T>`.
pub trait CanBeTupled {
    fn can_be_tupled(&self) -> bool;
}

impl CanBeTupled for rustc_ast::Ty {
    /// Returns true if the AST type can be directly wrapped in `Tagged<T>`.
    fn can_be_tupled(&self) -> bool {
        let ty = self.peel_refs();
        let Some(ty_sym) = ty.kind.is_simple_path() else {
            return false;
        };

        matches!(
            ty_sym,
            rustc_span::sym::i8
                | rustc_span::sym::i16
                | rustc_span::sym::i32
                | rustc_span::sym::i64
                | rustc_span::sym::i128
                | rustc_span::sym::u8
                | rustc_span::sym::u16
                | rustc_span::sym::u32
                | rustc_span::sym::u64
                | rustc_span::sym::u128
                | rustc_span::sym::f16
                | rustc_span::sym::f32
                | rustc_span::sym::f64
                | rustc_span::sym::f128
                | rustc_span::sym::isize
                | rustc_span::sym::usize
                | rustc_span::sym::bool
                | rustc_span::sym::char
        )
    }
}

impl CanBeTupled for rustc_middle::ty::Ty<'_> {
    /// Returns true if the MIR type can be directly wrapped in `Tagged<T>`.
    fn can_be_tupled(&self) -> bool {
        self.is_integral() || self.is_floating_point() || self.is_bool() || self.is_char()
    }
}

impl CanBeTupled for rustc_ast::token::Lit {
    /// Returns true if the AST literal type can be directly wrapped in `Tagged<T>`.
    fn can_be_tupled(&self) -> bool {
        match self.kind {
            rustc_ast::token::LitKind::Integer
            | rustc_ast::token::LitKind::Float
            | rustc_ast::token::LitKind::Bool
            | rustc_ast::token::LitKind::Char => true,
            _ => false,
        }
    }
}
