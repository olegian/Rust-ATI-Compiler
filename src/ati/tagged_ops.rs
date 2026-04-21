use crate::ati::{
    ati::ATI_ANALYSIS,
    tagged::{Id, Tagged, TaggedRef, TaggedRefMut},
};

// =====================    COMPARISON OPS / MARKERS    ===================
/// Every comparison across any pair of {Tagged, TaggedRef, TaggedRefMut}
/// unions the two tags in the value union-find.
/// This trait lets the macros below avoid matching on each tagged
/// form separately
trait TaggedCmpable<T: ?Sized> {
    fn tagged_id(&self) -> &Id;
    fn tagged_value(&self) -> &T;
}

impl<T: ?Sized> TaggedCmpable<T> for Tagged<T> {
    fn tagged_id(&self) -> &Id { &self.0 }
    fn tagged_value(&self) -> &T { &self.1 }
}

impl<'a, T: ?Sized> TaggedCmpable<T> for TaggedRef<'a, T> {
    fn tagged_id(&self) -> &Id { self.0 }
    fn tagged_value(&self) -> &T { self.1 }
}

impl<'a, T: ?Sized> TaggedCmpable<T> for TaggedRefMut<'a, T> {
    fn tagged_id(&self) -> &Id { &*self.0 }
    fn tagged_value(&self) -> &T { &*self.1 }
}

// PartialEq + PartialOrd between an Lhs and Rhs (both of which must impl
// TaggedCmpable<T>). Unions the tags on every call before delegating
// comparison to the underlying T.
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
                    .union_and_get_id(self.tagged_id(), other.tagged_id());
                self.tagged_value().eq(other.tagged_value())
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
                    .union_and_get_id(self.tagged_id(), other.tagged_id());
                self.tagged_value().partial_cmp(other.tagged_value())
            }
        }
    };
}

// Eq + Ord for a self-type comparison. Eq/Ord require the same type on both
// sides, so only the three self-self cases are valid.
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
                    .union_and_get_id(self.tagged_id(), other.tagged_id());
                self.tagged_value().cmp(other.tagged_value())
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


// =====================    ARITHEMATIC OPS    ===================
// all of these operators merge together the tags of the result, lhs, and rhs.
macro_rules! impl_tagged_arithematic_op {
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

impl_tagged_arithematic_op!(Add, add, AddAssign, add_assign, +);
impl_tagged_arithematic_op!(Sub, sub, SubAssign, sub_assign, -);
impl_tagged_arithematic_op!(Mul, mul, MulAssign, mul_assign, *);
impl_tagged_arithematic_op!(Div, div, DivAssign, div_assign, /);
impl_tagged_arithematic_op!(Rem, rem, RemAssign, rem_assign, %);
impl_tagged_arithematic_op!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
impl_tagged_arithematic_op!(BitOr,  bitor,  BitOrAssign,  bitor_assign, |);
impl_tagged_arithematic_op!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

// =====================    SHIFT OPS    ===================
// these operators merge together the tags of the lhs and the result, but not
// the rhs.
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
// these operators just get pushed down to act on the underlying value.
impl<T> std::ops::Neg for Tagged<T>
where
    T: std::ops::Neg<Output = T>,
{
    type Output = Tagged<T>;

    fn neg(self) -> Self::Output {
        Tagged(self.0, -self.1)
    }
}

impl<T> std::ops::Not for Tagged<T>
where
    T: std::ops::Not<Output = T>,
{
    type Output = Tagged<T>;
    fn not(self) -> Self::Output {
        Tagged(self.0, !self.1)
    }
}

// this is a really important impl! This gets used for deref coercion,
// which allows for a &Tagged<T> to automatically be coereced to &T,
// which allows for dispatching methods that are defined on T using a 
// Tagged<T>.
impl<T> std::ops::Deref for Tagged<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

