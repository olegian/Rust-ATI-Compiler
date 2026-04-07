/* This file is a part of the runtime library injected into the compiled project.
 * It defines the Tagged<T> type which ultimately represents a tuple (Id, T). All
 * tracked values are transformed into this tagged type to be able to uniquely
 * identify where they are used. Id's are used within the various union find structure
 * (defined in ati.rs), to track interactions between values, and represent abstract
 * type sets.
*/

use crate::ati::ati::Site;

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
pub struct Tagged<T>(pub Id, pub T);

impl<T, const N: usize> Tagged<[T; N]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, N)
    }
}

impl<T> Tagged<&[T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, self.1.len())
    }
}

impl<T> Tagged<&mut [T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, self.1.len())
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

/// The BindToSite trait is defined for all type T, and allows
/// for all data types to be observed at a particular site. The
/// `T.bind()` function is called within function stubs.
pub trait BindToSite {
    fn bind(&self, site: &mut Site, var_name: &str);
}

/// Most generic implementation used by all non-tagged types.
impl<T> BindToSite for T {
    default fn bind(&self, site: &mut Site, var_name: &str) {}
}

/// Most generic implementation used by all tagged types.
impl<T> BindToSite for Tagged<T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}

impl<T> BindToSite for &Tagged<T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}

impl<T> BindToSite for &mut Tagged<T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}

/// More specific implementation, used when binding arrays.
/// This has every element of the array be represented within the site,
/// alongside the length of the array.
impl<T, const N: usize> BindToSite for Tagged<[T; N]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

impl<T, const N: usize> BindToSite for &Tagged<[T; N]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

impl<T, const N: usize> BindToSite for &mut Tagged<[T; N]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..N {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

/// Similar to BindToSite for Tagged<[T; N]>, but for slices instead!
impl<T> BindToSite for Tagged<&[T]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..self.len().1 {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

impl<T> BindToSite for &Tagged<&[T]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..self.len().1 {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

impl<T> BindToSite for &mut Tagged<&mut [T]> {
    fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(&format!("{var_name}_LEN"), self.len().0);

        for i in 0..self.len().1 {
            self.1[i].bind(site, &format!("{var_name}[{i}]"));
        }
    }
}

// impl<T> std::cmp::PartialEq for Tagged<T>
// where
//     T: std::cmp::PartialEq,
// {
//     fn eq(&self, other: &Self) -> bool {
//         self.1 == other.1
//     }
// }

// impl<T> std::cmp::PartialOrd for Tagged<T>
// where
//     T: std::cmp::PartialOrd,
// {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         self.1.partial_cmp(&other.1)
//     }
// }

// impl std::range::Step for Tagged<usize> {
//     fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
//         if start.1 <= end.1 {
//             let steps = end.1 - start.1;
//             (steps, Some(steps))
//         } else {
//             (0, None)
//         }
//     }

//     fn forward_checked(start: Self, count: usize) -> Option<Self> {
//         Some(Tagged(start.0, start.1 + count))
//     }

//     fn backward_checked(start: Self, count: usize) -> Option<Self> {
//         Some(Tagged(start.0, start.1 - count))
//     }
// }

// impl<'a, A, B, C> std::ops::Index<Tagged<A>> for Tagged<&'a B>
// where
//     B: std::ops::Index<A, Output=&'a C>,
//     C: 'a
// {
//     type Output = Tagged<&'a C>;

//     fn index(&self, index: Tagged<A>) -> &Self::Output {
//         // let entry = self.1[index.1];
//         &Tagged(self.0, self.1[index.1])
//     }
// }

// could do something like this instead? then replace all [] ops with .at()?
// impl<'a, Collection, Idx, T> Tagged<&'a Collection>
// where
//     Collection: std::ops::Index<Idx, Output=T>,
// {
//     pub fn at(&self, index: Tagged<Idx>) -> Tagged<&'a T> {
//         Tagged(self.0, &self.1[index.1])
//     }

//     pub fn at_range(&self, range: std::range::Range<usize>) -> Tagged<&'a [T]> {
//         todo!()
//     }
// }
