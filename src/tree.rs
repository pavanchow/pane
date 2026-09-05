//! The layout tree.
//!
//! A workspace's tiled windows live in a binary tree. Every leaf holds one window.
//! Every internal node is a split with an orientation and a ratio. This is the binary
//! space partition: each split cuts its region into exactly two, so the leaves always
//! form an exact partition of whatever region the root is given.

use crate::geometry::{Rect, SplitDir, WindowId};

/// A single tiled window placement produced by walking the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub id: WindowId,
    /// The full cell owned by this window before any gap is applied. Cells tile the
    /// region exactly.
    pub cell: Rect,
    /// The visible window rectangle, the cell shrunk by the gap.
    pub rect: Rect,
}

/// A node in the layout tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    /// A window at a leaf.
    Leaf(WindowId),
    /// A split of the region into two children.
    Split {
        dir: SplitDir,
        /// Share of the region given to the first child, in `[0.05, 0.95]`.
        ratio: f64,
        a: Box<Node>,
        b: Box<Node>,
    },
}

/// Result of trying to resize a split from a given leaf.
enum ResizeState {
    /// The target leaf is not in this subtree.
    NotHere,
    /// The target leaf is in this subtree but no matching split has been adjusted yet.
    Found,
    /// A split has been adjusted, unwind without further changes.
    Applied,
}

impl Node {
    /// Number of windows in the subtree.
    pub fn leaf_count(&self) -> usize {
        match self {
            Node::Leaf(_) => 1,
            Node::Split { a, b, .. } => a.leaf_count() + b.leaf_count(),
        }
    }

    /// True when `id` is somewhere in the subtree.
    pub fn contains(&self, id: WindowId) -> bool {
        match self {
            Node::Leaf(x) => *x == id,
            Node::Split { a, b, .. } => a.contains(id) || b.contains(id),
        }
    }

    /// Append every window id in the subtree, left to right.
    pub fn window_ids(&self, out: &mut Vec<WindowId>) {
        match self {
            Node::Leaf(x) => out.push(*x),
            Node::Split { a, b, .. } => {
                a.window_ids(out);
                b.window_ids(out);
            }
        }
    }

    /// The first (leftmost) window id in the subtree.
    pub fn first_leaf(&self) -> WindowId {
        match self {
            Node::Leaf(x) => *x,
            Node::Split { a, .. } => a.first_leaf(),
        }
    }

    /// Walk the subtree over `area`, appending a [`Placement`] per window.
    ///
    /// This is the heart of the engine. Each split divides its area exactly in two, so
    /// the produced cells always tile `area` with no overlap and no uncovered space.
    pub fn collect(&self, area: Rect, gap: i64, out: &mut Vec<Placement>) {
        match self {
            Node::Leaf(id) => out.push(Placement {
                id: *id,
                cell: area,
                rect: area.inset(gap),
            }),
            Node::Split { dir, ratio, a, b } => {
                let (ra, rb) = area.split(*dir, *ratio);
                a.collect(ra, gap, out);
                b.collect(rb, gap, out);
            }
        }
    }

