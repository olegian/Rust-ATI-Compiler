//! Defines how facts are transferred between the first "Gather" compilation and the "Instrument"
//! compilation, via the [`FirstPassInfo`] struct.
//!
//! The struct is composed of two well-typed pieces:
//!   * [functions::FnIndex], a registry of functions and methods that will be instrumented, split
//!     into namespaces.
//!   * [`SpanFacts<F>`], generic span-keyed fact store. One field per kind of
//!     syntactic fact pass 1 wants to communicate to pass 2. Markers (set
//!     semantics) use `SpanFacts<()>`, while payload-bearing facts pick a struct for `F`.
//!
//! Overall, this allows the Gather compilation to consistently use the [`SpanFacts::mark`]
//! for set-based facts, and [`SpanFacts::record`] for payload-bearing facts. The instrument
//! compilation can then use [`SpanFacts::contains`] and [`SpanFacts::get`] to query this info
//! in a consistent manner.
//!
//! This kind of representation is chosen so that this structure is easy to extend. To add
//! a new type of tracked fact, choose a payload, and add the field to [`FirstPassInfo`],
//! then utilize the same API described above.

use crate::callbacks::gather::first_pass_info::span_facts::SpanFacts;

mod functions;
mod span_facts;
mod span_key;

pub use functions::{FnNamespace, ModPath};

/// Payload for `untracked_fn_calls`: information about a call to an
/// untracked function, recorded by pass 1 against the call's syntactic span.
#[derive(Debug, Clone, Copy)]
pub struct UntrackedCall {
    /// Whether the return type at the call site is tupleable (i.e. a tracked
    /// primitive).
    pub ret_is_tupleable: bool,
    // FIXME: these function calls could return complex types, like structs,
    // which can be tupled but that requires defining a new struct with
    // Tagged variants of all fields, and that's hard to do, ignoring for
    // now. hopefully it won't be a problem. We almost definitely want
    // more fields in this struct
}

/// Contains all information that is going to be passed between the
/// first and second compilation rounds. Populated by invoking the
/// compiler using the GatherAtiInfo callbacks.
///
/// Each field is self-contained and exposes its own `record`/`get` (or
/// `mark`/`contains`) API.
#[derive(Debug, Default)]
pub struct FirstPassInfo {
    /// Registry of every fn/method to instrument.
    pub fns: functions::FnIndex,

    /// Calls to untracked functions, keyed by call-expression span.
    pub untracked_fn_calls: SpanFacts<UntrackedCall>,

    /// Indexing expressions where a range is used as the index. These are places where a 
    /// `.subslice()` call must be inserted.
    pub index_by_range: SpanFacts<()>,

    /// Spans of `Ref` expressions referring to a type T which is tupleable, which require a
    /// `.as_tagged_ref()` call.
    pub ref_to_tupleable_ty: SpanFacts<()>,

    /// Spans of unary `*` expressions whose operand's type is `&T` / `&mut T`
    /// with `T` tupleable. Post-instrumentation these operate on a
    /// `TaggedRef` / `TaggedRefMut`, and a raw `*` would strip the tag.
    pub tag_stripping_deref: SpanFacts<()>,

    /// Spans of `Assign` / `AssignOp` whose LHS is `*expr` with `expr` typed
    /// `&mut T` and `T` tupleable. These are places where a `.assign()` call must be used 
    /// to write to both the value and Id places.
    pub assign_through_tagged_ref_mut: SpanFacts<()>,

    /// Spans of expressions whose post-instrumentation type is a
    /// `TaggedRefMut<T>`, i.e. their typeck-resolved (post-adjustment) type
    /// is `&mut T` with `T` tupleable. `TaggedRefMut` is move-only, so any
    /// pass-2 rewrite that consumes such an expression (binding it into
    /// `let __ati_lhs = ...`, moving it into the emitted parameters of the inner function) must 
    /// reborrow instead, otherwise the original binding is invalidated for any use
    /// later in the function. The reborrow is always a semantically safe operation in an
    /// operand position.
    pub ref_mut_to_tupleable: SpanFacts<()>,

    /// Spans of match target expressions which are tagged types. These types 
    /// require untupling, so that the patterns within each arm of the statement
    /// can actually match on the target.
    pub match_on_tagged: SpanFacts<()>,

    /// Spans of literal / range sub-patterns whose pattern type is tupleable.
    /// Post-instrumentation, the position holding such a sub-pattern has type
    /// `Tagged<T>` rather than `T`, so a bare literal/range no longer
    /// type-checks. Pass 2 lifts each marked sub-pattern out into a fresh
    /// binding plus a match-guard fragment that re-checks the original
    /// pattern against the dereferenced inner value.
    pub tagged_lit_pat: SpanFacts<()>,
}
