#![allow(clippy::new_without_default)]
use std::cmp::Ordering;
use std::fmt::Debug;
use std::mem::{replace, swap};

#[derive(Debug)]
pub struct RBTree<K, V>
where
    K: Ord,
{
    pub root: Option<Box<Node<K, V>>>,
    pub size: usize,
}

#[derive(Debug)]
pub struct Node<K, V>
where
    K: Ord,
{
    pub key: K,
    pub value: V,
    pub color: Color,
    pub right: Option<Box<Node<K, V>>>,
    pub left: Option<Box<Node<K, V>>>,
}

#[derive(Debug, PartialEq)]
pub enum Color {
    Red,
    Black,
}

impl Color {
    pub fn is_black<K, V>(node: &Option<Box<Node<K, V>>>) -> bool
    where
        K: Ord,
    {
        node.as_ref()
            .map(|node| node.color == Color::Black)
            .unwrap_or(true)
    }

    pub fn is_red<K, V>(node: &Option<Box<Node<K, V>>>) -> bool
    where
        K: Ord,
    {
        !Color::is_black(node)
    }
}

#[derive(Debug)]
enum Direction {
    Left,
    Right,
}

#[derive(Debug)]
enum InsertionResult<V> {
    OldResult(V),
    RightViolation,
    GrandRightViolation,
    GrandLeftViolation,
    LeftViolation,
    Inserted,
}

pub enum Traversal {
    Preorder,
    Inorder,
    Postorder,
}

