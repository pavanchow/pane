//! Pane is a dependency free tiling window manager engine.
//!
//! It has no display server bindings. Instead it treats the screen as an abstract
//! rectangle and computes where every window goes. That choice makes the whole thing
//! headless and fully testable, and it lets the core correctness property be proven: for
//! any sequence of window operations the tiled windows always form an exact partition of
//! the screen.
//!
//! # Modules
//! - [`geometry`]: integer rectangles and the exact split math.
//! - [`tree`]: the binary space partition layout tree.
//! - [`workspace`]: one layout tree plus floating windows and focus.
//! - [`manager`]: the operation interface over multiple workspaces.
//! - [`invariant`]: the partition invariant checker.
//! - [`render`]: the output abstraction and a text renderer.
//!
//! # Quick start
//! ```
//! use pane::{WindowManager, Op, Rect, SplitDir, Dir};
//!
//! let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
//! wm.apply(Op::Open(SplitDir::Vertical));
//! wm.apply(Op::Open(SplitDir::Horizontal));
//! wm.apply(Op::Focus(Dir::Left));
//!
//! let report = wm.report();
//! assert!(report.ok);
//! assert_eq!(report.covered + report.gap_area, wm.screen().area());
//! ```

pub mod geometry;
pub mod invariant;
pub mod manager;
pub mod render;
pub mod tree;
pub mod workspace;

pub use geometry::{Dir, Rect, SplitDir, WindowId};
pub use invariant::{check, PartitionReport};
pub use manager::{Op, WindowManager, RESIZE_STEP};
pub use render::{Frame, Renderer, TextRenderer};
pub use tree::{Node, Placement};
pub use workspace::{Floating, Workspace};
