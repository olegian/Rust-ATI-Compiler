//! Array and slice support for the runtime library.
//!
//! Pass 2 rewrites every owned array `[T; N]` to a tagged array [TaggedArray] and every
//! `&[T]` / `&mut [T]` to a [TaggedRef] /
//! [TaggedRefMut]. The wrapper id stored alongside the
//! collection plays the role of the "length id". Indexing operations union it with the index
//! id so that out-of-bounds checks count as interactions, and slicing carries the wrapper id
//! into the resulting borrow.
//!
//! This file collects every array and slice helper, including the [TaggedArray] type alias,
//! length and indexing operators, the [TaggedSliceIndex] trait used to lower `arr[range]`
//! through it, the inherent `.subslice()` and `.subslice_mut()` methods, and the
//! [SiteBind] implementations for every array and slice
//! shape.

use crate::ati::ati::{ATI_ANALYSIS, Site};
use crate::ati::refs::{TaggedRef, TaggedRefMut};
use crate::ati::site_binds::SiteBind;
use crate::ati::tagged::{TagTuple, Tagged};

// =================== TYPE ALIAS ===================

/// Tagged array, the wrapped form of `[T; N]`. Stores the per-collection id alongside the
/// data itself. Pass 2 emits this as the result of every array literal.
pub type TaggedArray<T, const N: usize> = Tagged<[T; N]>;

// =================== LEN ===================

impl<T, const N: usize> TaggedArray<T, N> {
    /// Length of the array as a tagged `usize`. Reuses the wrapper id, so the returned length
    /// carries the same id as the array.
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, N)
    }
}
impl<'a, T> TaggedRef<'a, [T]> {
    /// Length of the borrowed slice as a tagged `usize`, carrying the slice's id.
    pub fn len(&self) -> Tagged<usize> {
        Tagged(*self.0, self.1.len())
    }
}
impl<'a, T> TaggedRefMut<'a, [T]> {
    /// Length of the borrowed mutable slice as a tagged `usize`, carrying the slice's id.
    pub fn len(&self) -> Tagged<usize> {
        Tagged(*self.0, self.1.len())
    }
}

// =================== SITE BIND ===================

/// Binding an array associates the length id and every element id with the site. References
/// to arrays delegate the same way, walking each element via the per-element impls.
impl<T, const N: usize> SiteBind for TaggedArray<T, N> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}.length"), self.len().0);
        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}
impl<'a, T, const N: usize> SiteBind for TaggedRef<'a, [T; N]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}.length"), *self.0);
        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}
impl<'a, T, const N: usize> SiteBind for TaggedRefMut<'a, [T; N]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}.length"), *self.0);
        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

/// Slices are represented as `TaggedRef<'_, [T]>` / `TaggedRefMut<'_, [T]>`. The source-level
/// `&[T]` / `&mut [T]` is absorbed into the wrapper via unsized coercion from
/// `TaggedRef<'_, [T; N]>`. Each element is recursively bound.
impl<'a, T> SiteBind for TaggedRef<'a, [T]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}.length"), self.len().0);
        for i in 0..self.1.len() {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}
