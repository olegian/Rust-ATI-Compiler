// Wrapper iterators for tagged slices/arrays.
//
// The standard library's slice/array iterators yield `&(mut?)Tagged<T>` /
// `Tagged<T>`, instead of `TaggedRef(Mut?)<T>` / `Tagged<T>`. The below structs
// and Iterator implementation fix that.
//
// `.enumerate()` is special cased
//
// FIXME:
// - `Sum<TaggedRef<T>>` for `Tagged<T>` and `Product` analogues are needed
//   for `.sum()` / `.product()` on these iterators
// - `FromIterator<TaggedRef<T>>` for ... is needed for `.collect()`
// - `.cloned()` / `.copied()` std variants require
//   `Iterator<Item=&T>` so don't apply to `Item=TaggedRef<T>`.
// - enumerate-after-combinators need mechanism to
//   thread length-id through `.filter()` / `.map()` / etc.

use crate::ati::tagged::{Id, Tagged, TaggedRef, TaggedRefMut};

// =================== SHIM ITERATORS ===================

/// Iterator over a tagged slice/array, yielding `TaggedRef<'a, T>` per
/// element. Created by `.iter()` on `TaggedRef<[Tagged<T>]>`,
/// `TaggedRef<[Tagged<T>; N]>`, or `&Tagged<[Tagged<T>; N]>`.
pub struct TaggedSliceIter<'a, T> {
    inner: std::slice::Iter<'a, Tagged<T>>,
    length_id: Id,
}

impl<'a, T> Iterator for TaggedSliceIter<'a, T> {
    type Item = TaggedRef<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|t| TaggedRef(&t.0, &t.1))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for TaggedSliceIter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|t| TaggedRef(&t.0, &t.1))
    }
}

impl<'a, T> ExactSizeIterator for TaggedSliceIter<'a, T> {}
impl<'a, T> std::iter::FusedIterator for TaggedSliceIter<'a, T> {}

impl<'a, T> TaggedSliceIter<'a, T> {
    /// Inherent override that shadows [`Iterator::enumerate`].
    pub fn enumerate(self) -> TaggedEnumerate<Self> {
        let length_id = self.length_id;
        TaggedEnumerate { inner: self, count: 0, length_id }
    }
}

/// Mutable variant of [`TaggedSliceIter`]. Yields `TaggedRefMut<'a, T>`.
pub struct TaggedSliceIterMut<'a, T> {
    inner: std::slice::IterMut<'a, Tagged<T>>,
    length_id: Id,
}

impl<'a, T> Iterator for TaggedSliceIterMut<'a, T> {
    type Item = TaggedRefMut<'a, T>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|t| TaggedRefMut(&mut t.0, &mut t.1))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T> DoubleEndedIterator for TaggedSliceIterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|t| TaggedRefMut(&mut t.0, &mut t.1))
    }
}

impl<'a, T> ExactSizeIterator for TaggedSliceIterMut<'a, T> {}
impl<'a, T> std::iter::FusedIterator for TaggedSliceIterMut<'a, T> {}

impl<'a, T> TaggedSliceIterMut<'a, T> {
    pub fn enumerate(self) -> TaggedEnumerate<Self> {
        let length_id = self.length_id;
        TaggedEnumerate { inner: self, count: 0, length_id }
    }
}

/// Owned iterator over a `Tagged<[Tagged<T>; N]>`. Yields each `Tagged<T>`.
pub struct TaggedArrayIntoIter<T, const N: usize> {
    inner: std::array::IntoIter<Tagged<T>, N>,
    length_id: Id,
}

