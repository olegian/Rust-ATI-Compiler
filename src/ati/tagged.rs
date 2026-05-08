/* This file is a part of the runtime library injected into the compiled project.
 * It defines the Tagged<T> type which ultimately represents a tuple (Id, T). All
 * tracked values are transformed into this tagged type to be able to uniquely
 * identify where they are used. Id's are used within the various union find structure
 * (defined in ati.rs), to track interactions between values, and represent abstract
 * type sets.
*/

use crate::ati::ati::ATI_ANALYSIS;

/// type alias for Ids for ease of use, and to be able to quickly swap this out
/// (although I doubt we'll need to).
pub type Id = u64;

/// Generates incrementing tags of type `Id`, with each call to `tag()`
#[derive(Debug)]
pub struct Tagger {
    next_id: Id,
}

impl Tagger {
    /// Creates a new Tagger
    pub fn new() -> Self {
        Tagger { next_id: 0 }
    }

    /// Fetches the next tag
    pub fn tag(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        id
    }
}

/// A tuple of a type T, alongside a unique `Id`.
/// This isn't expected to be created directly, but is instead
/// used as a return type from `ATI::track`.
#[derive(Debug, Clone, Copy)]
pub struct Tagged<T: ?Sized>(pub Id, pub T);

/// A shared "view" over a tagged value: one borrow of the Id, one borrow of
/// the inner value. Produced by instrumenting `&x` when `x` is (directly or
/// transitively) a `Tagged<..>`. The `T: ?Sized` bound lets `TaggedRef<'a, [T]>`
/// serve as the slice representation, with unsized coercion from
/// `TaggedRef<'a, [T; N]>` via `CoerceUnsized`.
///
/// Dereferences to `T`, which enables calling any `&self` method on `T` via
/// auto-deref. Methods that would otherwise be hung off `Tagged<T>` (e.g. the
/// slice `.len()`) are declared directly on `TaggedRef` / `TaggedRefMut` since
/// there is no single `Tagged<T>` value in memory to deref through.
pub struct TaggedRef<'a, T: ?Sized>(pub &'a Id, pub &'a T);
pub struct TaggedRefMut<'a, T: ?Sized>(pub &'a mut Id, pub &'a mut T);

// Used to go from a Tagged<T> = (Id, T) -> TaggedRef(Mut?)<T> = (&(mut?) Id, &(mut?) T)
impl<T> Tagged<T> {
    fn as_tagged_ref(&self) -> TaggedRef<'_, T> {
        TaggedRef(&self.0, &self.1)
    }
    fn as_tagged_ref_mut(&mut self) -> TaggedRefMut<'_, T> {
        TaggedRefMut(&mut self.0, &mut self.1)
    }
}

impl<'a, T> TaggedRefMut<'a, T> {
    // Write both the id and value through the mutable borrow. Assignment
    // is not an interaction, the destination's prior id is overwritten,
    // not unioned with the RHS id. Raw `*refmut = v.1` via DerefMut would leak
    // the id of the LHS slot unchanged; this helper is what pass 2 emits at
    // assignments through a TaggedRefMut instead.
    pub fn assign(&mut self, v: Tagged<T>) {
        *self.0 = v.0;
        *self.1 = v.1;
    }
}

// Projections that preserve the Id while carving a sub-reference out of the
// inner value (slice indexing, field access, etc.). Both `map` variants
// consume `self`, so the closure receives the full lifetime `'a`.
impl<'a, T: ?Sized> TaggedRef<'a, T> {
    pub fn map<U: ?Sized>(self, f: impl FnOnce(&'a T) -> &'a U) -> TaggedRef<'a, U> {
        TaggedRef(self.0, f(self.1))
    }
}

impl<'a, T: ?Sized> TaggedRefMut<'a, T> {
    pub fn map<U: ?Sized>(
        self,
        f: impl FnOnce(&'a mut T) -> &'a mut U,
    ) -> TaggedRefMut<'a, U> {
        TaggedRefMut(self.0, f(self.1))
    }

    /// Manual analogue of Rust's implicit `&mut` reborrow. `TaggedRefMut` is
    /// move-only (must not be Copy/Clone to preserve unique-borrow
    /// semantics), so every value-position use of a `TaggedRefMut` binding
    /// other than the last needs `.reborrow()` where the source code would
    /// have implicitly reborrowed.
    pub fn reborrow(&mut self) -> TaggedRefMut<'_, T> {
        TaggedRefMut(self.0, &mut *self.1)
    }
}

impl<'a, T: ?Sized> std::ops::Deref for TaggedRef<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.1
    }
}
impl<'a, T: ?Sized> std::ops::Deref for TaggedRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.1
    }
}
impl<'a, T: ?Sized> std::ops::DerefMut for TaggedRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.1
    }
}

// Copy/Clone for TaggedRef only. TaggedRefMut holds unique borrows and must not
// be Copy/Clone to avoid aliasing the mutable refs
impl<'a, T: ?Sized> Clone for TaggedRef<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: ?Sized> Copy for TaggedRef<'a, T> {}

