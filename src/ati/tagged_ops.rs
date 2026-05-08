//! Operator-trait implementations for the tagged wrappers.
//!
//! Pass 2 leaves arithmetic, comparison, and shift operators looking like ordinary Rust code,
//! so the standard library's overloaded operator dispatch handles instrumentation through
//! the impls in this file. Each operator unions the operand ids in the
//! [ATI_ANALYSIS] value union-find before delegating to the
//! wrapped primitive.
//!
//! Comparison operators ([PartialEq], [PartialOrd], [Eq], [Ord]) are covered for all nine
//! ordered pairs of [Tagged] / [TaggedRef] / [TaggedRefMut]. Arithmetic and bitwise operators 
//! (`+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`) merge the lhs, rhs, and result ids. Shift operators
//! (`<<`, `>>`) merge only the lhs and the result, since the rhs is treated as a count rather
//! than a value-level interaction. Unary `Neg` and `Not` push down to the underlying value
//! while keeping the id intact, and `Deref` on [Tagged] enables auto-deref to any `T` method.

use crate::ati::{
    ati::ATI_ANALYSIS,
    refs::{TaggedRef, TaggedRefMut},
    tagged::{TagTuple, Tagged},
};

// =====================    COMPARISON OPS    ===================
// Comparison goes through the [TagTuple] trait, which exposes `.id()` and `.value()` on every
// tagged wrapper. The macros below take advantage of that to avoid matching on each shape
// separately.

/// `PartialEq` and `PartialOrd` between an `lhs` and `rhs` (both of which must impl
/// [TagTuple]). Unions the tags on every call before delegating comparison to the underlying
/// `T`.
macro_rules! impl_tagged_partial_cmp {
    ($($gens:tt),+ ; $lhs:ty, $rhs:ty) => {
        impl<$($gens),+> std::cmp::PartialEq<$rhs> for $lhs
        where
            T: std::cmp::PartialEq,
        {
            fn eq(&self, other: &$rhs) -> bool {
                ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.id(), &other.id());
                self.value().eq(other.value())
            }
        }

        impl<$($gens),+> std::cmp::PartialOrd<$rhs> for $lhs
        where
            T: std::cmp::PartialOrd,
        {
            fn partial_cmp(&self, other: &$rhs) -> Option<std::cmp::Ordering> {
                ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.id(), &other.id());
                self.value().partial_cmp(other.value())
            }
        }
    };
}

/// `Eq` and `Ord` for a self-type comparison. `Eq` and `Ord` require the same type on both
/// sides, so only the three self-self cases are valid.
macro_rules! impl_tagged_total_cmp {
    ($($gens:tt),+ ; $ty:ty) => {
        impl<$($gens),+> std::cmp::Eq for $ty where T: std::cmp::Eq {}

        impl<$($gens),+> std::cmp::Ord for $ty
        where
            T: std::cmp::Ord,
        {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.id(), &other.id());
                self.value().cmp(other.value())
            }
        }
    };
}

// All nine ordered pairs of {Tagged, TaggedRef, TaggedRefMut}.
impl_tagged_partial_cmp!(T;             Tagged<T>,           Tagged<T>);
impl_tagged_partial_cmp!('a, T;         Tagged<T>,           TaggedRef<'a, T>);
impl_tagged_partial_cmp!('a, T;         Tagged<T>,           TaggedRefMut<'a, T>);
impl_tagged_partial_cmp!('a, T;         TaggedRef<'a, T>,    Tagged<T>);
impl_tagged_partial_cmp!('a, T;         TaggedRef<'a, T>,    TaggedRef<'a, T>);
impl_tagged_partial_cmp!('a, 'b, T;     TaggedRef<'a, T>,    TaggedRefMut<'b, T>);
impl_tagged_partial_cmp!('a, T;         TaggedRefMut<'a, T>, Tagged<T>);
impl_tagged_partial_cmp!('a, 'b, T;     TaggedRefMut<'a, T>, TaggedRef<'b, T>);
impl_tagged_partial_cmp!('a, T;         TaggedRefMut<'a, T>, TaggedRefMut<'a, T>);

impl_tagged_total_cmp!(T;       Tagged<T>);
impl_tagged_total_cmp!('a, T;   TaggedRef<'a, T>);
impl_tagged_total_cmp!('a, T;   TaggedRefMut<'a, T>);

// =====================    ARITHMETIC OPS    ===================

/// Arithmetic-style operators (`+`, `-`, `*`, `/`, `%`, `&`, `|`, `^`) and their assigning
/// counterparts. The result id is the union of the two operand ids in the value union-find,
/// and the wrapped value is computed by delegating to `T`'s own operator. Covers the four
/// owned/borrowed combinations (`Tagged op Tagged`, `Tagged op TaggedRef`, and the opposite
/// pairs) plus the two assigning variants.
macro_rules! impl_tagged_arithmetic_op {
    (
        $trait:ident,
        $method:ident,
        $assign_trait:ident,
        $assign_method:ident,
        $op:tt
    ) => {
        impl<T> std::ops::$trait for Tagged<T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Self) -> Self::Output {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.0, &rhs.0);
                Tagged(merged, self.1 $op rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait<TaggedRef<'a, T>> for Tagged<T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: TaggedRef<'a, T>) -> Self::Output {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.0, rhs.0);
                Tagged(merged, self.1 $op *rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait for TaggedRef<'a, T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Self) -> Self::Output {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(self.0, rhs.0);
                Tagged(merged, *self.1 $op *rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait<Tagged<T>> for TaggedRef<'a, T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Tagged<T>) -> Self::Output {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(self.0, &rhs.0);
                Tagged(merged, *self.1 $op rhs.1)
            }
        }

        impl<T> std::ops::$assign_trait for Tagged<T>
        where
            T: std::ops::$trait<Output = T> + Copy,
        {
            fn $assign_method(&mut self, rhs: Self) {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.0, &rhs.0);
                *self = Tagged(merged, self.1 $op rhs.1);
            }
        }

        impl<'a, T: Copy> std::ops::$assign_trait<TaggedRef<'a, T>> for Tagged<T>
        where
            T: std::ops::$trait<Output = T> + Copy,
        {
            fn $assign_method(&mut self, rhs: TaggedRef<'a, T>) {
                let merged = ATI_ANALYSIS
                    .lock()
                    .unwrap()
                    .union_and_get_id(&self.0, rhs.0);
                *self = Tagged(merged, self.1 $op *rhs.1);
            }
        }
    };
}

