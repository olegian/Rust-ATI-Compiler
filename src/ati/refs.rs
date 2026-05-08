//! Defines the borrow-form tagged wrappers, [TaggedRef] and [TaggedRefMut].
//!
//! Pass 2 rewrites every reference to a tracked value into one of these wrappers, instead of
//! using a regular `&Tagged<T>` or `&mut Tagged<T>`. The wrappers split the borrow into one
//! reference to the [Id] and one reference to the inner value, which
//! lets a slice-shaped reference live as `TaggedRef<'_, [T]>` even when the storage of the
//! id and the values is not contiguous in memory.
//!
//! [TaggedRef] is shared and `Copy`. [TaggedRefMut] is unique and must not be `Copy` or
//! `Clone`, so pass 2 emits explicit [Reborrow::reborrow] calls anywhere the source code
//! would have relied on the compiler's implicit `&mut` reborrow.

use crate::ati::tagged::{Id, TagTuple, Tagged};

/// A shared "view" over a tagged value, consisting of one borrow of the id and one borrow of
/// the inner value.
///
/// Produced by instrumenting `&x` when `x` is a [Tagged]. The `T: ?Sized` bound lets
/// `TaggedRef<'a, [T]>` serve as the slice representation, with unsized coercion from
/// `TaggedRef<'a, [T; N]>` via [CoerceUnsized](std::ops::CoerceUnsized).
///
/// Dereferences to `T`, which enables calling any `&self` method on `T` via auto-deref.
/// Methods that would otherwise be hung off [Tagged] (e.g. the slice `.len()`) are declared
/// directly on [TaggedRef] and [TaggedRefMut], since there is no single [Tagged] value in
/// memory to deref through.
pub struct TaggedRef<'a, T: ?Sized>(pub &'a Id, pub &'a T);

/// Unique-borrow variant of [TaggedRef]. Holds one mutable borrow of the id and one mutable
/// borrow of the inner value.
///
/// Move-only by construction. Pass 2 inserts an explicit [Reborrow::reborrow] call
/// anywhere the source code would have relied on Rust's implicit `&mut` reborrow.
pub struct TaggedRefMut<'a, T: ?Sized>(pub &'a mut Id, pub &'a mut T);

impl<T> Tagged<T> {
    /// Borrows `self` as a [TaggedRef]. Used by pass 2 to lower `&x` of a [Tagged] expression.
    fn as_tagged_ref(&self) -> TaggedRef<'_, T> {
        TaggedRef(&self.0, &self.1)
    }

    /// Borrows `self` as a [TaggedRefMut]. Used by pass 2 to lower `&mut x` of a [Tagged]
    /// expression.
    fn as_tagged_ref_mut(&mut self) -> TaggedRefMut<'_, T> {
        TaggedRefMut(&mut self.0, &mut self.1)
    }
}

impl<'a, T: ?Sized> TagTuple for TaggedRef<'a, T> {
    type Inner = T;
    fn id(&self) -> Id {
        *self.0
    }
    fn value(&self) -> &T {
        self.1
    }
}

impl<'a, T: ?Sized> TagTuple for TaggedRefMut<'a, T> {
    type Inner = T;
    fn id(&self) -> Id {
        *self.0
    }
    fn value(&self) -> &T {
        &*self.1
    }
}

impl<'a, T> TaggedRefMut<'a, T> {
    /// Writes both the id and value through the mutable borrow. Assignment is not an
    /// interaction, so the destination's prior id is overwritten, not unioned with the RHS id.
    /// Raw `*refmut = v.1` via [DerefMut](std::ops::DerefMut) would leak the id of the LHS
    /// slot unchanged. This helper is what pass 2 emits for assignments through a
    /// [TaggedRefMut] instead.
    pub fn assign(&mut self, v: Tagged<T>) {
        *self.0 = v.0;
        *self.1 = v.1;
    }
}

impl<'a, T: ?Sized> TaggedRef<'a, T> {
    /// Pushes an operation into the underlyling `T`.
    pub fn map<U: ?Sized>(self, f: impl FnOnce(&'a T) -> &'a U) -> TaggedRef<'a, U> {
        TaggedRef(self.0, f(self.1))
    }
}

