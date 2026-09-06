//! Output abstraction.
//!
//! The engine never talks to a display server directly. It produces a [`Frame`] and
//! hands it to a [`Renderer`]. A real backend would draw the rectangles on a screen. The
//! bundled [`TextRenderer`] prints them, which is what makes the whole system headless
//! and testable.

use crate::geometry::{Rect, WindowId};
use crate::invariant::PartitionReport;
use crate::tree::Placement;
use crate::workspace::Floating;

/// A complete snapshot of one workspace ready to be drawn.
#[derive(Debug, Clone)]
pub struct Frame {
    pub workspace: usize,
    pub screen: Rect,
    pub gap: i64,
    pub monocle: bool,
    pub tiled: Vec<Placement>,
    pub floating: Vec<Floating>,
    pub focus: Option<WindowId>,
    pub report: PartitionReport,
}

/// Anything that can turn a [`Frame`] into output.
pub trait Renderer {
    fn render(&mut self, frame: &Frame);
}

/// A renderer that writes a plain text description of the frame to a string buffer.
#[derive(Debug, Default)]
pub struct TextRenderer {
    pub buffer: String,
}

impl TextRenderer {
    pub fn new() -> TextRenderer {
        TextRenderer::default()
    }
}

impl Renderer for TextRenderer {
    fn render(&mut self, frame: &Frame) {
        use std::fmt::Write;
        let b = &mut self.buffer;
        let _ = writeln!(
            b,
            "workspace {} screen {}x{} gap {}{}",
            frame.workspace,
            frame.screen.w,
            frame.screen.h,
            frame.gap,
            if frame.monocle { " [monocle]" } else { "" }
        );
        for p in &frame.tiled {
            let focused = frame.focus == Some(p.id);
            let _ = writeln!(
                b,
                "  window {}{} rect x={} y={} w={} h={}",
                p.id,
                if focused { " *" } else { "" },
                p.rect.x,
                p.rect.y,
                p.rect.w,
                p.rect.h
            );
        }
        for f in &frame.floating {
            let focused = frame.focus == Some(f.id);
            let _ = writeln!(
                b,
                "  float  {}{} rect x={} y={} w={} h={}",
                f.id,
                if focused { " *" } else { "" },
                f.rect.x,
                f.rect.y,
                f.rect.w,
                f.rect.h
            );
        }
        let _ = writeln!(b, "  {}", frame.report.readout());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariant;

    #[test]
    fn text_renderer_lists_windows() {
        let screen = Rect::new(0, 0, 100, 100);
        let tiled = vec![Placement {
            id: 1,
            cell: screen,
            rect: screen,
        }];
        let report = invariant::check(screen, &tiled);
        let frame = Frame {
            workspace: 0,
            screen,
            gap: 0,
            monocle: false,
            tiled,
            floating: vec![],
            focus: Some(1),
            report,
        };
        let mut r = TextRenderer::new();
        r.render(&frame);
        assert!(r.buffer.contains("window 1 *"));
        assert!(r.buffer.contains("invariant HOLDS"));
    }
}
