//! Range support for the runtime library.
//!
//! Ranges in instrumented code carry a wrapper [Id](crate::ati::tagged::Id) on top of the
//! standard library range structure. The wrapper id is unioned with the endpoint ids at
//! construction, so any iteration or indexing through the range interacts with both endpoints.
//! Pass 2 lowers each `a..b` form to one of the `track_range_*` constructors below.
//!
//! This file contains every range-shaped helper, including the six type aliases, the
//! constructors on [ATI], the [Iterator] and [RangeBounds](std::ops::RangeBounds)
//! implementations,
//! [TaggedSliceIndex] implementations for
//! `arr[range]`-style indexing, and the [SiteBind]
//! implementations used to register a range to a site.

use crate::ati::arrays::TaggedSliceIndex;
use crate::ati::ati::{ATI, ATI_ANALYSIS, Site};
use crate::ati::site_binds::SiteBind;
use crate::ati::tagged::Tagged;

// =================== TYPE ALIASES ===================

/// Tagged half-open range, the wrapped form of `start..end`.
pub type TaggedRange<T> = Tagged<std::ops::Range<Tagged<T>>>;
/// Tagged inclusive range, the wrapped form of `start..=end`.
pub type TaggedRangeInclusive<T> = Tagged<std::ops::RangeInclusive<Tagged<T>>>;
/// Tagged unbounded-end range, the wrapped form of `start..`.
pub type TaggedRangeFrom<T> = Tagged<std::ops::RangeFrom<Tagged<T>>>;
/// Tagged unbounded-start half-open range, the wrapped form of `..end`.
pub type TaggedRangeTo<T> = Tagged<std::ops::RangeTo<Tagged<T>>>;
/// Tagged unbounded-start inclusive range, the wrapped form of `..=end`.
pub type TaggedRangeToInclusive<T> = Tagged<std::ops::RangeToInclusive<Tagged<T>>>;
/// Tagged fully unbounded range, the wrapped form of `..`.
pub type TaggedRangeFull = Tagged<std::ops::RangeFull>;

// =================== CONSTRUCTORS ===================

impl ATI {
    /// Constructs a tagged half-open range. Allocates a fresh wrapper id and unions it with
    /// both endpoint ids, so any later iteration or indexing through this range interacts
    /// with both endpoints.
    pub fn track_range<T>(start: Tagged<T>, end: Tagged<T>) -> Tagged<std::ops::Range<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.make_id();
        ati.union_and_get_id(&id, &start.0);
        ati.union_and_get_id(&id, &end.0);
        Tagged(id, std::ops::Range { start, end })
    }

    /// Inclusive variant of [ATI::track_range].
    pub fn track_range_inclusive<T>(
        start: Tagged<T>,
        end: Tagged<T>,
    ) -> Tagged<std::ops::RangeInclusive<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.make_id();
        ati.union_and_get_id(&id, &start.0);
        ati.union_and_get_id(&id, &end.0);
        Tagged(id, std::ops::RangeInclusive::new(start, end))
    }

    /// Open-ended variant of [ATI::track_range], only the start endpoint is bound.
    pub fn track_range_from<T>(start: Tagged<T>) -> Tagged<std::ops::RangeFrom<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.make_id();
        ati.union_and_get_id(&id, &start.0);
        Tagged(id, std::ops::RangeFrom { start })
    }

    /// Half-open variant of [ATI::track_range] with no start.
    pub fn track_range_to<T>(end: Tagged<T>) -> Tagged<std::ops::RangeTo<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.make_id();
        ati.union_and_get_id(&id, &end.0);
        Tagged(id, std::ops::RangeTo { end })
    }

    /// Inclusive variant of [ATI::track_range_to].
    pub fn track_range_to_inclusive<T>(
        end: Tagged<T>,
    ) -> Tagged<std::ops::RangeToInclusive<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.make_id();
        ati.union_and_get_id(&id, &end.0);
        Tagged(id, std::ops::RangeToInclusive { end })
    }

    /// Fully unbounded variant of [ATI::track_range]. Carries only the wrapper id.
    pub fn track_range_full() -> Tagged<std::ops::RangeFull> {
        let id = ATI_ANALYSIS.lock().unwrap().make_id();
        Tagged(id, std::ops::RangeFull)
    }
}

// =================== LEN ===================

impl<T> TaggedRange<T>
where
    T: Copy + std::ops::Sub<Output = T> + std::cmp::PartialOrd,
    usize: std::convert::TryFrom<T>,
{
    /// Length of this half-open range as a tagged `usize`. Reuses the wrapper id, so the
    /// returned length carries the same id the original range was constructed with. Returns 0
    /// for an inverted (empty) range, and 0 if `end - start` does not fit in a `usize`.
    pub fn len(&self) -> Tagged<usize> {
        let start = self.1.start.1;
        let end = self.1.end.1;
        let n = if end <= start {
            0
        } else {
            <usize as std::convert::TryFrom<T>>::try_from(end - start)
                .ok()
                .unwrap_or(0)
        };
        Tagged(self.0, n)
    }
}

