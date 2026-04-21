use crate::ati::tagged::{Id, Tagged, TaggedArray, TaggedRange, TaggedRangeFrom, TaggedRangeFull, TaggedRangeInclusive, TaggedRangeTo, TaggedRangeToInclusive, TaggedRef, TaggedRefMut};

/// A trait which all collections implement, providing a method which
/// groups together the tags of values at the same nesting level.
/// Used to guarantee that arbitrarily high dimensional arrays
/// maintain the property that all elements, and dimension-sizes
/// are in the same AT.
pub trait Collect {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize);
}

// Leaf cases for single values
impl<T> Collect for Tagged<T> {
    default fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
    }
}

impl<T> Collect for T {
    default fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {}
}

// [T; N]
impl<T, const N: usize> Collect for TaggedArray<T, N> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);

        for i in 0..N {
            self.1[i].collect_ids_by_level(ids, depth + 1);
        }
    }
}

// &[T; N]
impl<'a, T, const N: usize> Collect for TaggedRef<'a, [T; N]> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);

        for i in 0..N {
            self.1[i].collect_ids_by_level(ids, depth + 1);
        }
    }
}

// &mut [T; N]
impl<'a, T, const N: usize> Collect for TaggedRefMut<'a, [T; N]> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);

        for i in 0..N {
            self.1[i].collect_ids_by_level(ids, depth + 1);
        }
    }
}

// &[T]
impl<'a, T> Collect for TaggedRef<'a, [T]> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);

        for i in 0..self.1.len() {
            self.1[i].collect_ids_by_level(ids, depth + 1);
        }
    }
}

// &mut [T]
impl<'a, T> Collect for TaggedRefMut<'a, [T]> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);

        for i in 0..self.1.len() {
            self.1[i].collect_ids_by_level(ids, depth + 1);
        }
    }
}

// &Tagged<T> view - one id at this depth, no further recursion since T is
// presumed to be a tupleable leaf (no further tagged structure inside). The
// non-specialized `Collect for T` fallback would push nothing, which would be
// incorrect since TaggedRef carries an Id.
impl<'a, T> Collect for TaggedRef<'a, T> {
    default fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);
    }
}
impl<'a, T> Collect for TaggedRefMut<'a, T> {
    default fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(*self.0);
    }
}

// Ranges
impl<T> Collect for TaggedRange<T> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
        self.1.start.collect_ids_by_level(ids, depth + 1);
        self.1.end.collect_ids_by_level(ids, depth + 1);
    }
}
impl<T> Collect for TaggedRangeInclusive<T> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
        self.1.start().collect_ids_by_level(ids, depth + 1);
        self.1.end().collect_ids_by_level(ids, depth + 1);
    }
}
impl<T> Collect for TaggedRangeFrom<T> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
        self.1.start.collect_ids_by_level(ids, depth + 1);
    }
}
impl<T> Collect for TaggedRangeTo<T> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
        self.1.end.collect_ids_by_level(ids, depth + 1);
    }
}
impl<T> Collect for TaggedRangeToInclusive<T> {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
        self.1.end.collect_ids_by_level(ids, depth + 1);
    }
}
impl Collect for TaggedRangeFull {
    fn collect_ids_by_level(&self, ids: &mut Vec<Vec<Id>>, depth: usize) {
        if ids.len() <= depth {
            ids.resize(depth + 1, Vec::new());
        }
        ids[depth].push(self.0);
    }
}
