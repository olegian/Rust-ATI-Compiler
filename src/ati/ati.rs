// MARK: tag.rs
use std::ops::{Add, Div, Mul, Sub};
use std::collections::HashMap;
use std::{cell::RefCell, rc::Rc};

pub type ATI = Rc<RefCell<AbstractTypeInference>>;
pub type Id = u64;

/// Generates unique increasing integer IDs for use as tags
pub struct Tagger {
    next_id: Id,
}

impl Tagger {
    pub fn new() -> Self {
        Tagger { next_id: 0 }
    }

    pub fn tag(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        id
    }
}

pub struct TaggedValue<T: Copy> {
    pub value: T,
    pub id: Id,
    pub ati: ATI,
}

impl<T> TaggedValue<T>
where
    T: Copy,
{
    pub fn new(value: T, id: Id, ati: ATI) -> Self {
        Self { value, id, ati }
    }

    pub fn unbind(&self) -> T {
        self.value
    }
}

// restrict T to primative types, but also that doesnt exist lol in rust
impl<'a, T> Add<&'a TaggedValue<T>> for &'a TaggedValue<T>
where
    T: Add<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn add(self, rhs: Self) -> Self::Output {
        self.ati.borrow_mut().union_tags(&self, &rhs);
        let res = AbstractTypeInference::track(self.ati.clone(), self.value + rhs.value);
        self.ati.borrow_mut().union_tags(&res, &self);

        res
    }
}

impl<'a, T> Sub<&'a TaggedValue<T>> for &'a TaggedValue<T>
where
    T: Add<Output = T> + Sub<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        self.ati.borrow_mut().union_tags(&self, &rhs);
        let res = AbstractTypeInference::track(self.ati.clone(), self.value - rhs.value);
        self.ati.borrow_mut().union_tags(&res, &self);

        res
    }
}

impl<'a, T> Mul<&'a TaggedValue<T>> for &'a TaggedValue<T>
where
    T: Mul<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        self.ati.borrow_mut().union_tags(&self, &rhs);
        let res = AbstractTypeInference::track(self.ati.clone(), self.value * rhs.value);
        self.ati.borrow_mut().union_tags(&res, &self);

        res
    }
}

impl<'a, T> Div<&'a TaggedValue<T>> for &'a TaggedValue<T>
where
    T: Div<Output = T> + Copy,
{
    type Output = TaggedValue<T>;

    fn div(self, rhs: Self) -> Self::Output {
        self.ati.borrow_mut().union_tags(&self, &rhs);
        let res = AbstractTypeInference::track(self.ati.clone(), self.value / rhs.value);
        self.ati.borrow_mut().union_tags(&res, &self);

        res
    }
}

impl<T> PartialEq for TaggedValue<T>
where
    T: Copy + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.ati.borrow_mut().union_tags(&self, other);
        self.value == other.value
    }
}
impl<T> Eq for TaggedValue<T> where T: Copy + PartialEq {}

impl<T> PartialOrd for TaggedValue<T>
where
    T: Copy + PartialEq + PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.value.partial_cmp(&other.value) {
            Some(core::cmp::Ordering::Equal) => Some(core::cmp::Ordering::Equal),
            ord => return ord,
        }
    }
}

impl<T> Ord for TaggedValue<T>
where
    T: Copy + PartialEq + PartialOrd,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ati.borrow_mut().union_tags(&self, other);
        self.value.partial_cmp(&other.value).unwrap()
    }
}

// MARK: site.rs

