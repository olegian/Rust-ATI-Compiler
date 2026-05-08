//! Defines the owned tagged-value wrapper [Tagged] alongside the [Id] type and the [Tagger]
//! id source.
//!
//! Every tracked value emitted by pass 2 is rewritten from `T` into `Tagged<T>`, which pairs the
//! original value with a unique [Id]. The id is what the union-find structures in
//! [crate::ati::ati] use to record interactions between values and to form abstract-type sets.
//!
//! Borrows of a tagged value (`&Tagged<T>`, `&mut Tagged<T>`) are not used directly. Pass 2
//! converts those into [TaggedRef](crate::ati::refs::TaggedRef) and
//! [TaggedRefMut](crate::ati::refs::TaggedRefMut), defined in [crate::ati::refs].

use crate::ati::ati::ATI_ANALYSIS;

/// Type alias for ids, kept short and easy to swap if the underlying integer width ever needs
/// to change.
pub type Id = u64;

/// Hands out fresh [Id]s, one per call to [Tagger::tag].
#[derive(Debug)]
pub struct Tagger {
    /// Next id that will be returned.
    next_id: Id,
}

impl Tagger {
    /// Creates a new [Tagger] that starts at id 0.
    pub fn new() -> Self {
        Tagger { next_id: 0 }
    }

    /// Returns the next id and advances the internal counter.
    pub fn tag(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        id
    }
}

/// A value of type `T` paired with a unique [Id].
///
/// Not intended to be constructed directly. Use [crate::ati::ati::ATI::track] or one of the
/// `track_range_*` constructors in [crate::ati::ranges] to obtain one.
#[derive(Debug, Clone, Copy)]
pub struct Tagged<T: ?Sized>(pub Id, pub T);

/// Common abstraction over every tagged wrapper, exposing the wrapper's id and a borrow of
/// its inner value.
///
/// Implemented for [Tagged], [TaggedRef](crate::ati::refs::TaggedRef), and
/// [TaggedRefMut](crate::ati::refs::TaggedRefMut). Lets generic code (operator impls, slice
/// indexing) reach into any tagged shape without matching on the concrete wrapper type.
pub trait TagTuple {
    /// The inner value type stored in this tagged wrapper.
    type Inner: ?Sized;
    /// Returns the wrapper's id by value.
    fn id(&self) -> Id;
    /// Returns a borrow of the wrapper's inner value.
    fn value(&self) -> &Self::Inner;
}

impl<T: ?Sized> TagTuple for Tagged<T> {
    type Inner = T;
    fn id(&self) -> Id {
        self.0
    }
    fn value(&self) -> &T {
        &self.1
    }
}

/// Display formatting for a tagged value, written as `(id, value)`. Useful for debugging
/// instrumented code.
impl<T> std::fmt::Display for Tagged<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

/// `Sum` impl that collapses an iterator of tagged values into a single tagged value via
/// repeated addition. The empty case allocates a fresh id and sums the empty `T` iterator,
/// matching the standard library's empty-sum behavior.
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

/// Borrowing variant of [`Sum`](std::iter::Sum) for tagged values, copies each item then
/// delegates to the owning sum.
impl<'a, T: Copy + 'a> std::iter::Sum<&'a Tagged<T>> for Tagged<T>
where
    Tagged<T>: std::ops::Add<Output = Tagged<T>>,
    T: std::iter::Sum,
{
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

/// `Product` impl that collapses an iterator of tagged values into a single tagged value via
/// repeated multiplication. The empty case allocates a fresh id and takes the empty product
/// over `T`.
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
