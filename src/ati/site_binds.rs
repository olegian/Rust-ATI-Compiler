use crate::ati::ati::Site;
use crate::ati::tagged::{
    Tagged, TaggedArray, TaggedRange, TaggedRangeFrom, TaggedRangeFull, TaggedRangeInclusive,
    TaggedRangeTo, TaggedRangeToInclusive, TaggedRef, TaggedRefMut,
};

/// Provides a method for recursively associating a variable `self` to some
/// ATI site, with the given name. All `Tagged<T>`'s should implement this trait.
/// Compound types have to add an implementation of this trait during compile
/// time, to allow them to be bound to sites during runtime.
/// The implementations below are required for the atomic/primitive types.
pub trait SiteBind {
    fn bind(&self, site: &mut Site, var_name: &str);
}

// ==========================    BASIC TYPES   ===============================

/// Most generic implementation used by all non-tagged types.
/// If the type is not tagged, then there is nothing to bind to the site,
/// resulting in a no-op.
impl<T> SiteBind for T {
    default fn bind(&self, _site: &mut Site, _var_name: &str) {}
}

/// Most generic implementation used by all atomic tagged types (like `Tagged<u32>`).
/// References to these values use `TaggedRef` / `TaggedRefMut` and bind the same
/// way (via the borrowed Id).
impl<T> SiteBind for Tagged<T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}
impl<'a, T: ?Sized> SiteBind for TaggedRef<'a, T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, *self.0);
    }
}
impl<'a, T: ?Sized> SiteBind for TaggedRefMut<'a, T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, *self.0);
    }
}

// Non-instrumented references to a SiteBind type delegate to their referent.
// The outer `&` / `&mut` carries no Id of its own, it's a raw Rust reference
// kept because pass 2 only converts the innermost `&` in a chain to a
// `TaggedRef`. For nested shapes like `&&TaggedRef<u32>` (from source `&&&u32`),
// these impls unwrap each outer layer until a `Tagged` / `TaggedRef` /
// `TaggedRefMut` / user-struct impl handles the actual bind.
impl<T> SiteBind for &T {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        (**self).bind(site, var_name);
    }
}
impl<T> SiteBind for &mut T {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        (**self).bind(site, var_name);
    }
}

// ==========================    ARRAY TYPES   ===============================

/// Binding an array should associate the length and values inside the array.
/// References to these arrays share the same recursive treatment, the
/// TaggedRef-over-array specialization walks each element via the
/// TaggedRef<..> -over-element impls that follow.
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

// ==========================    SLICE TYPES   ===============================

/// Slices are repred as `TaggedRef<'_, [T]>` / `TaggedRefMut<'_, [T]>`. The source-level
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

// ==========================    RANGE TYPES   ===============================

/// Im not convinced that all ranges need a start/end bind, which is separate from the outer one.
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

// ==========================    TUPLE TYPES   ===============================
/// Implements SiteBinds for tuples, where each entry in the tuple gets bound
/// to the site.
/// This is implemented for tuples up to length 12, following the convention of
/// the standard library. If more than 12 is ever necessary, add the site binds below.
macro_rules! tuple_impl_site_bind {
    ($($idx:tt : $T:ident),+) => {
        impl<$($T),+> SiteBind for ($($T,)+) {
            fn bind(&self, site: &mut Site, var_name: &str) {
                $(
                    self.$idx.bind(site, &format!("{}.{}", var_name, stringify!($idx)));
                )+
            }
        }
        impl<'a, $($T),+> SiteBind for TaggedRef<'a, ($($T,)+)> {
            fn bind(&self, site: &mut Site, var_name: &str) {
                site.bind(var_name, *self.0);
                $(
                    self.1.$idx.bind(site, &format!("{}.{}", var_name, stringify!($idx)));
                )+
            }
        }
        impl<'a, $($T),+> SiteBind for TaggedRefMut<'a, ($($T,)+)> {
            fn bind(&self, site: &mut Site, var_name: &str) {
                site.bind(var_name, *self.0);
                $(
                    self.1.$idx.bind(site, &format!("{}.{}", var_name, stringify!($idx)));
                )+
            }
        }
    };
}

tuple_impl_site_bind!(0: A, 1: B);
tuple_impl_site_bind!(0: A, 1: B, 2: C);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K, 11: L);
tuple_impl_site_bind!(0: A, 1: B, 2: C, 3: D, 4: E, 5: F, 6: G, 7: H, 8: I, 9: J, 10: K, 11: L, 12: M);
