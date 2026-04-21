/* Defines all types used to perform dynamic ATI. Every type in this file
 * is also defined in the instrumented code by `define_types.rs`.
 *
 * Key points include:
 * 1. `struct ATI` - A single global instance of this struct exists in the program
 *    accessible everywhere within the instrumented files, which holds the value_uf
 *    UnionFind (tracking all value interaction, globally) alongside the actual
 *    abstract type partition at each site. All interactions with ATI instrumentation
 *    are done by calling methods associated with this struct.
 * 2. `struct Site` - A program point, created in stubs, which stores the abstract
 *    types of variables registered to it.
 * 3. `struct Sites` - Maintains a collection of program points, all the sites in the
 *    instrumented file.
 * 4. `struct UnionFind` - A simple union find data structure, with some classic rank
 *    optimization.
 * 5. `trait BindToSite` - A trait which simplifies how values are associated with specific
 *    sites within each function stub. This trait uses an unstable feature called specialization
 *    to allow for more complicated dispatch patterns, based off the "most similar" type.
*/

// FIXME: this file definitiely has some dead code everywhere, and can probably be
// refactored to remove some functions.

use crate::ati::{collection::Collect, index::{TaggedSliceIndex, TaggedSliceable}, tagged::{Id, Tagged, TaggedRef, TaggedRefMut, Tagger}};

/// Top-level global that owns all information about all value interactions
/// and ATI site states.
pub static ATI_ANALYSIS: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<ATI>>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(ATI::new())));

/// Represents a Site under analysis, ultimately a mapping of in-scope
/// variables to thier values at the start and end of each function.
#[derive(Debug)]
pub struct Site {
    type_uf: UnionFind,
    var_tags: std::collections::BTreeMap<String, Id>,
    observed_var_tags: std::collections::HashMap<String, Id>,
    name: String, // Debug information
}

impl Site {
    /// Creates a new empty Site.
    pub fn new(name: &str) -> Self {
        Site {
            type_uf: UnionFind::new(),
            var_tags: std::collections::BTreeMap::new(),
            observed_var_tags: std::collections::HashMap::new(),
            name: name.to_owned(),
        }
    }

    /// Records that a particular `tv: Tagged<T>` was bound to a variable
    /// named `var_name` at this site.
    pub fn bind(&mut self, var_name: &str, id: Id) {
        self.observed_var_tags.insert(var_name.into(), id);
    }

    /// Algorithm from paper, updates ATI information based on observed_vars
    pub fn update(&mut self, value_uf: &mut UnionFind) {
        // for each variable
        for (var, new_tag) in self.observed_var_tags.iter_mut() {
            match self.var_tags.get_mut(var) {
                // we have previously seen this variable at this site, 
                // and chosen some previous leader tag (within the value_uf)
                // to be the canonnical representation for the abstract type 
                // associated with this variable.
                Some(prev_leader) => {
                    let new_leader = value_uf.find(prev_leader).unwrap();
                    if new_leader != *prev_leader {
                        // the leader has changed, merge new leader with old leader in 
                        // type_uf, and update the new_tag 

                        // make sure type_uf is aware of this new leader,
                        self.type_uf.introduce_tag(new_leader);
                        *prev_leader = self.type_uf.union_tags(&prev_leader, &new_leader).unwrap();
                    }

                    let new_tag_leader = value_uf.find(new_tag).unwrap();
                    self.type_uf.introduce_tag(new_tag_leader);
                    *prev_leader = self.type_uf.union_tags(&new_tag_leader, &prev_leader).unwrap();
                },

                // this is the first time we are observing this variable at this site.
                // make the value_uf leader of whatever Id is associated with the current value 
                // stored within this variable the canonnical abstract type set of this variable.
                None => {
                    // find the current leader tag associated with the value's interaction set
                    let leader = value_uf.find(new_tag).unwrap();

                    // make sure that the type_uf is aware that this leader tag is a representative
                    // of a new abstract type set. If this leader already corresponds to some existing
                    // AT set, then this results in a no-op. "The type_uf was already introduced to this
                    // AT set before!"
                    self.type_uf.introduce_tag(leader);
                    let leader = self.type_uf.find(&leader).unwrap();

                    // record that this variable is within the AT set represented by the leader
                    self.var_tags.insert(var.clone(), leader);
                },
            }
        }
    }

    /// Produces ATI output, called at the end of main.
    pub fn report(&mut self) {
        println!("{}", self.name);
        for (var, tag) in self.var_tags.iter() {
            let leader = self.type_uf.find(tag).unwrap();
            println!("{var} -> {leader:?}");
        }
        println!("---");
    }

