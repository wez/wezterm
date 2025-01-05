//! This crate implements a binary tree with a Zipper based Cursor implementation.
//!
//! For more details on the Zipper concept, check out these resources:
//! * <https://www.st.cs.uni-saarland.de//edu/seminare/2005/advanced-fp/docs/huet-zipper.pdf>
//! * <https://donsbot.wordpress.com/2007/05/17/roll-your-own-window-manager-tracking-focus-with-a-zipper/>
//! * <https://stackoverflow.com/a/36168919/149111>

use std::cmp::PartialEq;
use std::fmt::Debug;

/// Represents a (mostly) "proper" binary tree; each Node has 0 or 2 children,
/// but there is a special case where the tree is rooted with a single leaf node.
/// Non-leaf nodes in the tree can be labelled with an optional node data type `N`,
/// which defaults to `()`.
/// Leaf nodes have a required leaf data type `L`.
pub enum Tree<L, N = ()> {
    Empty,
    Node {
        left: Box<Self>,
        right: Box<Self>,
        data: Option<N>,
    },
    Leaf(L),
}

impl<L, N> PartialEq for Tree<L, N>
where
    L: PartialEq,
    N: PartialEq,
{
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (Self::Empty, Self::Empty) => true,
            (
                Self::Node {
                    left: l_left,
                    right: l_right,
                    data: l_data,
                },
                Self::Node {
                    left: r_left,
                    right: r_right,
                    data: r_data,
                },
            ) => (l_left == r_left) && (l_right == r_right) && (l_data == r_data),
            (Self::Leaf(l), Self::Leaf(r)) => l == r,
            _ => false,
        }
    }
}

impl<L, N> Debug for Tree<L, N>
where
    L: Debug,
    N: Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Empty => fmt.write_str("Empty"),
            Self::Node { left, right, data } => fmt
                .debug_struct("Node")
                .field("left", &left)
                .field("right", &right)
                .field("data", &data)
                .finish(),
            Self::Leaf(l) => fmt.debug_tuple("Leaf").field(&l).finish(),
        }
    }
}

/// Represents a location in the tree for the Zipper; the path contains directions
/// from the current position back towards the root of the tree.
enum Path<L, N> {
    /// The current position is the top of the tree
    Top,
    /// The current position is the left hand side of its parent node;
    /// Cursor::it holds the left node of the tree with the fields here
    /// in Path::Left representing the partially constructed state of
    /// the parent Tree::Node
    Left {
        right: Box<Tree<L, N>>,
        data: Option<N>,
        up: Box<Self>,
    },
    /// The current position is the right hand side of its parent node;
    /// Cursor::it holds the right node of the tree with the fields here
    /// in Path::Right representing the partially constructed state of
    /// the parent Tree::Node
    Right {
        left: Box<Tree<L, N>>,
        data: Option<N>,
        up: Box<Self>,
    },
}

impl<L, N> Debug for Path<L, N>
where
    L: Debug,
    N: Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Top => fmt.write_str("Top"),
            Self::Left { right, data, up } => fmt
                .debug_struct("Left")
                .field("right", &right)
                .field("data", &data)
                .field("up", &up)
                .finish(),
            Self::Right { left, data, up } => fmt
                .debug_struct("Right")
                .field("left", &left)
                .field("data", &data)
                .field("up", &up)
                .finish(),
        }
    }
}

/// The cursor is used to indicate the current position within the tree and enable
/// constant time mutation operations on that position as well as movement around
/// the tree.
/// The cursor isn't a reference to a location within the tree; it is an alternate
/// representation of the tree and thus requires ownership of the tree to create.
/// When you are done using the cursor you may wish to transform it back into
/// a tree.
pub struct Cursor<L, N> {
    it: Box<Tree<L, N>>,
    path: Box<Path<L, N>>,
}

impl<L, N> Debug for Cursor<L, N>
where
    L: Debug,
    N: Debug,
{
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        fmt.debug_struct("Cursor")
            .field("it", &self.it)
            .field("path", &self.path)
            .finish()
    }
}

pub struct ParentIterator<'a, L, N> {
    path: &'a Path<L, N>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathBranch {
    IsLeft,
    IsRight,
}

impl<'a, L, N> std::iter::Iterator for ParentIterator<'a, L, N> {
    type Item = (PathBranch, &'a Option<N>);

    fn next(&mut self) -> Option<Self::Item> {
        match self.path {
            Path::Top => None,
            Path::Left { data, up, .. } => {
                self.path = &*up;
                Some((PathBranch::IsLeft, data))
            }
            Path::Right { data, up, .. } => {
                self.path = &*up;
                Some((PathBranch::IsRight, data))
            }
        }
    }
}

impl<L, N> Tree<L, N> {
    /// Construct a new empty tree
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self::Empty
    }

