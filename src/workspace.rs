//! A workspace: one independent layout tree plus its floating windows and focus.

use crate::geometry::{Dir, Rect, SplitDir, WindowId};
use crate::tree::{remove, Node, Placement};

/// A window that has been pulled out of the tiling and now overlays it.
///
/// Floating windows are deliberately excluded from the partition invariant. They sit on
/// top of the tiled layout at an explicit rectangle and do not consume tiled space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Floating {
    pub id: WindowId,
    pub rect: Rect,
}

/// One workspace. Holds an optional tiled tree, a list of floating windows, and the id
/// of the focused window (which may be tiled or floating).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Workspace {
    pub tree: Option<Node>,
    pub floating: Vec<Floating>,
    pub focus: Option<WindowId>,
    /// When true only one tiled window is shown, filling the whole tiling region. The
    /// layout tree is preserved untouched so leaving monocle restores the exact tiling.
    pub monocle: bool,
}

impl Workspace {
    /// An empty workspace.
    pub fn new() -> Workspace {
        Workspace::default()
    }

    /// True when the focused window is currently floating.
    pub fn focus_is_floating(&self) -> bool {
        match self.focus {
            Some(f) => self.floating.iter().any(|w| w.id == f),
            None => false,
        }
    }

    /// The tiled placements for this workspace over `screen` with `gap`.
    ///
    /// In the normal case the layout tree is walked so the cells tile the screen exactly.
    /// In monocle mode a single placement is produced: the monocle target window takes the
    /// whole screen as its cell, so the partition invariant still holds with one cell equal
    /// to the screen.
    pub fn tiled(&self, screen: Rect, gap: i64) -> Vec<Placement> {
        let mut out = Vec::new();
        if let Some(t) = &self.tree {
            if self.monocle {
                let id = self.monocle_target(t);
                out.push(Placement {
                    id,
                    cell: screen,
                    rect: screen.inset(gap),
                });
            } else {
                t.collect(screen, gap, &mut out);
            }
        }
        out
    }

    /// The window that monocle mode shows: the focused window when it is a tiled leaf,
    /// otherwise the first tiled leaf.
    fn monocle_target(&self, tree: &Node) -> WindowId {
        match self.focus {
            Some(f) if tree.contains(f) => f,
            _ => tree.first_leaf(),
        }
    }

    /// Toggle monocle (fullscreen zoom) mode. The tree is left intact.
    pub fn toggle_monocle(&mut self) -> bool {
        self.monocle = !self.monocle;
        true
    }

    /// The tiled window ids in left to right tree order.
    fn tree_order(&self) -> Vec<WindowId> {
        let mut ids = Vec::new();
        if let Some(t) = &self.tree {
            t.window_ids(&mut ids);
        }
        ids
    }

    /// Open a new window, splitting the focused tiled leaf in `dir`.
    ///
    /// When the tree is empty the new window becomes the whole tiling. When focus is on
    /// a floating window the split is applied at the first tiled leaf. The new window
    /// takes focus.
    pub fn open(&mut self, new_id: WindowId, dir: SplitDir) {
        match &mut self.tree {
            None => self.tree = Some(Node::Leaf(new_id)),
            Some(t) => {
                let anchor = match self.focus {
                    Some(f) if t.contains(f) => f,
                    _ => t.first_leaf(),
                };
                t.insert(anchor, new_id, dir, false);
            }
        }
        self.focus = Some(new_id);
    }

    /// Close the focused window. Returns the closed id if there was one.
    pub fn close(&mut self) -> Option<WindowId> {
        let target = self.focus?;
        if self.floating.iter().any(|w| w.id == target) {
            self.floating.retain(|w| w.id != target);
        } else {
            let (new_tree, _) = remove(self.tree.take(), target);
            self.tree = new_tree;
        }
        self.focus = self.default_focus();
        Some(target)
    }

    /// Move focus to the nearest window in `dir`, chosen by cell geometry.
    ///
    /// In monocle mode there is only one visible cell, so focus instead cycles through the
    /// tree order: `Right` and `Down` step to the next window, `Left` and `Up` to the
    /// previous, wrapping around.
    pub fn focus_dir(&mut self, dir: Dir, screen: Rect, gap: i64) -> bool {
        if self.monocle {
            return self.cycle_focus(dir);
        }
        let placements = self.tiled(screen, gap);
        let Some(current) = self.focus else {
            return false;
        };
        let Some(from) = placements.iter().find(|p| p.id == current) else {
            return false;
        };
        if let Some(target) = nearest(from.cell, dir, &placements, current) {
            self.focus = Some(target);
            true
        } else {
            false
        }
    }