/// Debug implementations for printing `TaggedRef<T>`s.
impl<'a, T: ?Sized + std::fmt::Debug> std::fmt::Debug for TaggedRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("TaggedRef")
            .field(self.0)
            .field(&self.1)
            .finish()
    }
}
impl<'a, T: ?Sized + std::fmt::Debug> std::fmt::Debug for TaggedRefMut<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("TaggedRefMut")
            .field(self.0)
            .field(&self.1)
            .finish()
    }
}

impl<'a, T: ?Sized + std::fmt::Display> std::fmt::Display for TaggedRef<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}
impl<'a, T: ?Sized + std::fmt::Display> std::fmt::Display for TaggedRefMut<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

// Automatic Unsized coercion: `TaggedRef<[T; N]>` -> `TaggedRef<[T]>` (and same for Mut).
// `U: ?Sized` because the *target* of the coercion is the unsized form (e.g. `[T]`).
impl<'a, T: std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<TaggedRef<'a, U>>
    for TaggedRef<'a, T>
{
}

impl<'a, T: std::marker::Unsize<U>, U: ?Sized> std::ops::CoerceUnsized<TaggedRefMut<'a, U>>
    for TaggedRefMut<'a, T>
{
}

// The following type aliases describe common special-cased structures built up using
// the above Tagged type
pub type TaggedArray<T, const N: usize> = Tagged<[T; N]>;
pub type TaggedRange<T> = Tagged<std::ops::Range<Tagged<T>>>;
pub type TaggedRangeInclusive<T> = Tagged<std::ops::RangeInclusive<Tagged<T>>>;
pub type TaggedRangeFrom<T> = Tagged<std::ops::RangeFrom<Tagged<T>>>;
pub type TaggedRangeTo<T> = Tagged<std::ops::RangeTo<Tagged<T>>>;
pub type TaggedRangeToInclusive<T> = Tagged<std::ops::RangeToInclusive<Tagged<T>>>;
pub type TaggedRangeFull = Tagged<std::ops::RangeFull>;

//////////////////// MISC TRAIT IMPLEMENTATIONS ////////////////////////////
/// Below is a lot of trait implementations that may or may not be necessary.
/// For now, they are helpful, and used as a proof of concept. These traits
/// allow for ranges to work as iterators, supporting all iterator methods.

/// Getting the length (from types that have a length), should return a
/// `Tagged<usize>`, with the Id of the outermost tag.
impl<T, const N: usize> TaggedArray<T, N> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, N)
    }
}
impl<'a, T> TaggedRef<'a, [T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(*self.0, self.1.len())
    }
}
impl<'a, T> TaggedRefMut<'a, [T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(*self.0, self.1.len())
    }
}

impl<T> TaggedRange<T>
where
    T: Copy + std::ops::Sub<Output = T>,
    usize: std::convert::TryFrom<T>,
{
    pub fn len(&self) -> Tagged<usize> {
        let diff = self.1.end.1 - self.1.start.1;
        let n = <usize as std::convert::TryFrom<T>>::try_from(diff)
            .ok()
            .unwrap_or(0);
        Tagged(self.0, n)
    }
}

impl<T> TaggedRangeInclusive<T>
where
    T: Copy + std::ops::Sub<Output = T>,
    usize: std::convert::TryFrom<T>,
{
    pub fn len(&self) -> Tagged<usize> {
        let diff = self.1.end().1 - self.1.start().1;
        let n = match <usize as std::convert::TryFrom<T>>::try_from(diff) {
            Ok(d) => d.saturating_add(1),
            Err(_) => 0,
        };
        Tagged(self.0, n)
    }
}

/// Iterator impls for tagged ranges. Rather than reimplementing every
/// Iterator adapter (.map, .filter, .sum, ...) we impl `Iterator` once on
/// the Tagged range itself; all ~70 default methods inherit for free.
/// Each yielded element carries the range's wrapper id so that values
/// produced by iteration participate in the range's AT. `for` loops keep
/// working via the blanket `impl<I: Iterator> IntoIterator for I`.
impl<T: Copy + std::iter::Step> Iterator for Tagged<std::ops::Range<Tagged<T>>> {
    type Item = Tagged<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.1.start.1 >= self.1.end.1 {
            return None;
        }
        let yielded = self.1.start.1;
        self.1.start.1 = <T as std::iter::Step>::forward(yielded, 1);
        Some(Tagged(self.0, yielded))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = <T as std::iter::Step>::steps_between(&self.1.start.1, &self.1.end.1);
        (n.0, n.1)
    }
}

impl<T: Copy + std::iter::Step> DoubleEndedIterator for Tagged<std::ops::Range<Tagged<T>>> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.1.start.1 >= self.1.end.1 {
            return None;
        }
        self.1.end.1 = <T as std::iter::Step>::backward(self.1.end.1, 1);
        Some(Tagged(self.0, self.1.end.1))
    }
}