    /// Emits the variable blocks for this site in .decls format.
    pub fn produce_decls(&mut self, output: &mut std::fs::File) {
        use std::io::Write;

        for (var, tag) in self.var_tags.iter() {
            let Some(var) = collapse_array_indices(var) else {
                continue;
            };
            let var = var.replace('\\', "\\\\").replace(' ', "\\_");
            writeln!(output, "variable {}", var).unwrap();
            writeln!(output, "  comparability {tag}").unwrap();
        }
    }
}

fn collapse_array_indices(name: &str) -> Option<String> {
    if name.ends_with(']') {
        let (base, rest) = name.split_once('[').unwrap();
        // `rest` looks like `0]`, `0][0]`, `3][7]`, etc.
        // Representative iff every bracketed index is `0`
        let is_representative = rest.replace("0]", "").replace('[', "").is_empty();
        return is_representative.then(|| format!("{base}[..]"));
    }

    if name.ends_with("_LEN") {
        if name.contains('[') {
            return None;
        }
        return Some(name.to_string());
    }

    Some(name.to_string())
}

// FIXME: this should really just be a stored value rather than something that is extracted
fn ppt_type_from_name(name: &str) -> &'static str {
    if name.ends_with(":::ENTER") {
        "enter"
    } else if name.ends_with(":::EXIT") {
        "exit"
    } else {
        panic!("unsupported ppt-type in site name: {}", name)
    }
}

/// Manages multiple Sites at once, to allow for analyzing multiple functions
pub struct Sites {
    locs: std::collections::BTreeMap<String, Site>,
}
impl Sites {
    /// Constructor
    pub fn new() -> Self {
        Sites {
            locs: std::collections::BTreeMap::new(),
        }
    }

    /// Pulls a site out of the map, for local modification.
    /// If no site with `id` exists, creates a new one.
    pub fn extract(&mut self, id: &str) -> Site {
        if !self.locs.contains_key(id) {
            Site::new(id)
        } else {
            self.locs.remove(id).unwrap()
        }
    }

    /// Puts a site that was locally modified back into the map.
    pub fn stash(&mut self, site: Site) {
        self.locs.insert(site.name.clone(), site);
    }

    /// Output results for all analyzed sites.
    pub fn report(&mut self) {
        println!("===ATI-ANALYSIS-START===");
        for (_, site) in self.locs.iter_mut() {
            
            site.report();
        }
    }

    /// Emits a .decls file covering all sites.
    pub fn produce_decls(&mut self, mut output: std::fs::File) {
        use std::io::Write;

        for (name, site) in self.locs.iter_mut() {
            let pt_name = name.replace('\\', "\\\\").replace(' ', "\\_");
            writeln!(output, "ppt {}", pt_name).unwrap();
            writeln!(output, "ppt-type {}", ppt_type_from_name(name)).unwrap();
            site.produce_decls(&mut output);
            writeln!(output, "").unwrap();
        }
    }
}

/// Basic UnionFind implementation, with some light rank optimization.
#[derive(Debug)]
pub struct UnionFind {
    id_to_index: std::collections::HashMap<Id, usize>,
    pub index_to_set: Vec<Id>,
    parent: Vec<usize>,
    rank: Vec<usize>,
    tagger: Tagger,
}

impl UnionFind {
    /// Constructor
    pub fn new() -> Self {
        Self {
            id_to_index: std::collections::HashMap::new(),
            index_to_set: Vec::new(),
            parent: Vec::new(),
            rank: Vec::new(),
            tagger: Tagger::new(),
        }
    }

    /// Creates a new set in the union find, returning
    /// an Id that corresponds to it.
    pub fn make_set(&mut self) -> Id {
        let id = self.tagger.tag();
        self.introduce_tag(id)
    }

    /// Adds the passed in id to the UnionFind, in it's own set.
    /// If a set already exists for this Id, does nothing.
    pub fn introduce_tag(&mut self, id: Id) -> Id {
        if self.id_to_index.contains_key(&id) {
            return id;
        }

        let index = self.parent.len();
        self.id_to_index.insert(id.clone(), index);
        self.index_to_set.push(id.clone());
        self.parent.push(index);
        self.rank.push(0);

        return id;
    }

    /// Gets the index in parent associated with this id.
    fn get_index(&self, id: &Id) -> Option<usize> {
        self.id_to_index.get(id).copied()
    }