impl<'a, T> SiteBind for TaggedRefMut<'a, [T]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}.length"), self.len().0);
        for i in 0..self.1.len() {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

// =================== REGULAR INDEXING ===================
// [T; N]
impl<Idx, T, const N: usize> std::ops::Index<Tagged<Idx>> for TaggedArray<T, N>
where
    [T; N]: std::ops::Index<Idx, Output = T>,
{
    type Output = T;

    fn index(&self, index: Tagged<Idx>) -> &Self::Output {
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(&self.0, &index.0);
        &self.1[index.1]
    }
}
impl<Idx, T, const N: usize> std::ops::IndexMut<Tagged<Idx>> for TaggedArray<T, N>
where
    [T; N]: std::ops::IndexMut<Idx, Output = T>,
{
    fn index_mut(&mut self, index: Tagged<Idx>) -> &mut Self::Output {
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(&self.0, &index.0);
        &mut self.1[index.1]
    }
}

// TaggedRef<[T]>
impl<'slice, Idx, T> std::ops::Index<Tagged<Idx>> for TaggedRef<'slice, [T]>
where
    [T]: std::ops::Index<Idx, Output = T>,
{
    type Output = T;

    fn index(&self, index: Tagged<Idx>) -> &Self::Output {
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(self.0, &index.0);
        &self.1[index.1]
    }
}

// TaggedRefMut<[T]>
impl<'slice, Idx, T> std::ops::Index<Tagged<Idx>> for TaggedRefMut<'slice, [T]>
where
    [T]: std::ops::Index<Idx, Output = T>,
{
    type Output = T;

    fn index(&self, index: Tagged<Idx>) -> &Self::Output {
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(self.0, &index.0);
        &self.1[index.1]
    }
}
impl<'slice, Idx, T> std::ops::IndexMut<Tagged<Idx>> for TaggedRefMut<'slice, [T]>
where
    [T]: std::ops::IndexMut<Idx, Output = T>,
{
    fn index_mut(&mut self, index: Tagged<Idx>) -> &mut Self::Output {
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(self.0, &index.0);
        &mut self.1[index.1]
    }
}

// =================== SLICE INDEXING ===================

/// Trait implemented by every tagged range type, allowing it to act as a slice index. When
/// pass 2 emits `arr[range]`, the index operation calls [TagTuple::id] to grab the range's
/// wrapper id (so it can be unioned with the collection id) and [TaggedSliceIndex::into_raw]
/// to convert the tagged range into the underlying standard library range that std's
/// [SliceIndex](std::slice::SliceIndex) machinery accepts.
pub trait TaggedSliceIndex<Idx>: TagTuple {
    /// The raw, untagged range type that this tagged range converts into.
    type Raw: std::slice::SliceIndex<[Idx], Output = [Idx]>;
    /// Consumes the tagged range and returns the equivalent untagged range, which is forwarded
    /// to the underlying [SliceIndex](std::slice::SliceIndex) machinery.
    fn into_raw(self) -> Self::Raw;
}

// Inherent `.subslice(range)` / `.subslice_mut(range)` helpers emitted by pass 2 for
// `&recv[range]` / `&mut recv[range]`. The single Tagged*Ref impls cover both `[T]` and
// `[T; N]` inner shapes via `Index`/`IndexMut` because std implements `Index<I>` for both
// when `I: SliceIndex<[T]>`. Owned containers (TaggedArray) borrow from the local through
// `&self` and stay separate.
impl<T, const N: usize> TaggedArray<T, N> {
    /// Builds a [TaggedRef] over a sub-slice of this array, given a tagged range. Unions the
    /// array id with the range id before forming the borrow, so the returned slice carries
    /// the merged length identity.
    pub fn subslice<R>(&self, range: R) -> TaggedRef<'_, [T]>
    where
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(&self.0, &range_id);
        TaggedRef(&self.0, &self.1[range.into_raw()])
    }

    /// Mutable variant of [TaggedArray::subslice]. Builds a [TaggedRefMut] over a sub-slice
    /// of this array.
    pub fn subslice_mut<R>(&mut self, range: R) -> TaggedRefMut<'_, [T]>
    where
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(&self.0, &range_id);
        TaggedRefMut(&mut self.0, &mut self.1[range.into_raw()])
    }
}

impl<'a, S: ?Sized> TaggedRef<'a, S> {
    /// Builds a [TaggedRef] over a sub-slice of this borrow, given a tagged range. Unions the
    /// borrow id with the range id before projecting through the inner slice.
    pub fn subslice<T, R>(self, range: R) -> TaggedRef<'a, [T]>
    where
        S: std::ops::Index<R::Raw, Output = [T]>,
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(self.0, &range_id);
        self.map(|s| &s[range.into_raw()])
    }
}

impl<'a, S: ?Sized> TaggedRefMut<'a, S> {
    /// Mutable variant of [TaggedRef::subslice]. Builds a [TaggedRefMut] over a sub-slice of
    /// this borrow.
    pub fn subslice_mut<T, R>(self, range: R) -> TaggedRefMut<'a, [T]>
    where
        S: std::ops::IndexMut<R::Raw, Output = [T]>,
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        ATI_ANALYSIS
            .lock()
            .unwrap()
            .union_and_get_id(self.0, &range_id);
        self.map(|s| &mut s[range.into_raw()])
    }
}