impl<K, V> Node<K, V>
where
    K: Ord,
{
    pub fn new(key: K, value: V) -> Self {
        Self {
            key,
            value,
            color: Color::Red,
            left: None,
            right: None,
        }
    }

    fn set_color(&mut self, color: Color) {
        self.color = color;
    }

    // Traverse the tree in `order`, applying the function to each node
    fn for_each(&self, f: &dyn Fn(&Node<K, V>), order: &Traversal) {
        match order {
            Traversal::Preorder => {
                f(self);
                if let Some(left) = &self.left {
                    left.for_each(f, order);
                }
                if let Some(right) = &self.right {
                    right.for_each(f, order);
                }
            }
            Traversal::Inorder => {
                if let Some(left) = &self.left {
                    left.for_each(f, order);
                }
                f(self);
                if let Some(right) = &self.right {
                    right.for_each(f, order);
                }
            }
            Traversal::Postorder => {
                if let Some(left) = &self.left {
                    left.for_each(f, order);
                }
                if let Some(right) = &self.right {
                    right.for_each(f, order);
                }
                f(self);
            }
        }
    }

    fn insert(&mut self, key: K, value: V) -> InsertionResult<V> {
        match self.key.cmp(&key) {
            Ordering::Equal => {
                let old = replace(&mut self.value, value);
                InsertionResult::OldResult(old)
            }
            Ordering::Less => {
                // insert to right...
                if let Some(right) = &mut self.right {
                    // ... and we have something already there
                    let res = right.insert(key, value);

                    // address any potential violations
                    return match res {
                        InsertionResult::RightViolation => {
                            // self is the grandparent, we just inserted to the right parent (red), a right child node (red)
                            return if Color::is_red(&self.left) {
                                // if the uncle node (to the left, is red), we need to recolor
                                // grandparent becomes red, parent and uncle become black
                                self.push_down_blackness();
                                InsertionResult::GrandRightViolation // keep checking for violations from the grandparent
                            } else {
                                // the uncle node is black, and since the inserted node was a right child of the parent
                                // perform a left rotation.
                                self.set_color(Color::Red);
                                self.rotate(Direction::Left);
                                self.set_color(Color::Black);

                                InsertionResult::Inserted
                            };
                        }
                        InsertionResult::LeftViolation => {
                            if Color::is_red(&self.left) {
                                self.push_down_blackness();
                                InsertionResult::GrandRightViolation
                            } else {
                                self.set_color(Color::Red);
                                self.right.as_mut().unwrap().rotate(Direction::Right);
                                self.rotate(Direction::Left);
                                self.set_color(Color::Black);

                                InsertionResult::Inserted
                            }
                        }
                        // we handled a violation by recoloring the lower node red, check for a violation
                        InsertionResult::GrandLeftViolation => {
                            if self.color == Color::Black {
                                InsertionResult::Inserted
                            } else {
                                InsertionResult::RightViolation
                            }
                        }
                        InsertionResult::GrandRightViolation => {
                            if self.color == Color::Black {
                                InsertionResult::Inserted
                            } else {
                                InsertionResult::RightViolation
                            }
                        }
                        // propogate non-violating insertion results
                        InsertionResult::Inserted => InsertionResult::Inserted,
                        InsertionResult::OldResult(v) => InsertionResult::OldResult(v),
                    };
                }

                // ... and nothing to the right! Insert the new red node there
                self.right = Some(Box::new(Node::new(key, value)));

                // but if the current node is also red, we have a violation
                if self.color == Color::Black {
                    InsertionResult::Inserted
                } else {
                    InsertionResult::RightViolation
                }
            }
            Ordering::Greater => {
                // this portion is analougous to above, just swapped with insertion direction
                // need to insert node to the left
                if let Some(left) = &mut self.left {
                    let res = left.insert(key, value);

                    return match res {
                        InsertionResult::RightViolation => {
                            if Color::is_red(&self.right) {
                                self.push_down_blackness();
                                InsertionResult::GrandLeftViolation
                            } else {
                                self.set_color(Color::Red);
                                self.left.as_mut().unwrap().rotate(Direction::Left);
                                self.rotate(Direction::Right);
                                self.set_color(Color::Black);

                                InsertionResult::Inserted
                            }
                        }
                        InsertionResult::LeftViolation => {
                            if Color::is_red(&self.right) {
                                self.push_down_blackness();
                                InsertionResult::GrandLeftViolation
                            } else {
                                self.set_color(Color::Red);
                                self.rotate(Direction::Right);
                                self.set_color(Color::Black);

                                InsertionResult::Inserted
                            }
                        }
                        InsertionResult::GrandLeftViolation => {
                            if self.color == Color::Black {
                                InsertionResult::Inserted
                            } else {
                                InsertionResult::LeftViolation
                            }
                        }
                        InsertionResult::GrandRightViolation => {
                            if self.color == Color::Black {
                                InsertionResult::Inserted
                            } else {
                                InsertionResult::LeftViolation
                            }
                        }
                        InsertionResult::Inserted => InsertionResult::Inserted,
                        InsertionResult::OldResult(v) => InsertionResult::OldResult(v),
                    };
                }

                self.left = Some(Box::new(Node::new(key, value)));
                if self.color == Color::Black {
                    InsertionResult::Inserted
                } else {
                    InsertionResult::LeftViolation
                }
            }
        }
    }

    fn push_down_blackness(&mut self) {
        self.set_color(Color::Red);
        if let Some(left) = self.left.as_mut() {
            left.set_color(Color::Black);
        }
        if let Some(right) = self.right.as_mut() {
            right.set_color(Color::Black);
        }
    }

    fn search(&self, key: &K) -> Option<&V> {
        match self.key.cmp(key) {
            Ordering::Equal => Some(&self.value),
            Ordering::Less => {
                if let Some(right) = &self.right {
                    return right.search(key);
                }
                None
            }
            Ordering::Greater => {
                if let Some(left) = &self.left {
                    return left.search(key);
                }
                None
            }
        }
    }

    fn swap(&mut self, other: &mut Box<Node<K, V>>) {
        swap(&mut self.value, &mut other.value); // swap A contents with B contents
        swap(&mut self.key, &mut other.key); // TODO: probably make a helper function for these swaps
        swap(&mut self.color, &mut other.color);
    }

    // rotate, with self as the pivot
    fn rotate(&mut self, dir: Direction) {
        match dir {
            Direction::Right => {
                /*
                       |                |
                       A(self)          B
                      / \              / \
                     B   T1    -->    T2  A
                    / \                  / \
                   T2  T3               T3  T1
                */
                if self.left.is_none() {
                    return; // can't rotate, but no harm, so just return
                }

                // remember, take's will shift ownership out of the node, and into these variables
                let b = self.left.as_mut().unwrap();
                let t2 = b.left.take();
                let t3 = b.right.take();

                let mut new_a = replace(&mut self.left, t2).unwrap(); // replace A.left with T2, preserve old left, which becomes new_a
                self.swap(&mut new_a); // swap all self contents with new_a (which used to be old_A.left)

                let t1 = self.right.take();
                let new_a_mut = new_a.as_mut();
                new_a_mut.left = t3;
                new_a_mut.right = t1;

                self.right = Some(new_a);
            }
            Direction::Left => {
                // this is analogous to above, it could probably be even implemented with some aliased variables
                if self.right.is_none() {
                    return;
                }

                let b = self.right.as_mut().unwrap();
                let t2 = b.right.take();
                let t3 = b.left.take();

                let mut new_a = replace(&mut self.right, t2).unwrap();
                self.swap(&mut new_a);

                let t1 = self.left.take();
                let new_a_mut = new_a.as_mut();
                new_a_mut.right = t3;
                new_a_mut.left = t1;

                self.left = Some(new_a);
            }
        }
    }
}

