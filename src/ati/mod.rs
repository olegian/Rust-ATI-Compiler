//! Runtime library injected into the instrumented crate.
//!
//! Every file in this module is read by `crate::callbacks::codegen::define_types` and inserted
//! into the crate root during the second compilation pass. The injected code provides the
//! types and globals that instrumentation calls into at runtime.
//!
//! At a high-level:
//!
//! [ati] defines [ATI](ati::ATI), [Site](ati::Site), [Sites](ati::Sites), and
//! [UnionFind](ati::UnionFind), the data structures that record value interactions and produce
//! the abstract type partition. The [ATI_ANALYSIS](ati::ATI_ANALYSIS) global owns the single
//! live instance of this state.
//!
//! [tagged] defines the [Tagged](tagged::Tagged) wrapper that pairs a tracked value with
//! a unique [Id](tagged::Id). [refs] defines [TaggedRef](refs::TaggedRef) and
//! [TaggedRefMut](refs::TaggedRefMut), the shared and unique borrow forms emitted whenever
//! pass 2 takes a reference to a tracked value.
//!
//! [arrays] holds every array and slice helper, including the
//! [TaggedArray](arrays::TaggedArray) type alias, length and indexing operators, slice-index
//! sugar, and the [SiteBind](site_binds::SiteBind) implementations for array and slice
//! shapes. [ranges] holds every range helper, including the six tagged range type aliases,
//! the constructors on [ATI](ati::ATI), iterator and [RangeBounds](std::ops::RangeBounds)
//! implementations, the [TaggedSliceIndex](arrays::TaggedSliceIndex) implementations, and
//! the [SiteBind](site_binds::SiteBind) implementations for ranges.
//!
//! [tagged_ops] implements the standard arithmetic, comparison, and shift operator traits
//! for the tagged wrappers. Each operator unions the operand ids in the value union-find
//! before delegating to the underlying primitive. [iterators] provides shim iterators that
//! yield [TaggedRef](refs::TaggedRef) and [TaggedRefMut](refs::TaggedRefMut) elements, plus
//! an enumerate variant that emits tagged indices.
//!
//! [site_binds] defines the [SiteBind](site_binds::SiteBind) trait and its blanket and
//! per-shape implementations. Pass 2's generated shims call `.bind()` on every variable to
//! register its tag with the enclosing [Site](ati::Site).
//!
//! The library is only injected once, into the crate root, so the
//! [ATI_ANALYSIS](ati::ATI_ANALYSIS) global is defined exactly once. Dependency files reach
//! these symbols through the `use crate::*;` statement injected by pass 2.

pub mod arrays;
pub mod ati;
pub mod iterators;
pub mod ranges;
pub mod refs;
pub mod site_binds;
pub mod tagged;
pub mod tagged_ops;