impl<'a, T: ?Sized> TaggedRefMut<'a, T> {
    /// Mutable variant of [TaggedRef::map]. Same id-preserving projection, but routes through
    /// `&mut` borrows.
    pub fn map<U: ?Sized>(
        self,
        f: impl FnOnce(&'a mut T) -> &'a mut U,
    ) -> TaggedRefMut<'a, U> {
        TaggedRefMut(self.0, f(self.1))
    }
}

/// This trait allows for explicit reborrowing of mutable references.
/// 
/// This manual analogue of Rust's implicit `&mut` reborrow, allows [TaggedRefMut] to remain
/// move-only (as it must not be `Copy` or `Clone`, to preserve unique-borrow semantics). 
/// Every value-position use of a [TaggedRefMut] binding other than the last needs an 
/// explicit `.reborrow()` call where the source code would have implicitly reborrowed a `&mut T`.
/// 
/// Defined as a trait so that it can be implemented on &mut Tagged<T>. This means
/// if a mutable reference to a Tagged<T> is ever constructed, we can simply reborrow
/// it to construct the correctly transformed TaggedRefMut form. This is especially handy when
/// a `ref mut` pattern binding is used.
trait Reborrow<'a, T: ?Sized> {
    fn reborrow(&'a mut self) -> TaggedRefMut<'a, T>;
}

impl<'a, T> Reborrow<'a, T> for Tagged<T> {
    fn reborrow(&'a mut self) -> TaggedRefMut<'a, T> {
        self.as_tagged_ref_mut()
    }
}

impl<'a, T: ?Sized> Reborrow<'a, T> for TaggedRefMut<'a, T> {
    fn reborrow(&'a mut self) -> TaggedRefMut<'a, T> {
        TaggedRefMut(self.0, &mut *self.1)
    }
}

impl<'a, T: ?Sized> std::ops::Deref for TaggedRef<'a, T> {
    type Target = T;

    /// Drops the id and yields the inner value reference, enabling auto-deref to any
    /// `&self`-method on `T`.
    fn deref(&self) -> &T {
        self.1
    }
}
impl<'a, T: ?Sized> std::ops::Deref for TaggedRefMut<'a, T> {
    type Target = T;

    /// Drops the id and yields the inner value reference, enabling auto-deref to any
    /// `&self`-method on `T`.
    fn deref(&self) -> &T {
        self.1
    }
}
impl<'a, T: ?Sized> std::ops::DerefMut for TaggedRefMut<'a, T> {
    /// Drops the id and yields the inner value mutable reference, enabling auto-deref to any
    /// `&mut self`-method on `T`.
    fn deref_mut(&mut self) -> &mut T {
        self.1
    }
}

// Copy/Clone for TaggedRef only. TaggedRefMut holds unique borrows and must not be
// Copy/Clone, to avoid aliasing the mutable refs.
impl<'a, T: ?Sized> Clone for TaggedRef<'a, T> {
    /// `Clone` for [TaggedRef] is a `Copy`, both contained borrows are shared.
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized> Copy for TaggedRef<'a, T> {}

/// `Debug` formatting for [TaggedRef], emitted as `TaggedRef(id, value)`.
impl<'a, T: ?Sized + std::fmt::Debug> std::fmt::Debug for TaggedRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("TaggedRef")
            .field(self.0)
            .field(&self.1)
            .finish()
    }
}
/// `Debug` formatting for [TaggedRefMut], emitted as `TaggedRefMut(id, value)`.
impl<'a, T: ?Sized + std::fmt::Debug> std::fmt::Debug for TaggedRefMut<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("TaggedRefMut")
            .field(self.0)
            .field(&self.1)
            .finish()
    }
}

/// `Display` formatting for [TaggedRef], emitted as `(id, value)`.
impl<'a, T: ?Sized + std::fmt::Display> std::fmt::Display for TaggedRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}
/// `Display` formatting for [TaggedRefMut], emitted as `(id, value)`.
impl<'a, T: ?Sized + std::fmt::Display> std::fmt::Display for TaggedRefMut<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

// Automatic Unsized coercion from `TaggedRef<[T; N]>` to `TaggedRef<[T]>`, same for Mut. 
// `U: ?Sized` because the *target* of the coercion is the unsized form (e.g. `[T]`).
impl<'a, T: std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<TaggedRef<'a, U>>
    for TaggedRef<'a, T>
{
}

impl<'a, T: std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<TaggedRefMut<'a, U>>
    for TaggedRefMut<'a, T>
{
}