impl<T> TaggedRangeInclusive<T>
where
    T: Copy + std::ops::Sub<Output = T> + std::cmp::PartialOrd,
    usize: std::convert::TryFrom<T>,
{
    /// Length of this inclusive range as a tagged `usize`. Same convention as
    /// [TaggedRange::len], with `end - start + 1` for the count and saturating addition to
    /// avoid overflow at the upper bound.
    pub fn len(&self) -> Tagged<usize> {
        let start = self.1.start().1;
        let end = self.1.end().1;
        let n = if end < start {
            0
        } else {
            match <usize as std::convert::TryFrom<T>>::try_from(end - start) {
                Ok(d) => d.saturating_add(1),
                Err(_) => 0,
            }
        };
        Tagged(self.0, n)
    }
}

// =================== ITERATOR IMPLS ===================

/// Iterator impls for tagged ranges. Rather than reimplementing every
/// Iterator adapter (.map, .filter, .sum, ...) we impl `Iterator` once on
/// the Tagged range itself.
///
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

/// `RangeInclusive` has a hidden `exhausted` flag we can't reach, so we encode exhaustion by
/// leaving `start > end` after yielding the final value. At the bound (`T::MAX` for `next`,
/// `T::MIN` for `next_back`), stepping the just-yielded endpoint is impossible, so we instead
/// step the opposite endpoint past the just-yielded one, which makes the next call observe
/// `start > end` and return `None`.
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
        match <T as std::iter::Step>::forward_checked(start, 1) {
            Some(next_start) => {
                self.1 = Tagged(start_id, next_start)..=Tagged(end_id, end);
            }

            None => {
                // start is at the upper bound. Stepping it forward overflows, so step `end`
                // backward by 1 instead. The new range satisfies `start > end`, so the next
                // call returns None.
                if let Some(new_end) = <T as std::iter::Step>::backward_checked(end, 1) {
                    self.1 = Tagged(start_id, start)..=Tagged(end_id, new_end);
                }
            }
        }
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
        match <T as std::iter::Step>::backward_checked(end, 1) {
            Some(next_end) => {
                self.1 = Tagged(start_id, start)..=Tagged(end_id, next_end);
            }
            None => {
                // end is at the lower bound. Stepping it backward underflows, so step `start`
                // forward by 1 instead. The new range satisfies `start > end`, so the next
                // call returns None.
                if let Some(new_start) = <T as std::iter::Step>::forward_checked(start, 1) {
                    self.1 = Tagged(start_id, new_start)..=Tagged(end_id, end);
                }
            }
        }
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

// =================== RANGE BOUNDS ===================

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

// =================== TAGGED SLICE INDEX ===================

impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRange<T>
where
    std::ops::Range<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::Range<T>;
    fn into_raw(self) -> Self::Raw {
        self.1.start.1..self.1.end.1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeInclusive<T>
where
    std::ops::RangeInclusive<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeInclusive<T>;
    fn into_raw(self) -> Self::Raw {
        self.1.start().1..=self.1.end().1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeFrom<T>
where
    std::ops::RangeFrom<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeFrom<T>;
    fn into_raw(self) -> Self::Raw {
        self.1.start.1..
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeTo<T>
where
    std::ops::RangeTo<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeTo<T>;
    fn into_raw(self) -> Self::Raw {
        ..self.1.end.1
    }
}
impl<Idx, T: Copy> TaggedSliceIndex<Idx> for TaggedRangeToInclusive<T>
where
    std::ops::RangeToInclusive<T>: std::slice::SliceIndex<[Idx], Output = [Idx]>,
{
    type Raw = std::ops::RangeToInclusive<T>;
    fn into_raw(self) -> Self::Raw {
        ..=self.1.end.1
    }
}
impl<T> TaggedSliceIndex<T> for TaggedRangeFull {
    type Raw = std::ops::RangeFull;
    fn into_raw(self) -> Self::Raw {
        ..
    }
}

// =================== SITE BIND ===================

// FIXME: Im not convinced that all ranges need a start/end bind, which is separate from 
// the outer one.
impl<T> SiteBind for TaggedRange<T> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
        site.bind(&format!("{var_name}.start"), self.1.start.0);
        site.bind(&format!("{var_name}.end"), self.1.end.0);
    }
}
impl<T> SiteBind for TaggedRangeInclusive<T> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
        site.bind(&format!("{var_name}.start"), self.1.start().0);
        site.bind(&format!("{var_name}.end"), self.1.end().0);
    }
}
impl<T> SiteBind for TaggedRangeFrom<T> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
        site.bind(&format!("{var_name}.start"), self.1.start.0);
    }
}
impl<T> SiteBind for TaggedRangeTo<T> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
        site.bind(&format!("{var_name}.end"), self.1.end.0);
    }
}
impl<T> SiteBind for TaggedRangeToInclusive<T> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
        site.bind(&format!("{var_name}.end"), self.1.end.0);
    }
}
impl SiteBind for TaggedRangeFull {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}