impl<T, const N: usize> Iterator for TaggedArrayIntoIter<T, N> {
    type Item = Tagged<T>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T, const N: usize> DoubleEndedIterator for TaggedArrayIntoIter<T, N> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<T, const N: usize> ExactSizeIterator for TaggedArrayIntoIter<T, N> {}
impl<T, const N: usize> std::iter::FusedIterator for TaggedArrayIntoIter<T, N> {}

impl<T, const N: usize> TaggedArrayIntoIter<T, N> {
    pub fn enumerate(self) -> TaggedEnumerate<Self> {
        let length_id = self.length_id;
        TaggedEnumerate { inner: self, count: 0, length_id }
    }
}

/// enumerate special casing, yields `(Tagged<usize>, I::Item)` where
/// every index reuses the same `length_id` captured at construction. The
/// counter itself is just a plain `usize`; only the value gets tagged.
pub struct TaggedEnumerate<I> {
    inner: I,
    count: usize,
    length_id: Id,
}

impl<I: Iterator> Iterator for TaggedEnumerate<I> {
    type Item = (Tagged<usize>, I::Item);
    fn next(&mut self) -> Option<Self::Item> {
        let elem = self.inner.next()?;
        let i = self.count;
        self.count += 1;
        Some((Tagged(self.length_id, i), elem))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for TaggedEnumerate<I> {}
impl<I: std::iter::FusedIterator> std::iter::FusedIterator for TaggedEnumerate<I> {}

// =================== INHERENT iter / iter_mut ===================
// These shadow the iter / iter_mut reachable through Deref so user code
// like arr.iter() resolves here without any AST instrumentation.

impl<'a, T> TaggedRef<'a, [Tagged<T>]> {
    pub fn iter(&self) -> TaggedSliceIter<'a, T> {
        TaggedSliceIter { inner: self.1.iter(), length_id: *self.0 }
    }
}

impl<'a, T, const N: usize> TaggedRef<'a, [Tagged<T>; N]> {
    pub fn iter(&self) -> TaggedSliceIter<'a, T> {
        TaggedSliceIter { inner: self.1.iter(), length_id: *self.0 }
    }
}

impl<'a, T> TaggedRefMut<'a, [Tagged<T>]> {
    pub fn iter_mut(&mut self) -> TaggedSliceIterMut<'_, T> {
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id: *self.0 }
    }
}

impl<'a, T, const N: usize> TaggedRefMut<'a, [Tagged<T>; N]> {
    pub fn iter_mut(&mut self) -> TaggedSliceIterMut<'_, T> {
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id: *self.0 }
    }
}

impl<T, const N: usize> Tagged<[Tagged<T>; N]> {
    pub fn iter(&self) -> TaggedSliceIter<'_, T> {
        TaggedSliceIter { inner: self.1.iter(), length_id: self.0 }
    }
    pub fn iter_mut(&mut self) -> TaggedSliceIterMut<'_, T> {
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id: self.0 }
    }
}

// =================== IntoIterator ===================

impl<'a, T> IntoIterator for TaggedRef<'a, [Tagged<T>]> {
    type Item = TaggedRef<'a, T>;
    type IntoIter = TaggedSliceIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        TaggedSliceIter { inner: self.1.iter(), length_id: *self.0 }
    }
}

impl<'a, T, const N: usize> IntoIterator for TaggedRef<'a, [Tagged<T>; N]> {
    type Item = TaggedRef<'a, T>;
    type IntoIter = TaggedSliceIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        TaggedSliceIter { inner: self.1.iter(), length_id: *self.0 }
    }
}

impl<'a, T> IntoIterator for TaggedRefMut<'a, [Tagged<T>]> {
    type Item = TaggedRefMut<'a, T>;
    type IntoIter = TaggedSliceIterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id: *self.0 }
    }
}

impl<'a, T, const N: usize> IntoIterator for TaggedRefMut<'a, [Tagged<T>; N]> {
    type Item = TaggedRefMut<'a, T>;
    type IntoIter = TaggedSliceIterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id: *self.0 }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a Tagged<[Tagged<T>; N]> {
    type Item = TaggedRef<'a, T>;
    type IntoIter = TaggedSliceIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        TaggedSliceIter { inner: self.1.iter(), length_id: self.0 }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut Tagged<[Tagged<T>; N]> {
    type Item = TaggedRefMut<'a, T>;
    type IntoIter = TaggedSliceIterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        let length_id = self.0;
        TaggedSliceIterMut { inner: self.1.iter_mut(), length_id }
    }
}

impl<T, const N: usize> IntoIterator for Tagged<[Tagged<T>; N]> {
    type Item = Tagged<T>;
    type IntoIter = TaggedArrayIntoIter<T, N>;
    fn into_iter(self) -> Self::IntoIter {
        // weirdly, self.1.into_iter() resolves to the <&[T; N]>::into_iter,
        // rather than the following call...
        TaggedArrayIntoIter {
            inner: <[Tagged<T>; N] as IntoIterator>::into_iter(self.1),
            length_id: self.0,
        }
    }
}