impl<K, V> RBTree<K, V>
where
    K: Ord,
{
    pub fn new() -> Self {
        Self {
            root: None,
            size: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    // Returns old value, if one exists
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(root) = &mut self.root {
            // we already have a root
            let res = root.insert(key, value);
            root.set_color(Color::Black);
            return match res {
                InsertionResult::OldResult(v) => Some(v),
                _ => {
                    self.size += 1;
                    None
                }
            };
        }

        // create a new root, degenerate case.
        let mut node = Box::new(Node::new(key, value));
        node.set_color(Color::Black);
        self.root = Some(node);
        self.size += 1;
        None
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        if let Some(root) = &self.root {
            root.search(key)
        } else {
            None
        }
    }

    pub fn for_each(&self, order: Traversal, f: &dyn Fn(&Node<K, V>)) {
        if let Some(root) = &self.root {
            root.for_each(f, &order);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use these tests as examples, but write your own to make sure that your implementation
    // is acting as expected!
    // Be sure to write a test case to validate the shape of your tree after many insertions,
    // remember you can use the iterator to double check that the structure matches what you expect.

    #[test]
    fn create() {
        let tree = RBTree::<i32, String>::new();
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn insert() {
        let mut tree = RBTree::new();

        for i in 0..20 {
            tree.insert(i, format!("value {}", i)); // note that this statement is enough to have rust infer the generic types of RBTree!
        }

        assert_eq!(tree.get(&5), Some(&String::from("value 5")));
        assert_eq!(tree.size(), 20);
    }

    #[test]
    fn test_small_example_tree() {
        let mut tree: RBTree<usize, String> = RBTree::new();

        // always assert size just in case
        assert!(tree.size() == 0);

        tree.insert(0, format!("val{}", 0));
        tree.insert(4, format!("val{}", 4));
        tree.insert(9, format!("val{}", 9));

        check_tree(&tree);
    }

    fn validate_node<K, V>(node: &Node<K, V>) -> u32
    where
        K: Ord,
    {
        // inv.2: red node must have black children
        if node.color == Color::Red {
            assert!(
                Color::is_black(&node.left) && Color::is_black(&node.right),
                "Invariant 2 violation detected"
            )
        }

        let l_black_height = if let Some(left) = &node.left {
            validate_node(left)
        } else {
            1
        };

        let r_black_height = if let Some(right) = &node.right {
            validate_node(right)
        } else {
            1
        };

        // inv.3 violation
        assert_eq!(
            l_black_height, r_black_height,
            "Invariant 3 violation detected."
        );

        if node.color == Color::Black {
            l_black_height + 1
        } else {
            l_black_height
        }
    }

    fn check_tree<K: Ord + Clone + std::fmt::Debug, V: Clone>(tree: &RBTree<K, V>) {
        if let Some(root) = &tree.root {
            assert!(root.color == Color::Black);
            validate_node(root);
        }
    }
}
