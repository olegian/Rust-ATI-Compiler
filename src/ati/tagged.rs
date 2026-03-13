use crate::ati::ati::{ATI, ATI_ANALYSIS};

pub type Id = u64;

/// Generates incrementing tags of type `Id`, with each call to `tag()`
#[derive(Debug)]
pub struct Tagger {
    next_id: Id,
}

impl Tagger {
    /// Creates a new Tagger
    pub fn new() -> Self {
        Tagger { next_id: 0 }
    }

    /// Fetches the next tag
    pub fn tag(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;

        id
    }
}

/// A tuple of a primative type T, alongside a unique `Id`.
/// This isn't expected to be created directly, but is instead
/// used as a return type from `ATI::track`.
///
/// Further, this struct implements `std::ops::{Add, Sub, Mul, Div}`,
/// alongside Ord, and Eq for less than and comparison,
/// as long as `T` implements each operator. Whenever two tagged values
/// are observed interacting through these operators, global `ATI_ANALYSIS`
/// is updated to record the interaction.
#[derive(Debug, Clone, Copy)]
pub struct Tagged<T>(pub Id, pub T);

impl<T, const N: usize> std::ops::Index<Tagged<usize>> for Tagged<[T; N]> {
    type Output = T;

    fn index(&self, index: Tagged<usize>) -> &Self::Output {
        &self.1[index.1]
    }
}

impl<T, const N: usize> std::ops::IndexMut<Tagged<usize>> for Tagged<[T; N]> {
    fn index_mut(&mut self, index: Tagged<usize>) -> &mut Self::Output {
        &mut self.1[index.1]
    }
}

impl<T> std::ops::Index<Tagged<usize>> for Tagged<&[T]> {
    type Output = T;

    fn index(&self, index: Tagged<usize>) -> &Self::Output {
        &self.1[index.1]
    }
}

impl<T> std::ops::Index<Tagged<usize>> for Tagged<&mut [T]> {
    type Output = T;

    fn index(&self, index: Tagged<usize>) -> &Self::Output {
        &self.1[index.1]
    }
}

impl<T> std::ops::IndexMut<Tagged<usize>> for Tagged<&mut [T]> {
    fn index_mut(&mut self, index: Tagged<usize>) -> &mut Self::Output {
        &mut self.1[index.1]
    }
}

impl<T, const N: usize> Tagged<[T; N]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, N)
    }
}

impl<T> Tagged<&[T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, self.1.len())
    }
}

impl<T> Tagged<&mut [T]> {
    pub fn len(&self) -> Tagged<usize> {
        Tagged(self.0, self.1.len())
    }
}

// for debugging purposes
impl<T> std::fmt::Display for Tagged<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

/// View Tagged docstring.
impl<T> std::ops::Add<Tagged<T>> for Tagged<T>
where
    T: std::ops::Add<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn add(self, rhs: Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 + rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Add<&Tagged<T>> for Tagged<T>
where
    T: std::ops::Add<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn add(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 + rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Add<Tagged<T>> for &Tagged<T>
where
    T: std::ops::Add<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn add(self, rhs: Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 + rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Add<&Tagged<T>> for &Tagged<T>
where
    T: std::ops::Add<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn add(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 + rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Sub<Tagged<T>> for Tagged<T>
where
    T: std::ops::Sub<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn sub(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.1 - rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Sub<&Tagged<T>> for Tagged<T>
where
    T: std::ops::Sub<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn sub(self, rhs: &Self) -> Self::Output {
        let res = ATI::track(self.1 - rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Sub<Tagged<T>> for &Tagged<T>
where
    T: std::ops::Sub<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn sub(self, rhs: Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 - rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Sub<&Tagged<T>> for &Tagged<T>
where
    T: std::ops::Sub<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn sub(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 - rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Mul<Tagged<T>> for Tagged<T>
where
    T: std::ops::Mul<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.1 * rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Mul<&Tagged<T>> for Tagged<T>
where
    T: std::ops::Mul<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn mul(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 * rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Mul<Tagged<T>> for &Tagged<T>
where
    T: std::ops::Mul<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn mul(self, rhs: Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 * rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Mul<&Tagged<T>> for &Tagged<T>
where
    T: std::ops::Mul<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn mul(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 * rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Div<Tagged<T>> for Tagged<T>
where
    T: std::ops::Div<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn div(self, rhs: Self) -> Self::Output {
        let res = ATI::track(self.1 / rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Div<&Tagged<T>> for Tagged<T>
where
    T: std::ops::Div<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn div(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 / rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Div<Tagged<T>> for &Tagged<T>
where
    T: std::ops::Div<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn div(self, rhs: Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 / rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> std::ops::Div<&Tagged<T>> for &Tagged<T>
where
    T: std::ops::Div<Output = T> + Copy,
{
    type Output = Tagged<T>;

    fn div(self, rhs: &Tagged<T>) -> Self::Output {
        let res = ATI::track(self.1 / rhs.1);

        let mut ati = ATI_ANALYSIS.lock().unwrap();
        ati.union_tags(&self, &rhs);
        ati.union_tags(&res, &self);

        res
    }
}

impl<T> PartialEq for Tagged<T>
where
    T: Copy + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, &other);
        self.1 == other.1
    }
}
impl<T> Eq for Tagged<T> where T: Copy + PartialEq {}

impl<T> PartialOrd for Tagged<T>
where
    T: Copy + PartialEq + PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, other);
        match self.1.partial_cmp(&other.1) {
            Some(core::cmp::Ordering::Equal) => Some(core::cmp::Ordering::Equal),
            ord => return ord,
        }
    }
}

impl<T> Ord for Tagged<T>
where
    T: Copy + Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        ATI_ANALYSIS.lock().unwrap().union_tags(&self, other);
        self.1.cmp(&other.1)
    }
}

impl<T> std::hash::Hash for Tagged<T>
where
    T: Copy + std::hash::Hash,
{
    fn hash<H>(&self, hasher: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.1.hash(hasher)
    }
}
