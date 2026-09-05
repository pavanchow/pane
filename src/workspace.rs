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
    pub fn tiled(&self, screen: Rect, gap: i64) -> Vec<Placement> {
        let mut out = Vec::new();
        if let Some(t) = &self.tree {
            t.collect(screen, gap, &mut out);
        }
        out
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
    pub fn focus_dir(&mut self, dir: Dir, screen: Rect, gap: i64) -> bool {
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
    pub fn move_dir(&mut self, dir: Dir, screen: Rect, gap: i64) -> bool {
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
