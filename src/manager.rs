//! The window manager: a set of workspaces over one screen, plus a scriptable
//! operation interface used by the CLI, the fuzz tests, and any real backend.

use crate::geometry::{Dir, Rect, SplitDir, WindowId};
use crate::invariant::{self, PartitionReport};
use crate::render::Frame;
use crate::tree::Node;
use crate::workspace::Workspace;

/// A single operation that can be applied to the manager.
///
/// This enum is the whole public verb set of the engine. Parsing text into `Op`s (see
/// [`Op::parse`]) is how the CLI drives the same code the tests drive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    /// Open a new window, splitting the focused leaf in the given orientation.
    Open(SplitDir),
    /// Close the focused window.
    Close,
    /// Move focus in a direction.
    Focus(Dir),
    /// Swap the focused window with its neighbour in a direction.
    Move(Dir),
    /// Resize the focused window in a direction by the default step.
    Resize(Dir),
    /// Toggle the focused window between tiled and floating.
    Float,
    /// Toggle monocle (fullscreen zoom) mode for the active workspace.
    Monocle,
    /// Switch to a workspace, creating it if needed.
    Workspace(usize),
}

impl Op {
    /// Parse one whitespace separated command token group into an [`Op`].
    ///
    /// Grammar, one op per token group: `open [h|v]`, `close`, `focus <dir>`,
    /// `move <dir>`, `resize <dir>`, `float`, `monocle`, `workspace <n>`. Directions are
    /// `left|right|up|down`.
    ///
    /// # Errors
    /// Returns an error string naming the offending token when the command word is
    /// unknown, a direction is missing or invalid, or a workspace number will not parse.
    pub fn parse(tokens: &[&str]) -> Result<Op, String> {
        let (head, rest) = tokens
            .split_first()
            .ok_or_else(|| "empty command".to_string())?;
        match head.to_ascii_lowercase().as_str() {
            "open" | "o" => {
                let dir = match rest.first().map(|s| s.to_ascii_lowercase()) {
                    None => SplitDir::Vertical,
                    Some(ref s) if s == "v" || s == "vertical" => SplitDir::Vertical,
                    Some(ref s) if s == "h" || s == "horizontal" => SplitDir::Horizontal,
                    Some(s) => return Err(format!("unknown split direction '{s}'")),
                };
                Ok(Op::Open(dir))
            }
            "close" | "c" => Ok(Op::Close),
            "focus" | "f" => Ok(Op::Focus(parse_dir(rest.first())?)),
            "move" | "m" => Ok(Op::Move(parse_dir(rest.first())?)),
            "resize" | "r" => Ok(Op::Resize(parse_dir(rest.first())?)),
            "float" => Ok(Op::Float),
            "monocle" | "mono" => Ok(Op::Monocle),
            "workspace" | "ws" => {
                let n = rest
                    .first()
                    .ok_or_else(|| "workspace needs a number".to_string())?
                    .parse::<usize>()
                    .map_err(|_| "workspace number must be a non negative integer".to_string())?;
                Ok(Op::Workspace(n))
            }
            other => Err(format!("unknown command '{other}'")),
        }
    }
}

fn parse_dir(tok: Option<&&str>) -> Result<Dir, String> {
    match tok.map(|s| s.to_ascii_lowercase()) {
        Some(ref s) if s == "left" || s == "l" => Ok(Dir::Left),
        Some(ref s) if s == "right" || s == "r" => Ok(Dir::Right),
        Some(ref s) if s == "up" || s == "u" => Ok(Dir::Up),
        Some(ref s) if s == "down" || s == "d" => Ok(Dir::Down),
        Some(s) => Err(format!("unknown direction '{s}'")),
        None => Err("direction required".to_string()),
    }
}

/// The default fraction a single resize step adds or removes from a split ratio.
pub const RESIZE_STEP: f64 = 0.05;

/// The window manager. Owns the workspaces, the active index, the screen, and the gap.
#[derive(Debug, Clone)]
pub struct WindowManager {
    workspaces: Vec<Workspace>,
    active: usize,
    screen: Rect,
    gap: i64,
    next_id: WindowId,
}