    /// Swap the focused window with its neighbour in `dir`, keeping focus on the moved
    /// window.
    ///
    /// In monocle mode the neighbour is chosen by tree order rather than geometry, matching
    /// how [`focus_dir`](Self::focus_dir) cycles.
    pub fn move_dir(&mut self, dir: Dir, screen: Rect, gap: i64) -> bool {
        if self.monocle {
            return self.cycle_move(dir);
        }
        let placements = self.tiled(screen, gap);
        let Some(current) = self.focus else {
            return false;
        };
        let Some(from) = placements.iter().find(|p| p.id == current) else {
            return false;
        };
        if let Some(target) = nearest(from.cell, dir, &placements, current) {
            if let Some(t) = &mut self.tree {
                return t.swap(current, target);
            }
        }
        false
    }

    /// Step the focus one window forward or backward in tree order, wrapping around.
    fn cycle_focus(&mut self, dir: Dir) -> bool {
        let order = self.tree_order();
        if order.len() < 2 {
            return false;
        }
        let Some(current) = self.focus else {
            return false;
        };
        let Some(idx) = order.iter().position(|&id| id == current) else {
            return false;
        };
        let n = order.len();
        let next = match dir {
            Dir::Right | Dir::Down => (idx + 1) % n,
            Dir::Left | Dir::Up => (idx + n - 1) % n,
        };
        self.focus = Some(order[next]);
        true
    }

    /// Swap the focused window with the next or previous window in tree order.
    fn cycle_move(&mut self, dir: Dir) -> bool {
        let order = self.tree_order();
        if order.len() < 2 {
            return false;
        }
        let Some(current) = self.focus else {
            return false;
        };
        let Some(idx) = order.iter().position(|&id| id == current) else {
            return false;
        };
        let n = order.len();
        let other = match dir {
            Dir::Right | Dir::Down => order[(idx + 1) % n],
            Dir::Left | Dir::Up => order[(idx + n - 1) % n],
        };
        match &mut self.tree {
            Some(t) => t.swap(current, other),
            None => false,
        }
    }

    /// Resize the focused window along `dir` by `delta`.
    pub fn resize(&mut self, dir: Dir, delta: f64) -> bool {
        let Some(current) = self.focus else {
            return false;
        };
        let Some(t) = &mut self.tree else {
            return false;
        };
        let (want, signed) = match dir {
            Dir::Right => (SplitDir::Vertical, delta),
            Dir::Left => (SplitDir::Vertical, -delta),
            Dir::Down => (SplitDir::Horizontal, delta),
            Dir::Up => (SplitDir::Horizontal, -delta),
        };
        t.resize(current, want, signed)
    }

    /// Toggle the focused window between tiled and floating.
    ///
    /// A newly floated window is given a centered rectangle half the size of the screen.
    /// A window returning to the tiling is inserted at the first tiled leaf.
    pub fn toggle_float(&mut self, screen: Rect) -> bool {
        let Some(current) = self.focus else {
            return false;
        };
        if self.floating.iter().any(|w| w.id == current) {
            self.floating.retain(|w| w.id != current);
            match &mut self.tree {
                None => self.tree = Some(Node::Leaf(current)),
                Some(t) => {
                    let anchor = t.first_leaf();
                    t.insert(anchor, current, SplitDir::Vertical, false);
                }
            }
            true
        } else if self.tree.as_ref().is_some_and(|t| t.contains(current)) {
            let (new_tree, _) = remove(self.tree.take(), current);
            self.tree = new_tree;
            let rect = Rect::new(
                screen.x + screen.w / 4,
                screen.y + screen.h / 4,
                screen.w / 2,
                screen.h / 2,
            );
            self.floating.push(Floating { id: current, rect });
            true
        } else {
            false
        }
    }