impl<T: Copy + std::iter::Step> ExactSizeIterator for Tagged<std::ops::Range<Tagged<T>>> where
    std::ops::Range<T>: ExactSizeIterator
{
}

impl<T: Copy + std::iter::Step> std::iter::FusedIterator for Tagged<std::ops::Range<Tagged<T>>> {}

/// RangeInclusive has a hidden `exhausted` flag we can't reach, so we
/// encode exhaustion by leaving `start > end` when we yield the last
/// value - `start == T::MAX` is the only case where this can't hold and
/// would double-yield; acceptable edge case for our instrumentation.
impl<T: Copy + std::iter::Step> Iterator for Tagged<std::ops::RangeInclusive<Tagged<T>>> {
    type Item = Tagged<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.1.start().1;
        let end = self.1.end().1;
        if start > end {
            return None;
        }
        let start_id = self.1.start().0;
        let end_id = self.1.end().0;
        let next_start = match <T as std::iter::Step>::forward_checked(start, 1) {
            Some(s) => s,
            None => start, // T::MAX: fall back to start == end (terminal but could double-yield)
        };
        self.1 = Tagged(start_id, next_start)..=Tagged(end_id, end);
        Some(Tagged(self.0, start))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.1.start().1 > self.1.end().1 {
            return (0, Some(0));
        }
        let n = <T as std::iter::Step>::steps_between(&self.1.start().1, &self.1.end().1);
        (n.0.saturating_add(1), n.1.and_then(|v| v.checked_add(1)))
    }
}

impl<T: Copy + std::iter::Step> DoubleEndedIterator
    for Tagged<std::ops::RangeInclusive<Tagged<T>>>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let start = self.1.start().1;
        let end = self.1.end().1;
        if start > end {
            return None;
        }
        let start_id = self.1.start().0;
        let end_id = self.1.end().0;
        let next_end = match <T as std::iter::Step>::backward_checked(end, 1) {
            Some(e) => e,
            None => end,
        };
        self.1 = Tagged(start_id, start)..=Tagged(end_id, next_end);
        Some(Tagged(self.0, end))
    }
}

impl<T: Copy + std::iter::Step> ExactSizeIterator for Tagged<std::ops::RangeInclusive<Tagged<T>>> where
    std::ops::RangeInclusive<T>: ExactSizeIterator
{
}

impl<T: Copy + std::iter::Step> std::iter::FusedIterator
    for Tagged<std::ops::RangeInclusive<Tagged<T>>>
{
}

impl<T: Copy + std::iter::Step> Iterator for Tagged<std::ops::RangeFrom<Tagged<T>>> {
    type Item = Tagged<T>;
    fn next(&mut self) -> Option<Self::Item> {
        let yielded = self.1.start.1;
        self.1.start.1 = <T as std::iter::Step>::forward(yielded, 1);
        Some(Tagged(self.0, yielded))
    }
}

impl<T: Copy + std::iter::Step> std::iter::FusedIterator
    for Tagged<std::ops::RangeFrom<Tagged<T>>>
{
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::Range<Tagged<T>>> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Included(&self.1.start.1)
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Excluded(&self.1.end.1)
    }
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::RangeInclusive<Tagged<T>>> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Included(&self.1.start().1)
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Included(&self.1.end().1)
    }
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::RangeFrom<Tagged<T>>> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Included(&self.1.start.1)
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Unbounded
    }
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::RangeTo<Tagged<T>>> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Unbounded
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Excluded(&self.1.end.1)
    }
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::RangeToInclusive<Tagged<T>>> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Unbounded
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Included(&self.1.end.1)
    }
}

impl<T> std::ops::RangeBounds<T> for Tagged<std::ops::RangeFull> {
    fn start_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Unbounded
    }
    fn end_bound(&self) -> std::ops::Bound<&T> {
        std::ops::Bound::Unbounded
    }
}

/// helpful for debugging purposes, allowing printing of tagged values.
impl<T> std::fmt::Display for Tagged<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

impl<T> std::iter::Sum for Tagged<T>
where
    Tagged<T>: std::ops::Add<Output = Tagged<T>>,
    T: std::iter::Sum,
{
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a + b).unwrap_or_else(|| {
            let id = ATI_ANALYSIS.lock().unwrap().make_id();
            Tagged(id, T::sum(std::iter::empty::<T>()))
        })
    }
}

impl<'a, T: Copy + 'a> std::iter::Sum<&'a Tagged<T>> for Tagged<T>
where
    Tagged<T>: std::ops::Add<Output = Tagged<T>>,
    T: std::iter::Sum,
{
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

impl<T> std::iter::Product for Tagged<T>
where
    Tagged<T>: std::ops::Mul<Output = Tagged<T>>,
    T: std::iter::Product,
{
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.reduce(|a, b| a * b).unwrap_or_else(|| {
            let id = ATI_ANALYSIS.lock().unwrap().make_id();
            Tagged(id, T::product(std::iter::empty::<T>()))
        })
    }
}
