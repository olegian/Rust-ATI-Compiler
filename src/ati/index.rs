/// Provides implementations of standard libary traits that allow for indexing into 
/// Tagged arrays and slices.
/// 
/// Indexing into an array or slice merges together the index Id with the collections length Id,
/// as the out of bounds check is considered an interaction.
/// 
/// Further, as arrays and slices can be accessed via range indexes, and the SliceIndex trait
/// is sealed within the compiler, we are unable to directly implement it to make range indexing
/// work out of the box. Defined within this file are `.subslice` functions, which allow us to
/// rewrite
/// ```rust
/// let x = &arr[a..b];
/// ```
/// into something that looks like:
/// ```rust
/// let x = arr.subslice(a..b);
/// ```

use crate::ati::{
    ati::ATI_ANALYSIS,
    tagged::{
        Id, Tagged, TaggedArray, TaggedRange, TaggedRangeFrom, TaggedRangeFull,
        TaggedRangeInclusive, TaggedRangeTo, TaggedRangeToInclusive, TaggedRef, TaggedRefMut,
    },
};
// ==============    REGULAR INDEXING   =================
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

// ==============    SLICE INDEXING   =================

/// Implementors of this trait are tagged-ranges, used as indexes that can
/// access some collection e.g. in `array[range]`, `range`'s type must implement this trait.
/// This allows for the Index operation to utilize the into_raw method to
/// convert the tagged range into a simple range, after merging appropriate ids.
pub trait TaggedSliceIndex<Idx> {
    type Raw: std::slice::SliceIndex<[Idx], Output = [Idx]>;
    fn id(&self) -> Id;
    fn into_raw(self) -> Self::Raw;
}

impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRange<T>
where
    std::ops::Range<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::Range<T>;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        self.1.start.1..self.1.end.1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeInclusive<T>
where
    std::ops::RangeInclusive<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeInclusive<T>;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        self.1.start().1..=self.1.end().1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeFrom<T>
where
    std::ops::RangeFrom<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeFrom<T>;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        self.1.start.1..
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeTo<T>
where
    std::ops::RangeTo<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeTo<T>;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        ..self.1.end.1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeToInclusive<T>
where
    std::ops::RangeToInclusive<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeToInclusive<T>;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        ..=self.1.end.1
    }
}
impl<T> TaggedSliceIndex<T> for TaggedRangeFull {
    type Raw = std::ops::RangeFull;
    fn id(&self) -> Id {
        self.0
    }
    fn into_raw(self) -> Self::Raw {
        ..
    }
}

// Inherent `.subslice(range)` / `.subslice_mut(range)` helpers emitted by
// pass 2 for `&recv[range]` / `&mut recv[range]`. The single Tagged*Ref
// impls cover both `[T]` and `[T; N]` inner shapes via `Index`/`IndexMut`
// because std implements `Index<I>` for both when `I: SliceIndex<[T]>`.
// Owned containers (TaggedArray) borrow from the local through `&self` and
// stay separate.
impl<T, const N: usize> TaggedArray<T, N> {
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
