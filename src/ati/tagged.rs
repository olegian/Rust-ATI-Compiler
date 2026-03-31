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
/// Operator interactions (Add, Sub, Mul, Div, comparisons, etc.) are tracked
/// by the AST transformation pass (TupleLiteralsVisitor::transform_binary_op)
/// rather than through operator overloads on this type.
#[derive(Debug, Clone, Copy)]
pub struct Tagged<T>(pub Id, pub T);

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
