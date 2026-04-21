/* Because we are invoking the compiler multiple times, we need some
 * way of relaying information between the multiple compilations. This file
 * defines some structs which can be used for just that.
 *
 * FirstPassInfo is used to relay information from the first pass, which
 * discovers what functions we are going to be instrumenting and where we are
 * making calls to untracked functions.
 *
 * FirstPassInfo is then used to during the second compilation to only
 * instrument specific functions, during which StubInfo is constructed.
 * StubInfo is used to record the updated data types used in function
 * inputs and outputs, as well as the function name and parameter names.
 * StubInfo is then consumed by the stub creation process, to add in
 * the correct stubs responsible for managing sites.
*/

use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::DefId;
use rustc_middle as mir;
use rustc_span::{Ident, Span};

use crate::common::CanBeTupled;

/// Contains all information that is going to be passed between the
/// first and second compilation rounds. Populated by invoking the
/// compiler, using the GatherAtiInfo callbacks.
#[derive(Debug, Default)]
pub struct FirstPassInfo {
    /// which user-defined functions are instrumented across the entire project
    tracked_fn_def_ids: HashSet<DefId>,
    tracked_fn_idents: HashSet<Ident>,

    /// places where a track_slice needs to be inserted, as a coercion from an array to a slice type occurred
    index_by_range_locs: HashSet<Span>,

    /// places where a non-tracked function is called
    /// mapped to whether the return type at that call site is tupleable (i.e. a tracked primitive).
    // FIXME: these function calls could return complex types, like structs, which can be tupled but that requires
    // defining a new struct with Tagged variants of all fields, and that's hard to do :(, ignoring for now.
    // hopefully it won't be a problem...
    untracked_fn_calls: HashMap<Span, bool>,

    /// Spans of Ref expressions, which refer to a type T which is tupleable.
    ref_to_tupleable_ty: HashSet<Span>,

    /// Spans of unary `*` expressions whose operand's type is `&T` / `&mut T`
    /// with `T` tupleable. Post-instrumentation these operate on a
    /// `TaggedRef` / `TaggedRefMut`, and a raw `*` would strip the tag —
    /// pass 2 rewrites them to rebuild a `Tagged<T>` from the borrowed fields.
    tag_stripping_deref_locs: HashSet<Span>,

    /// Spans of `Assign` / `AssignOp` whose LHS is `*expr` with `expr` typed
    /// `&mut T` and `T` tupleable. A raw write through the instrumented
    /// `TaggedRefMut<T>` hits `DerefMut` and only overwrites `.1` — the id
    /// from the RHS never reaches the slot. Pass 2 rewrites these sites to
    /// `expr.assign(rhs)` (double-write, no UF merge: assignment is not an
    /// interaction).
    assign_through_tagged_ref_mut_locs: HashSet<Span>,
}

impl FirstPassInfo {
    /// register that a function with `ident` and `def_id` should
    /// instrumented later
    // NOTE: This is only really useful for extern crates and library files
    // that we are unable to instrument. For now, there is no reason to do this
    // as we assume that all code
    pub fn observe_tracked_fn(&mut self, ident: &Ident, def_id: DefId) {
        self.tracked_fn_idents.insert(ident.clone());
        self.tracked_fn_def_ids.insert(def_id);
    }

    /// register that a function call was made to an untracked function at
    /// `loc`, which returned a value of type `ty`
    pub fn observe_untracked_fn_call<'a>(&mut self, loc: Span, ty: mir::ty::Ty<'a>) {
        self.untracked_fn_calls.insert(loc, ty.can_be_tupled());
    }

    pub fn observe_index_by_range(&mut self, loc: Span) {
        self.index_by_range_locs.insert(loc);
    }
    
    pub fn observe_ref_to_tupleable_ty(&mut self, loc: Span) {
        self.ref_to_tupleable_ty.insert(loc);
    }
    
    pub fn is_span_ref_to_tupleable_ty(&self, loc: &Span) -> bool {
        self.ref_to_tupleable_ty.contains(loc)
    }

    pub fn observe_tag_stripping_deref(&mut self, loc: Span) {
        self.tag_stripping_deref_locs.insert(loc);
    }

    pub fn observe_assign_through_tagged_ref_mut(&mut self, loc: Span) {
        self.assign_through_tagged_ref_mut_locs.insert(loc);
    }

    pub fn is_tag_stripping_deref(&self, loc: &Span) -> bool {
        self.tag_stripping_deref_locs.contains(loc)
    }

    pub fn is_assign_through_tagged_ref_mut(&self, loc: &Span) -> bool {
        self.assign_through_tagged_ref_mut_locs.contains(loc)
    }

    /// returns true if this identifier represent a tracked function
    pub fn is_fn_ident_tracked(&self, ident: &Ident) -> bool {
        self.tracked_fn_idents.contains(ident)
    }

    /// returns true if this def_id represents a tracked function
    pub fn is_fn_def_id_tracked(&self, def_id: &DefId) -> bool {
        self.tracked_fn_def_ids.contains(def_id)
    }

    /// returns whether the return type of an untracked function call at this
    /// location is tupleable, if such a call exists
    pub fn is_untracked_call_ret_tupleable(&self, location: &Span) -> Option<bool> {
        self.untracked_fn_calls.get(location).copied()
    }

    pub fn is_span_index_by_range(&self, location: &Span) -> bool {
        self.index_by_range_locs.contains(location)
    }
}