    /// Split the leaf holding `target`, adding `new_id` as a sibling.
    ///
    /// The existing window keeps its place and the new window takes the other half.
    /// Returns false when `target` is not a leaf in this subtree.
    pub fn insert(
        &mut self,
        target: WindowId,
        new_id: WindowId,
        dir: SplitDir,
        new_first: bool,
    ) -> bool {
        match self {
            Node::Leaf(id) if *id == target => {
                let existing = Node::Leaf(*id);
                let fresh = Node::Leaf(new_id);
                let (a, b) = if new_first {
                    (fresh, existing)
                } else {
                    (existing, fresh)
                };
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a: Box::new(a),
                    b: Box::new(b),
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => {
                a.insert(target, new_id, dir, new_first)
                    || b.insert(target, new_id, dir, new_first)
            }
        }
    }

    /// Swap the positions of two windows in the tree.
    ///
    /// Returns true when both windows were found and swapped.
    pub fn swap(&mut self, x: WindowId, y: WindowId) -> bool {
        if x == y || !self.contains(x) || !self.contains(y) {
            return false;
        }
        self.swap_inner(x, y);
        true
    }

    fn swap_inner(&mut self, x: WindowId, y: WindowId) {
        match self {
            Node::Leaf(id) => {
                if *id == x {
                    *id = y;
                } else if *id == y {
                    *id = x;
                }
            }
            Node::Split { a, b, .. } => {
                a.swap_inner(x, y);
                b.swap_inner(x, y);
            }
        }
    }

    /// Adjust the ratio of the nearest ancestor split of `id` whose orientation is
    /// `want`. `delta` is added when the leaf is on the first side and subtracted when
    /// it is on the second, so a positive delta always grows the focused window.
    pub fn resize(&mut self, id: WindowId, want: SplitDir, delta: f64) -> bool {
        matches!(self.resize_inner(id, want, delta), ResizeState::Applied)
    }

    fn resize_inner(&mut self, id: WindowId, want: SplitDir, delta: f64) -> ResizeState {
        match self {
            Node::Leaf(x) => {
                if *x == id {
                    ResizeState::Found
                } else {
                    ResizeState::NotHere
                }
            }
            Node::Split { dir, ratio, a, b } => {
                match a.resize_inner(id, want, delta) {
                    ResizeState::Applied => return ResizeState::Applied,
                    ResizeState::Found => {
                        if *dir == want {
                            *ratio = (*ratio + delta).clamp(0.05, 0.95);
                            return ResizeState::Applied;
                        }
                        return ResizeState::Found;
                    }
                    ResizeState::NotHere => {}
                }
                match b.resize_inner(id, want, delta) {
                    ResizeState::Applied => ResizeState::Applied,
                    ResizeState::Found => {
                        if *dir == want {
                            *ratio = (*ratio - delta).clamp(0.05, 0.95);
                            ResizeState::Applied
                        } else {
                            ResizeState::Found
                        }
                    }
                    ResizeState::NotHere => ResizeState::NotHere,
                }
            }
        }
    }

    /// True when every ratio in the tree is inside the legal range.
    pub fn ratios_in_range(&self) -> bool {
        match self {
            Node::Leaf(_) => true,
            Node::Split { ratio, a, b, .. } => {
                (0.05..=0.95).contains(ratio) && a.ratios_in_range() && b.ratios_in_range()
            }
        }
    }
}

/// Remove the window `id` from an optional tree, promoting its sibling into its place.
///
/// Returns the new tree and whether the window was found. When the removed leaf was the
/// whole tree the result is `None`. Because removing a freshly opened window drops the
/// split that created it, close after open restores the exact prior tree.
pub fn remove(node: Option<Node>, id: WindowId) -> (Option<Node>, bool) {
    match node {
        None => (None, false),
        Some(n) => remove_node(n, id),
    }
}

fn remove_node(node: Node, id: WindowId) -> (Option<Node>, bool) {
    match node {
        Node::Leaf(x) => {
            if x == id {
                (None, true)
            } else {
                (Some(Node::Leaf(x)), false)
            }
        }
        Node::Split { dir, ratio, a, b } => {
            let (na, found_a) = remove_node(*a, id);
            if found_a {
                return match na {
                    Some(n) => (
                        Some(Node::Split {
                            dir,
                            ratio,
                            a: Box::new(n),
                            b,
                        }),
                        true,
                    ),
                    None => (Some(*b), true),
                };
            }
            let a = na.expect("sibling A preserved when target not found");
            let (nb, found_b) = remove_node(*b, id);
            if found_b {
                return match nb {
                    Some(n) => (
                        Some(Node::Split {
                            dir,
                            ratio,
                            a: Box::new(a),
                            b: Box::new(n),
                        }),
                        true,
                    ),
                    None => (Some(a), true),
                };
            }
            let b = nb.expect("sibling B preserved when target not found");
            (
                Some(Node::Split {
                    dir,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                false,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: WindowId) -> Node {
        Node::Leaf(id)
    }

    #[test]
    fn insert_splits_the_target_leaf() {
        let mut t = leaf(1);
        assert!(t.insert(1, 2, SplitDir::Vertical, false));
        assert_eq!(t.leaf_count(), 2);
        assert!(t.contains(1) && t.contains(2));
    }

    #[test]
    fn close_after_open_restores_tree() {
        let before = leaf(1);
        let mut after = before.clone();
        after.insert(1, 2, SplitDir::Horizontal, false);
        let (restored, found) = remove(Some(after), 2);
        assert!(found);
        assert_eq!(restored, Some(before));
    }

    #[test]
    fn remove_promotes_sibling() {
        let mut t = leaf(1);
        t.insert(1, 2, SplitDir::Vertical, false);
        t.insert(2, 3, SplitDir::Horizontal, false);
        let (t, found) = remove(Some(t), 1);
        assert!(found);
        let t = t.unwrap();
        assert_eq!(t.leaf_count(), 2);
        assert!(t.contains(2) && t.contains(3) && !t.contains(1));
    }

    #[test]
    fn swap_exchanges_positions() {
        let mut t = leaf(1);
        t.insert(1, 2, SplitDir::Vertical, false);
        let mut ids = vec![];
        t.window_ids(&mut ids);
        assert_eq!(ids, vec![1, 2]);
        assert!(t.swap(1, 2));
        let mut ids = vec![];
        t.window_ids(&mut ids);
        assert_eq!(ids, vec![2, 1]);
    }

    #[test]
    fn resize_grows_focused_window() {
        let mut t = leaf(1);
        t.insert(1, 2, SplitDir::Vertical, false);
        assert!(t.resize(1, SplitDir::Vertical, 0.1));
        if let Node::Split { ratio, .. } = t {
            assert!((ratio - 0.6).abs() < 1e-9);
        } else {
            panic!("expected split");
        }
    }

    #[test]
    fn resize_clamps_to_range() {
        let mut t = leaf(1);
        t.insert(1, 2, SplitDir::Vertical, false);
        for _ in 0..100 {
            t.resize(1, SplitDir::Vertical, 0.5);
        }
        assert!(t.ratios_in_range());
    }
}