    /// Finds the parent Id which represents the leader of the set
    /// which contains `id`.
    pub fn find(&mut self, id: &Id) -> Option<Id> {
        let index = self.get_index(id)?;
        let leader_index = self.find_index(index);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Associates the set represented by id1 and id2
    pub fn union_tags(&mut self, id1: &Id, id2: &Id) -> Option<Id> {
        let i1 = self.get_index(id1)?;
        let i2 = self.get_index(id2)?;
        let leader_index = self.union_indices(i1, i2);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Finds the parent index of the set at index `x` of self.parent
    fn find_index(&mut self, mut x: usize) -> usize {
        let mut root = x;
        while self.parent[root] != root {
            root = self.parent[root];
        }

        // path compression
        while self.parent[x] != root {
            let next = self.parent[x];
            self.parent[x] = root;
            x = next;
        }

        root
    }

    /// Associates the indecies `x` and `y` together, putting them
    /// in the same set.
    fn union_indices(&mut self, x: usize, y: usize) -> usize {
        let x_root = self.find_index(x);
        let y_root = self.find_index(y);

        if x_root == y_root {
            return x_root;
        }

        if self.rank[x_root] < self.rank[y_root] {
            self.parent[x_root] = y_root;
            y_root
        } else if self.rank[x_root] > self.rank[y_root] {
            self.parent[y_root] = x_root;
            x_root
        } else {
            self.parent[y_root] = x_root;
            self.rank[x_root] += 1;
            x_root
        }
    }
}

/// This struct owns all necessary information for analysis.
pub struct ATI {
    value_uf: UnionFind,
    sites: Sites,
}

impl ATI {
    /// Intializes a new global ATI tracker.
    pub fn new() -> Self {
        Self {
            value_uf: UnionFind::new(),
            sites: Sites::new(),
        }
    }

    /// Moves a value from a standard type T to a Tagged<T>,
    /// assigning it a unique Id
    pub fn track<T>(value: T) -> Tagged<T>
    where {
        let id = ATI_ANALYSIS.lock().unwrap().value_uf.make_set();
        Tagged(id, value)
    }

    /// Wraps a raw array into a `Tagged<[E; N]>` with a fresh wrapper id, and
    /// unifies tags so all elements at every depth share an AT per depth. The
    /// element type `E` is any `Trackable` — typically `Tagged<U>`, but arrays
    /// of references (e.g. `[&a[..], &b[..], &c[..]]` → `[&Tagged<&[T]>; 3]`)
    /// also satisfy this bound because we specialize `Trackable` for
    /// `&Tagged<..>` and `&mut Tagged<..>` wrappers. Non-tracked elements fall
    /// into the default impl that contributes no ids.
    ///
    /// Specializations walk through nested `Tagged<[..]>` and `Tagged<&[..]>`
    /// elements so arbitrarily-nested arrays end up with: one AT per nesting
    /// depth containing every element at that depth, plus one AT for the new
    /// wrapper.
    pub fn track_array<T: Collect, const N: usize>(array: [T; N]) -> Tagged<[T; N]> {
        let id = ATI_ANALYSIS.lock().unwrap().value_uf.make_set();

        let mut ids_by_level: Vec<Vec<Id>> = Vec::new();
        for i in 0..N {
            array[i].collect_ids_by_level(&mut ids_by_level, 0);
        }

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        for level_ids in ids_by_level.iter() {
            for i in 0..level_ids.len().saturating_sub(1) {
                ati.value_uf.union_tags(&level_ids[i], &level_ids[i + 1]);
            }
        }

        Tagged(id, array)
    }

    /// Borrow a tagged array as a `TaggedRef<[T]>`. Relies on `CoerceUnsized`
    /// to convert `TaggedRef<[T; N]>` -> `TaggedRef<[T]>` at the return site.
    pub fn track_slice<'a, T, const N: usize>(array: &'a Tagged<[T; N]>) -> TaggedRef<'a, [T]> {
        TaggedRef(&array.0, &array.1)
    }