/// A site captures a set of lines in the source code under analysis. A site starts
/// when it is created with `new()`, collects variables under analysis at that site
/// with `observe_var()`, and is then closed with `update()`.
///
/// During execution, when new variables are binded via `let`, the name of the variable, and
/// the tag of the value stored in that variable is loaded into `observed_var_tags`.
/// The tag of the value must match the tag stored in the global interaction set.
///
/// Then, when `update` is called, the observed variables are added into `type_uf`,
/// using the passed `value_uf` state (which tracks which value tags have been
/// placed into the same interaction set, globally) to determine which variables
/// belong to the same abstract types.
///
/// Two variables are considered to have the same abstract type, if there exists some
/// execution path in which the tags of the values binded to those variables have at some point
/// interacted, over the course of the entire programs execution.
///
/// `var_tags` contains the ATI output, mapping the variable identifiers (names) to a value tag,
/// the leader tag of a set of values in `value_uf` which have been observed interacting together.
pub struct Site {
    type_uf: UnionFind,
    var_tags: HashMap<String, Id>,
    observed_var_tags: Vec<(String, Id)>,
    name: String, // Debug information
}

impl Site {
    pub fn new(name: &str) -> Self {
        Site {
            type_uf: UnionFind::new(),
            var_tags: HashMap::new(),
            observed_var_tags: Vec::new(),
            name: name.to_owned(),
        }
    }

    pub fn bind_param<T>(&mut self, var_name: &str, tv: &TaggedValue<T>)
    where
        T: Copy,
    {
        self.observed_var_tags.push((var_name.into(), tv.id));
    }

    /// Registers a new variable pertaining to this analysis site.
    pub fn bind<T>(&mut self, var_name: &str, tv: TaggedValue<T>) -> TaggedValue<T>
    where
        T: Copy,
    {
        self.observed_var_tags.push((var_name.into(), tv.id));
        tv
    }

    /// Algorithm from "Dynamic inference of Abstract Types" by Guo et. al.
    pub fn update(&mut self, value_uf: &mut UnionFind) {
        for (new_var, new_var_tag) in &self.observed_var_tags {
            let new_leader_tag = value_uf.find(new_var_tag).unwrap(); // ? is this unwrap safe? 
            let new_leader_tag = self.type_uf.introduce_tag(new_leader_tag);

            if let Some(old_tag) = self.var_tags.get(new_var) {
                let old_leader_tag = value_uf.find(old_tag).unwrap();

                let merged = self
                    .type_uf
                    .union_tags(&old_leader_tag, &new_leader_tag)
                    .unwrap();
                self.var_tags.insert(new_var.clone(), merged);
            } else {
                self.var_tags.insert(new_var.clone(), new_leader_tag);
            }
        }
    }

    pub fn report(&self) {
        println!("=== {} === ", self.name);
        for (var, tag) in self.var_tags.iter() {
            println!("{var} -> {tag:?}");
        }
        println!("\n");
    }
}

pub struct Sites {
    locs: HashMap<String, Site>,
}
impl Sites {
    pub fn new() -> Self {
        Sites {
            locs: HashMap::new(),
        }
    }

    /// Registers a new site with a given id, or returns
    /// the site with the provided id.
    pub fn extract(&mut self, id: &str) -> Site {
        if !self.locs.contains_key(id) {
            Site::new(id)
        } else {
            self.locs.remove(id).unwrap()
        }
    }

    pub fn stash(&mut self, site: Site) {
        self.locs.insert(site.name.clone(), site);
    }

    pub fn report(&self) {
        for (_, site) in self.locs.iter() {
            site.report();
        }
    }
}

// MARK: union_find.rs

/// Implementation of a UnionFind data structure, in which elements are identified via
/// a unique SetId (which necessarily implements `Eq + Hash + Clone`). This allows
/// SetId to be a String representation of the address of a particular variable,
/// any other identifying information, or even a full struct which stores this identifier
/// alongside whatever useful metadata is helpful for debugging or organizational
/// purposes.
///
/// Each inserted element maintains a 1-1 mapping with it's SetId, passed in when
/// invoking `make_set`. Each element tracks it's parent via the `parent` Vec.
/// When elements are added into the structure, it appends a new element to this
/// Vec. `parent[i]` is the index of the leader element. If `parent[i] == i`,
/// then element `i` is the leader. `index_to_set[i]` returns the SetId (including
/// whatever metadata was associated with it). `find(SetId)` will locate the SetId
/// of the set leader.
///
/// `rank` is used for determining which direction to perform the union, ultimately
/// just the standard optimization done with UnionFind structures.
pub struct UnionFind {
    id_to_index: HashMap<Id, usize>,
    pub index_to_set: Vec<Id>,
    parent: Vec<usize>,
    rank: Vec<usize>,
    tagger: Tagger,
}