impl WindowManager {
    /// Create a manager for `screen` with `gap` and a single empty workspace.
    pub fn new(screen: Rect, gap: i64) -> WindowManager {
        WindowManager {
            workspaces: vec![Workspace::new()],
            active: 0,
            screen,
            gap: gap.max(0),
            next_id: 1,
        }
    }

    /// The active workspace index.
    pub fn active(&self) -> usize {
        self.active
    }

    /// The number of workspaces that currently exist.
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// The screen rectangle.
    pub fn screen(&self) -> Rect {
        self.screen
    }

    /// The configured gap.
    pub fn gap(&self) -> i64 {
        self.gap
    }

    /// The id of the focused window in the active workspace.
    pub fn focus(&self) -> Option<WindowId> {
        self.workspaces[self.active].focus
    }

    /// True when the active workspace is in monocle mode.
    pub fn monocle(&self) -> bool {
        self.workspaces[self.active].monocle
    }

    /// The number of windows, tiled plus floating, in the active workspace. Useful for
    /// bounding stress runs so the layout does not grow without limit.
    pub fn active_window_count(&self) -> usize {
        let ws = &self.workspaces[self.active];
        let tiled = ws.tree.as_ref().map_or(0, Node::leaf_count);
        tiled + ws.floating.len()
    }