    /// Pick a sensible focus target after a close. Prefers a tiled window, then a
    /// floating one, then nothing.
    fn default_focus(&self) -> Option<WindowId> {
        if let Some(t) = &self.tree {
            return Some(t.first_leaf());
        }
        self.floating.first().map(|w| w.id)
    }
}

/// Find the nearest window to `from` in direction `dir`, excluding `current`.
///
/// A candidate qualifies when its center lies on the correct side of `from`'s center.
/// Among candidates the one with the smallest squared distance wins, with the window id
/// breaking ties so the choice is deterministic.
fn nearest(from: Rect, dir: Dir, placements: &[Placement], current: WindowId) -> Option<WindowId> {
    let (fx, fy) = from.center();
    let mut best: Option<(i64, WindowId)> = None;
    for p in placements {
        if p.id == current {
            continue;
        }
        let (cx, cy) = p.cell.center();
        let qualifies = match dir {
            Dir::Left => cx < fx,
            Dir::Right => cx > fx,
            Dir::Up => cy < fy,
            Dir::Down => cy > fy,
        };
        if !qualifies {
            continue;
        }
        let dx = cx - fx;
        let dy = cy - fy;
        let dist = dx * dx + dy * dy;
        match best {
            Some((bd, bid)) if bd < dist || (bd == dist && bid <= p.id) => {}
            _ => best = Some((dist, p.id)),
        }
    }
    best.map(|(_, id)| id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        w: 1000,
        h: 800,
    };

    #[test]
    fn open_grows_the_tree() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        assert_eq!(ws.tiled(SCREEN, 0).len(), 2);
        assert_eq!(ws.focus, Some(2));
    }

    #[test]
    fn close_after_open_restores() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        let before = ws.tree.clone();
        ws.open(2, SplitDir::Horizontal);
        ws.close();
        assert_eq!(ws.tree, before);
    }

    #[test]
    fn focus_moves_right() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        ws.focus = Some(1);
        assert!(ws.focus_dir(Dir::Right, SCREEN, 0));
        assert_eq!(ws.focus, Some(2));
    }

    #[test]
    fn monocle_shows_one_fullscreen_window() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        ws.open(3, SplitDir::Horizontal);
        assert_eq!(ws.tiled(SCREEN, 0).len(), 3);
        ws.toggle_monocle();
        let placements = ws.tiled(SCREEN, 0);
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].cell, SCREEN);
        assert_eq!(placements[0].id, 3);
    }

    #[test]
    fn monocle_preserves_tree_on_exit() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Horizontal);
        let before = ws.tiled(SCREEN, 4);
        ws.toggle_monocle();
        ws.toggle_monocle();
        assert_eq!(ws.tiled(SCREEN, 4), before);
    }

    #[test]
    fn monocle_focus_cycles_tree_order() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        ws.open(3, SplitDir::Vertical);
        ws.focus = Some(1);
        ws.toggle_monocle();
        assert!(ws.focus_dir(Dir::Right, SCREEN, 0));
        assert_eq!(ws.focus, Some(2));
        assert!(ws.focus_dir(Dir::Right, SCREEN, 0));
        assert_eq!(ws.focus, Some(3));
        assert!(ws.focus_dir(Dir::Right, SCREEN, 0));
        assert_eq!(ws.focus, Some(1));
        assert!(ws.focus_dir(Dir::Left, SCREEN, 0));
        assert_eq!(ws.focus, Some(3));
    }

    #[test]
    fn monocle_move_swaps_in_tree_order() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        ws.focus = Some(1);
        ws.toggle_monocle();
        assert!(ws.move_dir(Dir::Right, SCREEN, 0));
        assert_eq!(ws.tree_order(), vec![2, 1]);
        assert_eq!(ws.focus, Some(1));
    }

    #[test]
    fn float_toggle_round_trips() {
        let mut ws = Workspace::new();
        ws.open(1, SplitDir::Vertical);
        ws.open(2, SplitDir::Vertical);
        ws.focus = Some(2);
        assert!(ws.toggle_float(SCREEN));
        assert!(ws.focus_is_floating());
        assert_eq!(ws.tiled(SCREEN, 0).len(), 1);
        assert!(ws.toggle_float(SCREEN));
        assert!(!ws.focus_is_floating());
        assert_eq!(ws.tiled(SCREEN, 0).len(), 2);
    }
}