impl UnionFind {
    /// Creates a new UnionFind
    pub fn new() -> Self {
        Self {
            id_to_index: HashMap::new(),
            index_to_set: Vec::new(),
            parent: Vec::new(),
            rank: Vec::new(),
            tagger: Tagger::new(),
        }
    }

    /// Creates a new unique element in its own set, to be tracked
    /// within this UnionFind. Duplicate SetIds are disallowed.
    ///
    /// Returns Some(i) if this SetId already corresponds to some set
    /// at parent[i] with rank[i]. Returns None if this operation created
    /// a new set.
    pub fn make_set(&mut self) -> Id {
        let id = self.tagger.tag();
        self.introduce_tag(id)
    }

    /// Similar to make_set, but does not create a new tag out of a variable
    /// just accepts an existing tag as input
    pub fn introduce_tag(&mut self, id: Id) -> Id {
        if self.id_to_index.contains_key(&id) {
            // return Some(*self.id_to_index.get(&id).unwrap());
            return id;
        }

        let index = self.parent.len();
        self.id_to_index.insert(id.clone(), index);
        self.index_to_set.push(id.clone());
        self.parent.push(index);
        self.rank.push(0);

        return id;
    }

    fn get_index(&self, id: &Id) -> Option<usize> {
        self.id_to_index.get(id).copied()
    }

    /// Find the leader SetId which represents the set that
    /// the passed in SetId identifies.
    pub fn find(&mut self, tag: &Id) -> Option<Id> {
        let index = self.get_index(tag)?;
        let leader_index = self.find_index(index);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Merges the sets which the two passed in id's identify.
    /// Returns the leader SetId of the merged set.
    pub fn union_tags(&mut self, t1: &Id, t2: &Id) -> Option<Id> {
        let i1 = self.get_index(t1)?;
        let i2 = self.get_index(t2)?;
        let leader_index = self.union_indices(i1, i2);
        Some(self.index_to_set[leader_index].clone())
    }

    /// Internal find function w/ path compression
    fn find_index(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find_index(self.parent[x]);
        }
        self.parent[x]
    }

    /// Internal union, performing union by rank
    fn union_indices(&mut self, x: usize, y: usize) -> usize {
        let x_root = self.find_index(x);
        let y_root = self.find_index(y);

        if x_root == y_root {
            return x_root;
        }

        // Union towards larger rank
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

// MARK: ati.rs
pub struct AbstractTypeInference {
    value_uf: UnionFind,
    sites: Sites,
}

impl AbstractTypeInference {
    pub fn new() -> Self {
        AbstractTypeInference {
            value_uf: UnionFind::new(),
            sites: Sites::new(),
        }
    }

    // use this function whenever a new literal is created
    pub fn track<T>(
        ati: ATI, // essentially, self.
        value: T, // value of variable
    ) -> TaggedValue<T>
    where
        T: Copy,
    {
        let id = ati.borrow_mut().value_uf.make_set();
        TaggedValue::new(value, id, ati)
    }

    pub fn get_site(&mut self, id: &str) -> Site {
        self.sites.extract(id)
    }

    pub fn update_site(&mut self, mut site: Site) {
        site.update(&mut self.value_uf);
        self.sites.stash(site);
    }

    pub fn union_tags<T>(&mut self, tv1: &TaggedValue<T>, tv2: &TaggedValue<T>)
    where
        T: Copy,
    {
        self.value_uf.union_tags(&tv1.id, &tv2.id);
    }

    pub fn report(&self) {
        self.sites.report();
    }
}