    fn ws(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active]
    }

    /// Apply one operation to the active workspace.
    pub fn apply(&mut self, op: Op) {
        let screen = self.screen;
        let gap = self.gap;
        match op {
            Op::Open(dir) => {
                let id = self.next_id;
                self.next_id += 1;
                self.ws().open(id, dir);
            }
            Op::Close => {
                self.ws().close();
            }
            Op::Focus(dir) => {
                self.ws().focus_dir(dir, screen, gap);
            }
            Op::Move(dir) => {
                self.ws().move_dir(dir, screen, gap);
            }
            Op::Resize(dir) => {
                self.ws().resize(dir, RESIZE_STEP);
            }
            Op::Float => {
                self.ws().toggle_float(screen);
            }
            Op::Monocle => {
                self.ws().toggle_monocle();
            }
            Op::Workspace(n) => {
                while self.workspaces.len() <= n {
                    self.workspaces.push(Workspace::new());
                }
                self.active = n;
            }
        }
    }

    /// A borrow of the active workspace, for inspection and testing.
    pub fn workspace(&self) -> &Workspace {
        &self.workspaces[self.active]
    }

    /// Check that the active workspace is well formed and return a line per problem.
    ///
    /// Well formed means every split ratio is in range, focus points at a window that
    /// really exists (or is `None` only when the workspace is empty), and no window id
    /// appears twice. Empty containers cannot occur by construction: a split always owns
    /// two real children and a leaf always owns a window.
    pub fn consistency_errors(&self) -> Vec<String> {
        let ws = &self.workspaces[self.active];
        let mut errors = Vec::new();

        let mut tiled_ids = Vec::new();
        if let Some(t) = &ws.tree {
            if !t.ratios_in_range() {
                errors.push("a split ratio is out of range".to_string());
            }
            t.window_ids(&mut tiled_ids);
        }
        let mut all_ids = tiled_ids.clone();
        all_ids.extend(ws.floating.iter().map(|f| f.id));

        let mut seen = all_ids.clone();
        seen.sort_unstable();
        seen.dedup();
        if seen.len() != all_ids.len() {
            errors.push("a window id appears more than once".to_string());
        }

        match ws.focus {
            None => {
                if !all_ids.is_empty() {
                    errors.push("focus is None but windows exist".to_string());
                }
            }
            Some(f) => {
                if !all_ids.contains(&f) {
                    errors.push(format!("focus points at missing window {f}"));
                }
            }
        }

        errors
    }

    /// The partition report for the active workspace's tiled windows.
    pub fn report(&self) -> PartitionReport {
        let tiled = self.workspaces[self.active].tiled(self.screen, self.gap);
        invariant::check(self.screen, &tiled)
    }

    /// Build a render [`Frame`] for the active workspace.
    pub fn frame(&self) -> Frame {
        let ws = &self.workspaces[self.active];
        let tiled = ws.tiled(self.screen, self.gap);
        let report = invariant::check(self.screen, &tiled);
        Frame {
            workspace: self.active,
            screen: self.screen,
            gap: self.gap,
            monocle: ws.monocle,
            tiled,
            floating: ws.floating.clone(),
            focus: ws.focus,
            report,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> WindowManager {
        WindowManager::new(Rect::new(0, 0, 1200, 800), 0)
    }

    #[test]
    fn parse_round_trip() {
        assert_eq!(Op::parse(&["open"]).unwrap(), Op::Open(SplitDir::Vertical));
        assert_eq!(Op::parse(&["open", "h"]).unwrap(), Op::Open(SplitDir::Horizontal));
        assert_eq!(Op::parse(&["focus", "left"]).unwrap(), Op::Focus(Dir::Left));
        assert_eq!(Op::parse(&["ws", "2"]).unwrap(), Op::Workspace(2));
        assert_eq!(Op::parse(&["monocle"]).unwrap(), Op::Monocle);
        assert_eq!(Op::parse(&["mono"]).unwrap(), Op::Monocle);
        assert!(Op::parse(&["bogus"]).is_err());
        assert!(Op::parse(&["focus"]).is_err());
    }

    #[test]
    fn apply_open_and_report_holds() {
        let mut wm = manager();
        wm.apply(Op::Open(SplitDir::Vertical));
        wm.apply(Op::Open(SplitDir::Horizontal));
        wm.apply(Op::Open(SplitDir::Vertical));
        assert!(wm.report().ok);
        assert_eq!(wm.frame().tiled.len(), 3);
    }

    #[test]
    fn workspaces_are_independent() {
        let mut wm = manager();
        wm.apply(Op::Open(SplitDir::Vertical));
        wm.apply(Op::Workspace(1));
        assert_eq!(wm.frame().tiled.len(), 0);
        wm.apply(Op::Open(SplitDir::Vertical));
        wm.apply(Op::Open(SplitDir::Vertical));
        assert_eq!(wm.frame().tiled.len(), 2);
        wm.apply(Op::Workspace(0));
        assert_eq!(wm.frame().tiled.len(), 1);
    }

    #[test]
    fn monocle_keeps_the_invariant() {
        let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
        for _ in 0..6 {
            wm.apply(Op::Open(SplitDir::Vertical));
        }
        wm.apply(Op::Monocle);
        assert!(wm.monocle());
        let frame = wm.frame();
        assert_eq!(frame.tiled.len(), 1);
        assert!(frame.report.ok);
        assert_eq!(frame.report.covered + frame.report.gap_area, wm.screen().area());
        wm.apply(Op::Monocle);
        assert!(!wm.monocle());
        assert_eq!(wm.frame().tiled.len(), 6);
        assert!(wm.report().ok);
    }

    #[test]
    fn deep_nesting_holds_the_invariant() {
        let mut wm = WindowManager::new(Rect::new(0, 0, 4096, 4096), 3);
        // Always split the focused window so the tree grows one level deeper each step,
        // producing a long spine of nested splits rather than a balanced tree.
        for i in 0..2000 {
            let dir = if i % 2 == 0 {
                SplitDir::Vertical
            } else {
                SplitDir::Horizontal
            };
            wm.apply(Op::Open(dir));
            let report = wm.report();
            assert!(report.ok, "deep nesting violated at step {i}: {}", report.errors.join("; "));
        }
        assert_eq!(wm.active_window_count(), 2000);
    }

    #[test]
    fn tiny_screen_never_panics_or_violates() {
        let mut wm = WindowManager::new(Rect::new(0, 0, 2, 2), 5);
        for _ in 0..50 {
            wm.apply(Op::Open(SplitDir::Vertical));
            wm.apply(Op::Open(SplitDir::Horizontal));
            assert!(wm.report().ok);
            assert!(wm.consistency_errors().is_empty());
        }
    }

    #[test]
    fn gap_is_accounted_exactly() {
        let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
        for _ in 0..5 {
            wm.apply(Op::Open(SplitDir::Vertical));
        }
        let r = wm.report();
        assert!(r.ok);
        assert!(r.gap_area > 0);
        assert_eq!(r.covered + r.gap_area, wm.screen().area());
    }
}
