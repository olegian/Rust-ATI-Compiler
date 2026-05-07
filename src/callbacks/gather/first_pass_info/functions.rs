//! Defines how information about instrumented functions is transfered between the first
//! and second compilation, via the [`FnIndex`] stored within [`FirstPassInfo`].
//!
//! The most important function of this index is to provide the Instrument pass with the
//! `base_ppt_names` used to identify each ppt. At that point, there is no `ldid` available
//! to perform a lookup from the decls file, therefore during the gather compilation we find
//! all functions and identify thier location via the file name and module path. The Insturment
//! pass can then use that to find the appropriate `base_ppt_name`.
//!
//! Futher, there are multiple points where either pass needs to ask "is this function
//! instrumented?", with different HIR/AST information available. The [`FnIndex`] also provides
//! methods to answer that question, in constant time.

use crate::callbacks::gather::type_key::TypeKey;

/// ::-joined module path matching `tcx.def_path_str` format.
/// `""` denotes the crate root.
pub type ModPath = String;

/// A `DeclsFile`-formated `base_ppt_name` which corresponds to some function.
pub type FnBasePptName = String;

/// Identifies whether a fn lookup is a free fn or a method on a type.
#[derive(Debug, Clone, Copy)]
pub enum FnNamespace<'a> {
    Free,
    Method(&'a TypeKey),
}

/// One module's slice of the index. Free fns and methods live in separate
/// maps because their natural key shapes differ, but namespace dispatch is
/// funneled through `slot` / `slot_mut` so `FnIndex`'s public methods don't
/// have to repeat the match.
///
/// Important: using string keys (rather than `Symbol`) so entries are stable across the two
/// compilation sessions.
#[derive(Debug, Default)]
struct ModEntry {
    free_fns: std::collections::HashMap<String, FnBasePptName>,
    methods: std::collections::HashMap<TypeKey, std::collections::HashMap<String, FnBasePptName>>,
}

impl ModEntry {
    // Retreives a mapping of all functions defined within the input namespace.
    fn slot(&self, ns: FnNamespace) -> Option<&std::collections::HashMap<String, FnBasePptName>> {
        match ns {
            FnNamespace::Free => Some(&self.free_fns),
            FnNamespace::Method(tk) => self.methods.get(tk),
        }
    }

    // See [`ModEntry::slot`].
    fn slot_mut(
        &mut self,
        ns: FnNamespace,
    ) -> &mut std::collections::HashMap<String, FnBasePptName> {
        match ns {
            FnNamespace::Free => &mut self.free_fns,
            FnNamespace::Method(tk) => self.methods.entry(tk.clone()).or_default(),
        }
    }
}

/// Registry of every fn/method that the gather pass wants the instrument pass to transform,
/// indexed both by `(mod_path, namespace, ident)` and by `DefId`.
#[derive(Debug, Default)]
pub struct FnIndex {
    mods: std::collections::HashMap<ModPath, ModEntry>,

    /// Side cache of every tracked `DefId`, maintained alongside `mods`.
    /// Pass-1 HIR call analysis hits this when a typeck-resolved
    /// `DefId` is available.
    by_def_id: std::collections::HashSet<rustc_span::def_id::DefId>,
}

impl FnIndex {
    /// Record that `def_id` (with display `ident` and `DeclsFile`-format
    /// `base_ppt_name`) lives at `mod_path` and should be instrumented.
    pub fn record(
        &mut self,
        mod_path: ModPath,
        ns: FnNamespace,
        ident: rustc_span::Ident,
        def_id: rustc_span::def_id::DefId,
        base_ppt_name: String,
    ) {
        self.mods
            .entry(mod_path)
            .or_default()
            .slot_mut(ns)
            .insert(ident.as_str().to_string(), base_ppt_name);
        self.by_def_id.insert(def_id);
    }

    /// Look up the recorded `FnBasePptName` at `(mod_path, ns, ident)`.
    pub fn lookup(&self, mod_path: &str, ns: FnNamespace, ident: &str) -> Option<&FnBasePptName> {
        self.mods.get(mod_path)?.slot(ns)?.get(ident)
    }

    /// Set of fn/method names defined in the `(mod_path, ns)` slot. Used by
    /// stub generation to choose a non-clashing inner name.
    pub fn names_in(&self, mod_path: &str, ns: FnNamespace) -> std::collections::HashSet<String> {
        self.mods
            .get(mod_path)
            .and_then(|m| m.slot(ns))
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether `def_id` was registered as a tracked function.
    pub fn contains(&self, def_id: &rustc_span::def_id::DefId) -> bool {
        self.by_def_id.contains(def_id)
    }
}
