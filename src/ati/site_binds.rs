//! Defines [SiteBind], the trait pass 2 invokes from generated shims to register every
//! in-scope tracked variable with a [Site].
//!
//! The blanket and atomic implementations live here. Array, slice, and range shapes are
//! covered in [crate::ati::arrays] and [crate::ati::ranges], next to the rest of those
//! shapes' helpers.
//!
//! For user-defined compound types (structs, enums), pass 2's codegen step in
//! `crate::callbacks::codegen::data_types` generates a per-type [SiteBind] implementation
//! that recursively delegates to each field's [SiteBind].

use crate::ati::ati::Site;
use crate::ati::refs::{TaggedRef, TaggedRefMut};
use crate::ati::tagged::Tagged;

/// Recursively binds the receiver as a variable named `var_name` at `site`. Every
/// `Tagged<T>` should implement this trait. Compound types have a per-type implementation
/// generated at compile time so that all of their fields can be bound at runtime.
///
/// The implementations below cover the atomic and primitive cases.
pub trait SiteBind {
    /// Records this value's id (or each of its component ids, for compound shapes) at `site`,
    /// keyed by `var_name`.
    fn bind(&self, site: &mut Site, var_name: &str);
}

// ==========================    BASIC TYPES   ===============================

/// Blanket implementation used by all non-tagged types. If the type is not tagged, there is
/// nothing to bind to the site, so this is a no-op.
impl<T> SiteBind for T {
    default fn bind(&self, _site: &mut Site, _var_name: &str) {}
}

/// Blanket implementation used by all atomic tagged types (like `Tagged<u32>`). The
/// per-shape impls in [crate::ati::arrays] and [crate::ati::ranges] override this for
/// arrays, slices, and ranges via specialization.
impl<T> SiteBind for Tagged<T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, self.0);
    }
}
/// Blanket implementation for shared tagged borrows, binds via the borrowed id.
impl<'a, T: ?Sized> SiteBind for TaggedRef<'a, T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, *self.0);
    }
}
/// Blanket implementation for unique tagged borrows, binds via the borrowed id.
impl<'a, T: ?Sized> SiteBind for TaggedRefMut<'a, T> {
    default fn bind(&self, site: &mut Site, var_name: &str) {
        site.bind(var_name, *self.0);
    }
}

// Non-instrumented references to a SiteBind type delegate to their referent. The outer `&`
// or `&mut` carries no id of its own, it's a raw Rust reference kept because pass 2 only
// converts the innermost `&` in a chain to a `TaggedRef`. For nested shapes like
// `&&TaggedRef<u32>` (from source `&&&u32`), these impls unwrap each outer layer until a
// `Tagged` / `TaggedRef` / `TaggedRefMut` / user-struct impl handles the actual bind.
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

// ==========================    TUPLE TYPES   ===============================
/// Implements [SiteBind] for tuples, with each entry bound under a `.0` / `.1` / etc. suffix
/// of the parent name. Implemented for tuples up to length 12, following the convention of
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
