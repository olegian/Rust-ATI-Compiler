//! Analysis state for dynamic abstract type inference.
//!
//! Every type in this file is injected into the instrumented crate by
//! `crate::callbacks::codegen::define_types`.
//!
//! Key points are summarized below.
//!
//! [ATI] is the single global owner of analysis state. The [`ATI_ANALYSIS`] static holds it
//! inside an `Arc<Mutex<..>>` so every instrumented method call can acquire it. It contains
//! the value union-find (which tracks every interaction between tracked values across the
//! whole program) and the collection of sites (which produce the per-site abstract type
//! partition).
//!
//! [Site] is a program point created by the shims emitted by
//! `crate::callbacks::codegen`. Each site records which tagged values were bound
//! to which variable name, and at the end of analysis emits the partition over those
//! variables. [Sites] owns the collection of every program point seen during analysis.
//!
//! [UnionFind] is a basic union-find structure with rank optimization, used both for tracking
//! value interactions globally and for collapsing variable tags within a single site into the
//! abstract type representative.

// FIXME: this file definitely has some dead code somewhere, and can probably be
// refactored to remove some functions.
use crate::ati::tagged::{Id, Tagged, Tagger};

/// Top-level global that owns all information about all value interactions
/// and ATI site states.
pub static ATI_ANALYSIS: std::sync::LazyLock<std::sync::Arc<std::sync::Mutex<ATI>>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(ATI::new())));

/// A program point under analysis. Maps in-scope variables to their tagged values at the
/// start and end of each function.
///
/// Each instrumented entry/exit ppt has a single [Site]. Variables observed at the ppt are
/// collected in `observed_var_tags`, and [`Site::update`] folds them into `var_tags` /
/// `type_uf` to produce the per-variable abstract type representative.
#[derive(Debug)]
pub struct Site {
    /// Per-site union-find used to collapse variable tags into a single abstract-type
    /// representative once the value union-find has had a chance to merge interactions.
    type_uf: UnionFind,
    /// Stable mapping from variable name to its current abstract-type representative.
    /// Populated incrementally by [`Site::update`] from `observed_var_tags`.
    var_tags: std::collections::BTreeMap<String, Id>,
    /// Variables seen at this site since the last [`Site::update`] call. Cleared on update.
    observed_var_tags: std::collections::HashMap<String, Id>,
    /// Human-readable ppt name, used for debug output and `.decls`-format emission.
    name: String,
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

    /// Records that the variable named `var_name` was bound to a tagged value with the given
    /// `id` at this site. Called from generated shims for each in-scope tracked variable.
    pub fn bind(&mut self, var_name: &str, id: Id) {
        self.observed_var_tags.insert(var_name.into(), id);
    }

    /// Folds the variables observed since the last update into the per-site partition.
    ///
    /// For each observed variable, looks up the current leader of its tag in `value_uf` and
    /// merges that leader with whatever leader was previously chosen for the variable. This is
    /// the algorithm from the paper. A variable's abstract type is the union-find class of
    /// every `value_uf` leader it has ever been observed to hold.
    pub fn update(&mut self, value_uf: &mut UnionFind) {
        // for each variable
        for (var, new_tag) in self.observed_var_tags.iter_mut() {
            match self.var_tags.get_mut(var) {
                // we have previously seen this variable at this site,
                // and chosen some previous leader tag (within the value_uf)
                // to be the canonical representation for the abstract type
                // associated with this variable.
                Some(prev_leader) => {
                    let new_leader = value_uf.find(prev_leader).unwrap();
                    if new_leader != *prev_leader {
                        // the leader has changed, merge new leader with old leader in
                        // type_uf, and update the new_tag

                        // make sure type_uf is aware of this new leader,
                        self.type_uf.introduce_tag(new_leader);
                        *prev_leader = self.type_uf.union_tags(prev_leader, &new_leader).unwrap();
                    }

                    let new_tag_leader = value_uf.find(new_tag).unwrap();
                    self.type_uf.introduce_tag(new_tag_leader);
                    *prev_leader = self
                        .type_uf
                        .union_tags(&new_tag_leader, prev_leader)
                        .unwrap();
                }

                // this is the first time we are observing this variable at this site.
                // make the value_uf leader of whatever Id is associated with the current value
                // stored within this variable the canonical abstract type set of this variable.
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
                }
            }
        }
    }

    /// Produces ATI output for this site to stdout. Called at the end of main.
    pub fn report(&mut self) {
        println!("{}", self.name);
        for (var, tag) in self.var_tags.iter() {
            let leader = self.type_uf.find(tag).unwrap();
            println!("{var} -> {leader:?}");
        }
        println!("---");
    }

    /// Emits the variable blocks for this site in `.ati` format.
    pub fn produce_ati(&mut self, output: &mut std::fs::File) {
        use std::io::Write;

        for (var, tag) in self.var_tags.iter() {
            // Do this in the merger. .ati files include all information for all
            // vars. even nested arrays. We will then reconstruct [..] comp information
            // by unioning the ATs of contained values.
            // let Some(var) = collapse_array_indices(var) else {
            //     continue;
            // };
            let var = var.replace('\\', "\\\\").replace(' ', "\\_");

            let leader = self.type_uf.find(tag).unwrap();
            writeln!(output, "var {} {}", var, leader).unwrap();
        }
    }
}