    /// Mutable borrow of a tagged array as a `TaggedRefMut<[T]>`. Splits the
    /// `&mut Tagged<[T; N]>` into separate mutable borrows of the Id and array
    /// fields, then relies on `CoerceUnsized` to unsize the array into a slice.
    pub fn track_slice_mut<'a, T, const N: usize>(
        array: &'a mut Tagged<[T; N]>,
    ) -> TaggedRefMut<'a, [T]> {
        TaggedRefMut(&mut array.0, &mut array.1)
    }

    /// Build a `TaggedRef<[T]>` viewing a subrange of `collection`. The
    /// collection's own Id is reused for the subslice view — the UF merge
    /// unifies the range and collection Id's leader, so any later tag
    /// operations on either borrow see the same AT.
    pub fn track_subslice<'a, T, S, R>(
        collection: &'a S,
        range: R,
    ) -> TaggedRef<'a, [T]>
    where
        S: TaggedSliceable<'a, T> + 'a,
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        let (collection_id, subslice) = collection.raw_subslice(range.into_raw());

        ATI_ANALYSIS.lock().unwrap().union_and_get_id(collection_id, &range_id);
        TaggedRef(collection_id, subslice)
    }

    /// Mutable-borrow counterpart of [`track_subslice`]. Because the Id borrow
    /// is shared with the collection's own Id (see [`track_subslice`]), the
    /// mutable subslice is handed an immutable borrow of the Id — downstream
    /// operations that want to mutate the Id of the overall collection must go
    /// through `ATI_ANALYSIS` rather than this borrow.
    pub fn track_subslice_mut<'a, T, S, R>(
        collection: &'a mut S,
        range: R,
    ) -> TaggedRefMut<'a, [T]>
    where
        S: TaggedSliceable<'a, T> + 'a,
        R: TaggedSliceIndex<T>,
    {
        let range_id = range.id();
        let (collection_id, subslice) = collection.raw_subslice_mut(range.into_raw());

        ATI_ANALYSIS.lock().unwrap().union_and_get_id(collection_id, &range_id);
        TaggedRefMut(collection_id, subslice)
    }

    pub fn track_range<T>(
        start: Tagged<T>,
        end: Tagged<T>,
    ) -> Tagged<std::ops::Range<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.value_uf.make_set();
        ati.value_uf.union_tags(&id, &start.0);
        ati.value_uf.union_tags(&id, &end.0);
        Tagged(id, std::ops::Range { start, end })
    }

    pub fn track_range_inclusive<T>(
        start: Tagged<T>,
        end: Tagged<T>,
    ) -> Tagged<std::ops::RangeInclusive<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.value_uf.make_set();
        ati.value_uf.union_tags(&id, &start.0);
        ati.value_uf.union_tags(&id, &end.0);
        Tagged(id, std::ops::RangeInclusive::new(start, end))
    }

    pub fn track_range_from<T>(
        start: Tagged<T>,
    ) -> Tagged<std::ops::RangeFrom<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.value_uf.make_set();
        ati.value_uf.union_tags(&id, &start.0);
        Tagged(id, std::ops::RangeFrom { start })
    }

    pub fn track_range_to<T>(
        end: Tagged<T>,
    ) -> Tagged<std::ops::RangeTo<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.value_uf.make_set();
        ati.value_uf.union_tags(&id, &end.0);
        Tagged(id, std::ops::RangeTo { end })
    }

    pub fn track_range_to_inclusive<T>(
        end: Tagged<T>,
    ) -> Tagged<std::ops::RangeToInclusive<Tagged<T>>> {
        let mut ati = ATI_ANALYSIS.lock().unwrap();
        let id = ati.value_uf.make_set();
        ati.value_uf.union_tags(&id, &end.0);
        Tagged(id, std::ops::RangeToInclusive { end })
    }

    pub fn track_range_full() -> Tagged<std::ops::RangeFull> {
        let id = ATI_ANALYSIS.lock().unwrap().value_uf.make_set();
        Tagged(id, std::ops::RangeFull)
    }

    /// Fetches a site, or creates it, with the given name.
    pub fn get_site(&mut self, name: &str) -> Site {
        self.sites.extract(name)
    }

    /// Update abstract types at this site, then store it back
    /// into the map. Call whenever you are done registering variables to a site.
    pub fn update_site(&mut self, mut site: Site) {
        site.update(&mut self.value_uf);
        self.sites.stash(site);
    }

    pub fn union_and_get_id(&mut self, id1: &Id, id2: &Id) -> Id {
        self.value_uf.union_tags(id1, id2).unwrap()
    }

    pub fn make_id(&mut self) -> Id {
        self.value_uf.make_set()
    }

    /// Observe two tagged values interacting together, merging them in
    /// value_uf.
    pub fn union_tags<T>(&mut self, tv1: &Tagged<T>, tv2: &Tagged<T>) {
        self.value_uf.union_tags(&tv1.0, &tv2.0);
    }

    /// Produce output partition that defines abstract types.
    pub fn report(&mut self) {
        self.sites.report();
    }

    /// Produce output in .decls format.
    // FIXME: would be nice to conditionally include either this or what is required for report() to function,
    // no reason to include both in executable everytime
    pub fn produce_decls(&mut self, output_file: &str) {
        let cwd = std::env::current_dir().expect("Unable to determine current working directory.");
        let file = cwd.join(output_file);
        let file = std::fs::File::create(file).unwrap();
        self.sites.produce_decls(file)
    }
}