impl_tagged_arithmetic_op!(Add, add, AddAssign, add_assign, +);
impl_tagged_arithmetic_op!(Sub, sub, SubAssign, sub_assign, -);
impl_tagged_arithmetic_op!(Mul, mul, MulAssign, mul_assign, *);
impl_tagged_arithmetic_op!(Div, div, DivAssign, div_assign, /);
impl_tagged_arithmetic_op!(Rem, rem, RemAssign, rem_assign, %);
impl_tagged_arithmetic_op!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
impl_tagged_arithmetic_op!(BitOr,  bitor,  BitOrAssign,  bitor_assign, |);
impl_tagged_arithmetic_op!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

// =====================    SHIFT OPS    ===================

/// Shift operators (`<<`, `>>`) and their assigning counterparts. Unlike the arithmetic
/// operators, the result id is freshly allocated and merged with only the lhs id, since the
/// rhs (the shift count) is treated as a count rather than a value-level interaction.
macro_rules! impl_tagged_shift_op {
    (
        $trait:ident,
        $method:ident,
        $assign_trait:ident,
        $assign_method:ident,
        $op:tt
    ) => {
        impl<T> std::ops::$trait for Tagged<T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Self) -> Self::Output {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, &self.0);
                Tagged(new_id, self.1 $op rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait<TaggedRef<'a, T>> for Tagged<T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: TaggedRef<'a, T>) -> Self::Output {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, &self.0);
                Tagged(new_id, self.1 $op *rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait for TaggedRef<'a, T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Self) -> Self::Output {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, self.0);
                Tagged(new_id, *self.1 $op *rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$trait<Tagged<T>> for TaggedRef<'a, T>
        where
            T: std::ops::$trait<Output = T>,
        {
            type Output = Tagged<T>;
            fn $method(self, rhs: Tagged<T>) -> Self::Output {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, self.0);
                Tagged(new_id, *self.1 $op rhs.1)
            }
        }

        impl<T> std::ops::$assign_trait for Tagged<T>
        where
            T: std::ops::$trait<Output = T> + Copy,
        {
            fn $assign_method(&mut self, rhs: Self) {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, &self.0);
                *self = Tagged(new_id, self.1 $op rhs.1)
            }
        }

        impl<'a, T: Copy> std::ops::$assign_trait<TaggedRef<'a, T>> for Tagged<T>
        where
            T: std::ops::$trait<Output = T> + Copy,
        {
            fn $assign_method(&mut self, rhs: TaggedRef<'a, T>) {
                let new_id = ATI_ANALYSIS.lock().unwrap().make_id();
                ATI_ANALYSIS.lock().unwrap().union_and_get_id(&new_id, &self.0);
                *self = Tagged(new_id, self.1 $op *rhs.1)
            }
        }
    };
}

impl_tagged_shift_op!(Shl, shl, ShlAssign, shl_assign, <<);
impl_tagged_shift_op!(Shr, shr, ShrAssign, shr_assign, >>);

// =====================    UNARY OPS    ===================
// These operators push down to act on the underlying value while preserving the id.

/// Negation, pushed through the wrapper to act on the inner value while keeping the id intact.
impl<T> std::ops::Neg for Tagged<T>
where
    T: std::ops::Neg<Output = T>,
{
    type Output = Tagged<T>;

    fn neg(self) -> Self::Output {
        Tagged(self.0, -self.1)
    }
}

/// Logical and bitwise complement, pushed through the wrapper to act on the inner value while
/// keeping the id intact.
impl<T> std::ops::Not for Tagged<T>
where
    T: std::ops::Not<Output = T>,
{
    type Output = Tagged<T>;
    fn not(self) -> Self::Output {
        Tagged(self.0, !self.1)
    }
}

/// Used for deref coercion, which allows a `&Tagged<T>` to be automatically
/// coerced to `&T`, in turn allowing methods defined on `T` to be dispatched on a `Tagged<T>`
/// receiver.
impl<T> std::ops::Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}