    /// Returns true if the tree is empty
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Transform the tree into its Zipper based Cursor representation
    pub fn cursor(self) -> Cursor<L, N> {
        Cursor {
            it: Box::new(self),
            path: Box::new(Path::Top),
        }
    }

    pub fn num_leaves(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Leaf(_) => 1,
            Self::Node { left, right, .. } => left.num_leaves() + right.num_leaves(),
        }
    }
}

impl<L, N> Cursor<L, N> {
    /// Construct a cursor representing a new empty tree
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            it: Box::new(Tree::Empty),
            path: Box::new(Path::Top),
        }
    }

    /// References the subtree at the current cursor position
    pub fn subtree(&self) -> &Tree<L, N> {
        &*self.it
    }

    /// Returns true if the current position is a leaf node
    pub fn is_leaf(&self) -> bool {
        matches!(&*self.it, Tree::Leaf(_))
    }

    /// Returns true if the current position is the left child of its parent
    pub fn is_left(&self) -> bool {
        matches!(&*self.path, Path::Left { .. })
    }

    /// Returns true if the current position is the right child of its parent
    pub fn is_right(&self) -> bool {
        matches!(&*self.path, Path::Right { .. })
    }

    pub fn is_top(&self) -> bool {
        matches!(&*self.path, Path::Top)
    }

    /// If the current position is the root of the empty tree,
    /// assign an initial leaf value.
    /// Consumes the cursor and returns a new cursor representing
    /// the mutated tree.
    /// If the current position isn't the top of the empty tree,
    /// yields `Err` containing the unchanged cursor.
    pub fn assign_top(self, leaf: L) -> Result<Self, Self> {
        match (&*self.it, &*self.path) {
            (Tree::Empty, Path::Top) => Ok(Self {
                it: Box::new(Tree::Leaf(leaf)),
                path: self.path,
            }),
            _ => Err(self),
        }
    }

    /// If the current position is a leaf node, return a mutable
    /// reference to the leaf data, else `None`.
    pub fn leaf_mut(&mut self) -> Option<&mut L> {
        match &mut *self.it {
            Tree::Leaf(l) => Some(l),
            _ => None,
        }
    }

    /// If the current position is not a leaf node, return a mutable
    /// reference to the node data container, else yields `Err`.
    #[allow(clippy::result_unit_err)]
    pub fn node_mut(&mut self) -> Result<&mut Option<N>, ()> {
        match &mut *self.it {
            Tree::Node { data, .. } => Ok(data),
            _ => Err(()),
        }
    }

    /// Return an iterator that will visit the chain of nodes leading
    /// to the root from the current position and yield their node
    /// data at each step of iteration.
    pub fn path_to_root(&self) -> ParentIterator<L, N> {
        ParentIterator { path: &*self.path }
    }

    /// If the current position is not a leaf node, assign the
    /// node data to the supplied value.
    /// Consumes the cursor and returns a new cursor representing the
    /// mutated tree.
    /// If the current position is a leaf node then yields `Err`
    /// containing the unchanged cursor.
    pub fn assign_node(mut self, value: Option<N>) -> Result<Self, Self> {
        match &mut *self.it {
            Tree::Node { data, .. } => {
                *data = value;
                Ok(self)
            }
            _ => Err(self),
        }
    }

    /// If the current position is a non-root leaf node, remove it
    /// and unsplit its parent by replacing its parent with either
    /// the opposite branch of the tree from this leaf.
    /// On success, yields the revised cursor, which now points to
    /// the newly unsplit node, along with the leaf value and prior
    /// parent node value.
    /// On failure, yields `Err` containing the unchanged cursor.
    pub fn unsplit_leaf(self) -> Result<(Self, L, Option<N>), Self> {
        if !self.is_leaf() || self.is_top() {
            return Err(self);
        }

        match (*self.it, *self.path) {
            (Tree::Leaf(l), Path::Left { right, data, up }) => Ok((
                Self {
                    it: right,
                    path: up,
                },
                l,
                data,
            )),
            (Tree::Leaf(l), Path::Right { left, data, up }) => {
                Ok((Self { it: left, path: up }, l, data))
            }
            (Tree::Leaf(_), Path::Top) => unreachable!(),
            (Tree::Empty, _) => unreachable!(),
            (Tree::Node { .. }, _) => unreachable!(),
        }
    }

    pub fn split_node_and_insert_left(self, to_insert: L) -> Result<Self, Self> {
        match *self.it {
            Tree::Node { left, right, data } => Ok(Self {
                it: Box::new(Tree::Node {
                    data: None,
                    right: Box::new(Tree::Node { left, right, data }),
                    left: Box::new(Tree::Leaf(to_insert)),
                }),
                path: self.path,
            }),
            _ => Err(self),
        }
    }

    pub fn split_node_and_insert_right(self, to_insert: L) -> Result<Self, Self> {
        match *self.it {
            Tree::Node { left, right, data } => Ok(Self {
                it: Box::new(Tree::Node {
                    data: None,
                    left: Box::new(Tree::Node { left, right, data }),
                    right: Box::new(Tree::Leaf(to_insert)),
                }),
                path: self.path,
            }),
            _ => Err(self),
        }
    }

    /// If the current position is a leaf, split it into a Node where
    /// the left side holds the current leaf value and the right side
    /// holds the provided `right` value.
    /// The cursor position remains unchanged.
    /// Consumes the cursor and returns a new cursor representing the
    /// mutated tree.
    /// If the current position is not a leaf, yields `Err` containing
    /// the unchanged cursor.
    pub fn split_leaf_and_insert_right(self, right: L) -> Result<Self, Self> {
        match *self.it {
            Tree::Leaf(left) => Ok(Self {
                it: Box::new(Tree::Node {
                    data: None,
                    left: Box::new(Tree::Leaf(left)),
                    right: Box::new(Tree::Leaf(right)),
                }),
                path: self.path,
            }),
            _ => Err(self),
        }
    }

    /// If the current position is a leaf, split it into a Node where
    /// the right side holds the current leaf value and the left side
    /// holds the provided `left` value.
    /// The cursor position remains unchanged.
    /// Consumes the cursor and returns a new cursor representing the
    /// mutated tree.
    /// If the current position is not a leaf, yields `Err` containing
    /// the unchanged cursor.
    pub fn split_leaf_and_insert_left(self, left: L) -> Result<Self, Self> {
        match *self.it {
            Tree::Leaf(right) => Ok(Self {
                it: Box::new(Tree::Node {
                    data: None,
                    left: Box::new(Tree::Leaf(left)),
                    right: Box::new(Tree::Leaf(right)),
                }),
                path: self.path,
            }),
            _ => Err(self),
        }
    }

    /// If the current position is not a leaf, move the cursor to
    /// its left child.
    /// Consumes the cursor and returns a new cursor representing the
    /// mutated tree.
    /// If the current position is a Leaf, yields `Err` containing
    /// the unchanged cursor.
    pub fn go_left(self) -> Result<Self, Self> {
        match *self.it {
            Tree::Node { left, right, data } => Ok(Self {
                it: left,
                path: Box::new(Path::Left {
                    data,
                    right,
                    up: self.path,
                }),
            }),
            _ => Err(self),
        }
    }

    /// If the current position is not a leaf, move the cursor to
    /// its right child.
    /// Consumes the cursor and returns a new cursor representing the
    /// mutated tree.
    /// If the current position is a Leaf, yields `Err` containing
    /// the unchanged cursor.
    pub fn go_right(self) -> Result<Self, Self> {
        match *self.it {
            Tree::Node { left, right, data } => Ok(Self {
                it: right,
                path: Box::new(Path::Right {
                    data,
                    left,
                    up: self.path,
                }),
            }),
            _ => Err(self),
        }
    }

    /// If the current position is not at the root of the tree,
    /// move up to the parent of the current position.
    /// Consumes the cursor and returns a new cursor representing the
    /// new location.
    /// If the current position is the top of the tree,
    /// yields `Err` containing the unchanged cursor.
    pub fn go_up(self) -> Result<Self, Self> {
        match *self.path {
            Path::Top => Err(self),
            Path::Right { left, data, up } => Ok(Self {
                it: Box::new(Tree::Node {
                    left,
                    right: self.it,
                    data,
                }),
                path: up,
            }),
            Path::Left { right, data, up } => Ok(Self {
                it: Box::new(Tree::Node {
                    right,
                    left: self.it,
                    data,
                }),
                path: up,
            }),
        }
    }

    /// Move the current position to the next in a preorder traversal.
    /// Returns the modified cursor position.
    ///
    /// In the case where there are no more nodes in the preorder traversal,
    /// yields `Err` with the newly adjusted cursor; calling `preorder_next`
    /// after it has yielded `Err` can potentially yield `Ok` with previously
    /// visited nodes, so the caller must take care to stop iterating when
    /// `Err` is received!
    pub fn preorder_next(mut self) -> Result<Self, Self> {
        // Since we are a "proper" binary tree, we know we cannot have
        // difficult cases such as a left without a right or vice versa.

        if self.is_leaf() {
            if self.is_left() {
                return self.go_up()?.go_right();
            }

            // while (We were on the right)
            loop {
                self = self.go_up()?;

                if self.is_top() {
                    return Err(self);
                }

                if self.is_left() {
                    return self.go_up()?.go_right();
                }
            }
        } else {
            self.go_left()
        }
    }

    /// Move the current position to the next in a postorder traversal.
    /// Returns the modified cursor position.
    ///
    /// In the case where there are no more nodes in the postorder traversal,
    /// yields `Err` with the newly adjusted cursor; calling `postorder_next`
    /// after it has yielded `Err` can potentially yield `Ok` with previously
    /// visited nodes, so the caller must take care to stop iterating when
    /// `Err` is received!
    pub fn postorder_next(mut self) -> Result<Self, Self> {
        // Since we are a "proper" binary tree, we know we cannot have
        // difficult cases such as a left without a right or vice versa.

        if self.is_leaf() {
            if self.is_right() {
                return self.go_up()?.go_left();
            }

            // while (We were on the left)
            loop {
                self = self.go_up()?;

                if self.is_top() {
                    return Err(self);
                }

                if self.is_right() {
                    return self.go_up()?.go_left();
                }
            }
        } else {
            self.go_right()
        }
    }

    /// Move to the nth (preorder) leaf from the current position.
    pub fn go_to_nth_leaf(mut self, n: usize) -> Result<Self, Self> {
        let mut next = 0;
        loop {
            if self.is_leaf() {
                if next == n {
                    return Ok(self);
                }
                next += 1;
            }
            self = self.preorder_next()?;
        }
    }

    /// Consume the cursor and return the root of the Tree
    pub fn tree(mut self) -> Tree<L, N> {
        loop {
            self = match self.go_up() {
                Ok(up) => up,
                Err(top) => return *top.it,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_split_and_iterate() {
        let t: Tree<i32, i32> = Tree::new()
            .cursor()
            .assign_top(1)
            .unwrap()
            .split_leaf_and_insert_right(2)
            .unwrap()
            .tree();

        let t = t
            .cursor()
            .go_to_nth_leaf(1)
            .unwrap()
            .split_leaf_and_insert_right(3)
            .unwrap()
            .tree();

        let mut leaves = vec![];

        let mut cursor = t.cursor();
        loop {
            eprintln!("cursor: {:?}", cursor);
            if cursor.is_leaf() {
                leaves.push(*cursor.leaf_mut().unwrap());
            }
            match cursor.preorder_next() {
                Ok(c) => cursor = c,
                Err(_) => break,
            }
        }

        assert_eq!(leaves, vec![1, 2, 3]);
    }

    #[test]
    fn populate() {
        let t: Tree<i32, i32> = Tree::new()
            .cursor()
            .assign_top(1)
            .unwrap()
            .split_leaf_and_insert_right(2)
            .unwrap()
            .tree();

        assert_eq!(
            t,
            Tree::Node {
                left: Box::new(Tree::Leaf(1)),
                right: Box::new(Tree::Leaf(2)),
                data: None
            }
        );

        let t = t.cursor().assign_node(Some(100)).unwrap().tree();

        assert_eq!(
            t,
            Tree::Node {
                left: Box::new(Tree::Leaf(1)),
                right: Box::new(Tree::Leaf(2)),
                data: Some(100),
            }
        );

        let t = t
            .cursor()
            .go_left()
            .unwrap()
            .split_leaf_and_insert_left(3)
            .unwrap()
            .assign_node(Some(101))
            .unwrap()
            .go_left()
            .unwrap()
            .split_leaf_and_insert_right(4)
            .unwrap()
            .assign_node(Some(102))
            .unwrap()
            .go_left()
            .unwrap()
            .split_leaf_and_insert_right(5)
            .unwrap()
            .assign_node(Some(103))
            .unwrap()
            .tree();

        assert_eq!(
            t,
            Tree::Node {
                left: Box::new(Tree::Node {
                    left: Box::new(Tree::Node {
                        left: Box::new(Tree::Node {
                            left: Box::new(Tree::Leaf(3)),
                            right: Box::new(Tree::Leaf(5)),
                            data: Some(103)
                        }),
                        right: Box::new(Tree::Leaf(4)),
                        data: Some(102)
                    }),
                    right: Box::new(Tree::Leaf(1)),
                    data: Some(101)
                }),
                right: Box::new(Tree::Leaf(2)),
                data: Some(100),
            }
        );

        let mut cursor = t.cursor();
        assert_eq!(100, cursor.node_mut().unwrap().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(101, cursor.node_mut().unwrap().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(102, cursor.node_mut().unwrap().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(103, cursor.node_mut().unwrap().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(3, cursor.leaf_mut().copied().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(5, cursor.leaf_mut().copied().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(4, cursor.leaf_mut().copied().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(1, cursor.leaf_mut().copied().unwrap());

        cursor = cursor.preorder_next().unwrap();
        assert_eq!(2, cursor.leaf_mut().copied().unwrap());

        assert!(cursor.preorder_next().is_err());
    }
}
