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

/// ::-joined module path matching `tcx.def_path_str` format.
/// `""` denotes the crate root.
pub type ModPath = String;

/// Namespace key for an impl-method's enclosing impl block. Distinguishes
/// inherent impls from trait impls so that impl Foo { fn bar() } and
/// impl SomeTrait for Foo { fn bar() } don't collide in the
/// (mod_path, type_key, method_ident) slot in FirstPassInfo.
///
/// Both paths are stored as fully qualified paths.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TypeKey {
    /// ::-joined path of the impl's self type,
    /// Generic args are dropped (impl Foo<u32> and impl<T> Foo<T>
    /// both produce "Foo").
    pub self_path: String,
    /// Some(path) for `impl Trait for T`, None for inherent impls. Same
    /// ::-joined ident-only format as self_path.
    pub trait_path: Option<String>,
}

impl TypeKey {
    /// Constructor for non-trait based impls
    pub fn inherent(self_path: impl Into<String>) -> Self {
        Self {
            self_path: self_path.into(),
            trait_path: None,
        }
    }

    /// Constructor for trait based impls
    pub fn trait_impl(self_path: impl Into<String>, trait_path: impl Into<String>) -> Self {
        Self {
            self_path: self_path.into(),
            trait_path: Some(trait_path.into()),
        }
    }
}

impl std::fmt::Display for TypeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.trait_path {
            Some(t) => write!(f, "{} as {}", self.self_path, t),
            None => f.write_str(&self.self_path),
        }
    }
}

/// Contains per-fn information learned in pass 1
#[derive(Debug, Clone)]
pub struct FnEntry {
    pub ident: Ident,
    pub def_id: DefId,

    /// DeclsFile::ppt_base_name for this function.
    pub base_ppt_name: String,
}

/// All instrumented fns defined in a single module, partitioned by their
/// namespace (free fn vs. method on a type).
///
/// It's important to use Strings rather than any kind of Symbol (or anything
/// that is compile-session dependant) as this needs to be stable between
/// the two compilation passes
#[derive(Debug, Default)]
pub struct ModEntry {
    /// Free functions in this module, keyed by fn name.
    pub free_fns: HashMap<String, FnEntry>,
    /// Methods, keyed by `TypeKey` and then by method name.
    pub methods: HashMap<TypeKey, HashMap<String, FnEntry>>,
}

/// Contains all information that is going to be passed between the
/// first and second compilation rounds. Populated by invoking the
/// compiler, using the GatherAtiInfo callbacks.
#[derive(Debug, Default)]
pub struct FirstPassInfo {
    /// Per-module index of every fn/method that pass 1 wants pass 2 to
    /// instrument. Note that ModEntry will also hold the appropriate
    /// base_ppt_name to use when constructing sites.
    mods: HashMap<ModPath, ModEntry>,

    /// Flat set of every tracked fn/method's DefId. Maintained as a side
    /// cache when entries are added to mods because find_calls.rs needs to
    /// determine if a particular call is tracked only given a DefId resolved by typeck.
    tracked_fn_def_ids: HashSet<DefId>,

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
    /// `TaggedRef` / `TaggedRefMut`, and a raw `*` would strip the tag.
    tag_stripping_deref_locs: HashSet<Span>,

    /// Spans of `Assign` / `AssignOp` whose LHS is `*expr` with `expr` typed
    /// `&mut T` and `T` tupleable. 
    assign_through_tagged_ref_mut_locs: HashSet<Span>,
}

impl FirstPassInfo {
    /// Record that `def_id` (with display `ident`, item span `item_span`, and
    /// `DeclsFile`-format `base_ppt_name`) lives at `mod_path` and should be
    /// instrumented. `type_key` is `Some(t)` for impl methods on type `t` (head
    /// ident, no generic args) and `None` for free fns
    pub fn observe_fn(
        &mut self,
        mod_path: ModPath,
        type_key: Option<TypeKey>,
        ident: Ident,
        def_id: DefId,
        base_ppt_name: String,
    ) {
        let entry = FnEntry {
            ident,
            def_id,
            base_ppt_name,
        };

        let key = ident.as_str().to_string();
        let mod_entry = self.mods.entry(mod_path).or_default();
        match type_key {
            None => {
                mod_entry.free_fns.insert(key, entry);
            }
            Some(tk) => {
                mod_entry
                    .methods
                    .entry(tk)
                    .or_default()
                    .insert(key, entry);
            }
        }

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

    /// Look up the recorded `FnEntry` for a free fn in module `mod_path` with
    /// name `ident`
    pub fn lookup_free_fn(&self, mod_path: &str, ident: &str) -> Option<&FnEntry> {
        self.mods.get(mod_path)?.free_fns.get(ident)
    }

    /// Look up the recorded `FnEntry` for a method in module `mod_path` on
    /// the impl identified by `type_key` with name `ident`
    pub fn lookup_method(
        &self,
        mod_path: &str,
        type_key: &TypeKey,
        ident: &str,
    ) -> Option<&FnEntry> {
        self.mods.get(mod_path)?.methods.get(type_key)?.get(ident)
    }

    /// Set of fn/method names defined in a particular `(mod_path, namespace)`
    /// slot, used by stub generation to choose a non-clashing inner name.
    pub fn known_fn_names_in(&self, mod_path: &str, type_key: Option<&TypeKey>) -> HashSet<String> {
        let Some(mod_entry) = self.mods.get(mod_path) else {
            return HashSet::new();
        };
        match type_key {
            None => mod_entry.free_fns.keys().cloned().collect(),
            Some(tk) => mod_entry
                .methods
                .get(tk)
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default(),
        }
    }
}