/// Owns the collection of every analyzed site, keyed by ppt name.
pub struct Sites {
    /// Sites currently parked in the collection. A site is removed from the map while a shim
    /// is registering variables to it (via [`Sites::extract`]) and reinserted via
    /// [`Sites::stash`] once that shim finishes.
    locs: std::collections::BTreeMap<String, Site>,
}
impl Sites {
    /// Creates an empty `Sites` collection.
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

    /// Outputs results for all analyzed sites to stdout.
    pub fn report(&mut self) {
        println!("===ATI-ANALYSIS-START===");
        for site in self.locs.values_mut() {
            site.report();
        }
    }

    /// Emits an `.ati` file covering all sites.
    pub fn produce_ati(&mut self, mut output: std::fs::File) {
        use std::io::Write;

        for (name, site) in self.locs.iter_mut() {
            let pt_name = name.replace('\\', "\\\\").replace(' ', "\\_");
            writeln!(output, "ppt {}", pt_name).unwrap();
            site.produce_ati(&mut output);
            writeln!(output).unwrap();
        }
    }
}

/// Basic UnionFind implementation, with light rank optimization.
///
/// Keys are [`Id`]s. Internally the structure stores parents and ranks in `Vec`s indexed by
/// dense `usize`s, with `id_to_index` / `index_to_set` translating between an [`Id`] and its
/// slot. A bundled [`Tagger`] hands out fresh [`Id`]s for [`UnionFind::make_set`].
#[derive(Debug)]
pub struct UnionFind {
    /// Reverse lookup from an externally meaningful [`Id`] to its dense index.
    id_to_index: std::collections::HashMap<Id, usize>,
    /// Dense index back to [`Id`].
    index_to_set: Vec<Id>,
    /// Standard union-find parent array, indexed by dense slot.
    parent: Vec<usize>,
    /// Per-slot rank, used to keep tree depth small during `union`.
    rank: Vec<usize>,
    /// Source of fresh [`Id`]s for [`UnionFind::make_set`].
    tagger: Tagger,
}

impl UnionFind {
    /// Creates an empty UnionFind.
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

    /// Adds the passed in id to the UnionFind, in its own set.
    /// If a set already exists for this Id, does nothing.
    pub fn introduce_tag(&mut self, id: Id) -> Id {
        if self.id_to_index.contains_key(&id) {
            return id;
        }

        let index = self.parent.len();
        self.id_to_index.insert(id, index);
        self.index_to_set.push(id);
        self.parent.push(index);
        self.rank.push(0);

        id
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
        Some(self.index_to_set[leader_index])
    }

    /// Associates the set represented by id1 and id2
    pub fn union_tags(&mut self, id1: &Id, id2: &Id) -> Option<Id> {
        let i1 = self.get_index(id1)?;
        let i2 = self.get_index(id2)?;
        let leader_index = self.union_indices(i1, i2);
        Some(self.index_to_set[leader_index])
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

    /// Associates the indices `x` and `y` together, putting them
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

/// Top-level analysis state. The single live instance is stored in [`ATI_ANALYSIS`].
///
/// Every instrumented operation acquires the surrounding `Mutex<ATI>`, mutates either the
/// global value union-find or the relevant [Site], and releases the lock.
pub struct ATI {
    /// Global value union-find. Every interaction between two tracked values (e.g. a
    /// comparison or an arithmetic op) merges the two operand ids here.
    value_uf: UnionFind,
    /// Collection of program points, keyed by ppt name.
    sites: Sites,
}

impl ATI {
    /// Initializes a new global ATI tracker.
    pub fn new() -> Self {
        Self {
            value_uf: UnionFind::new(),
            sites: Sites::new(),
        }
    }

    /// Moves a value from a standard type `T` to a [`Tagged<T>`],
    /// assigning it a unique Id.
    pub fn track<T>(value: T) -> Tagged<T> {
        let id = ATI_ANALYSIS.lock().unwrap().value_uf.make_set();
        Tagged(id, value)
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

    /// Unions two ids in the global value union-find and returns the new leader.
    pub fn union_and_get_id(&mut self, id1: &Id, id2: &Id) -> Id {
        self.value_uf.union_tags(id1, id2).unwrap()
    }

    /// Allocates a fresh id in the value union-find. Used by operators that produce a result
    /// not directly equivalent to either operand (e.g. comparison, shift).
    pub fn make_id(&mut self) -> Id {
        self.value_uf.make_set()
    }

    /// Observes two tagged values interacting and merges their ids in the value union-find.
    pub fn union_tags<T>(&mut self, tv1: &Tagged<T>, tv2: &Tagged<T>) {
        self.value_uf.union_tags(&tv1.0, &tv2.0);
    }

    /// Produces the output partition that defines abstract types, written to stdout.
    pub fn report(&mut self) {
        self.sites.report();
    }

    /// Writes the analysis result to `output_file` in `.ati` format.
    // FIXME: would be nice to conditionally include either this or what is required for
    // report() to function, no reason to include both in executable every time
    pub fn produce_ati(&mut self, output_file: &str) {
        let cwd = std::env::current_dir().expect("Unable to determine current working directory.");
        let file = cwd.join(output_file);
        let file = std::fs::File::create(file).unwrap();
        self.sites.produce_ati(file)
    }
}
