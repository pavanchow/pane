//! The partition invariant, the property that proves the engine is correct.
//!
//! Claim: for any tiled layout the window cells form an exact partition of the screen.
//! With gaps enabled the visible windows plus the gap area account for the screen
//! exactly. This module checks that claim on a concrete set of placements and returns a
//! detailed report so tests and the CLI can assert on it.

use crate::geometry::Rect;
use crate::tree::Placement;

/// The outcome of checking the partition invariant against a set of placements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionReport {
    pub screen: Rect,
    /// Sum of the visible window areas (cells shrunk by the gap).
    pub covered: i64,
    /// Screen area not covered by any window, that is the gap area.
    pub gap_area: i64,
    /// Whether every checked condition held.
    pub ok: bool,
    /// One line per violated condition. Empty when `ok`.
    pub errors: Vec<String>,
}

impl PartitionReport {
    /// A short human readable readout, used by the CLI and mirrored by the playground.
    pub fn readout(&self) -> String {
        format!(
            "invariant {} | screen area {} = windows {} + gaps {}",
            if self.ok { "HOLDS" } else { "VIOLATED" },
            self.screen.area(),
            self.covered,
            self.gap_area
        )
    }
}

/// Check the partition invariant for `placements` over `screen`.
///
/// The proof has two layers. First the cells (gap zero regions) must exactly partition
/// the screen: every cell inside the screen, no two cells overlapping, and their areas
/// summing to the screen area. For axis aligned rectangles those three facts together
/// force complete coverage with no gaps. Second, with a gap applied, every window must
/// sit inside its own cell and no two windows may overlap, and the window areas plus the
/// independently accumulated gap area must equal the screen area.
pub fn check(screen: Rect, placements: &[Placement]) -> PartitionReport {
    let mut errors = Vec::new();

    // An empty tiling trivially leaves the whole screen as gap.
    if placements.is_empty() {
        return PartitionReport {
            screen,
            covered: 0,
            gap_area: screen.area(),
            ok: true,
            errors,
        };
    }

    let mut cell_area_sum = 0i64;
    let mut covered = 0i64;
    let mut gap_from_cells = 0i64;

    for p in placements {
        cell_area_sum += p.cell.area();
        covered += p.rect.area();
        gap_from_cells += p.cell.area() - p.rect.area();

        if !p.cell.within(screen) {
            errors.push(format!("cell for window {} escapes the screen", p.id));
        }
        if !p.rect.within(p.cell) {
            errors.push(format!("window {} escapes its cell", p.id));
        }
    }

    // Cells must exactly account for the screen area.
    if cell_area_sum != screen.area() {
        errors.push(format!(
            "cell areas sum to {} but screen area is {}",
            cell_area_sum,
            screen.area()
        ));
    }

    // No two cells may overlap. This plus the area equality proves a perfect partition.
    for i in 0..placements.len() {
        for j in (i + 1)..placements.len() {
            if placements[i].cell.overlaps(placements[j].cell) {
                errors.push(format!(
                    "cells for windows {} and {} overlap",
                    placements[i].id, placements[j].id
                ));
            }
            if placements[i].rect.overlaps(placements[j].rect) {
                errors.push(format!(
                    "windows {} and {} overlap",
                    placements[i].id, placements[j].id
                ));
            }
        }
    }

    // The gap area computed two independent ways must agree, and windows plus gaps must
    // reconstruct the screen exactly.
    let gap_area = screen.area() - covered;
    if gap_area != gap_from_cells {
        errors.push(format!(
            "gap accounting disagrees: screen minus windows is {gap_area} but cell minus window sum is {gap_from_cells}"
        ));
    }
    if covered + gap_area != screen.area() {
        errors.push(format!(
            "windows {} plus gaps {} do not equal screen {}",
            covered,
            gap_area,
            screen.area()
        ));
    }

    PartitionReport {
        screen,
        covered,
        gap_area,
        ok: errors.is_empty(),
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Placement;

    #[test]
    fn empty_layout_is_all_gap() {
        let screen = Rect::new(0, 0, 100, 100);
        let r = check(screen, &[]);
        assert!(r.ok);
        assert_eq!(r.gap_area, 10_000);
    }

    #[test]
    fn perfect_two_way_split_holds() {
        let screen = Rect::new(0, 0, 100, 100);
        let placements = vec![
            Placement {
                id: 1,
                cell: Rect::new(0, 0, 50, 100),
                rect: Rect::new(0, 0, 50, 100),
            },
            Placement {
                id: 2,
                cell: Rect::new(50, 0, 50, 100),
                rect: Rect::new(50, 0, 50, 100),
            },
        ];
        assert!(check(screen, &placements).ok);
    }

    #[test]
    fn overlap_is_caught() {
        let screen = Rect::new(0, 0, 100, 100);
        let placements = vec![
            Placement {
                id: 1,
                cell: Rect::new(0, 0, 60, 100),
                rect: Rect::new(0, 0, 60, 100),
            },
            Placement {
                id: 2,
                cell: Rect::new(50, 0, 50, 100),
                rect: Rect::new(50, 0, 50, 100),
            },
        ];
        assert!(!check(screen, &placements).ok);
    }

    #[test]
    fn uncovered_gap_is_caught() {
        let screen = Rect::new(0, 0, 100, 100);
        let placements = vec![Placement {
            id: 1,
            cell: Rect::new(0, 0, 50, 100),
            rect: Rect::new(0, 0, 50, 100),
        }];
        assert!(!check(screen, &placements).ok);
    }

    #[test]
    fn cell_escaping_the_screen_is_caught() {
        let screen = Rect::new(0, 0, 100, 100);
        // A single cell that reaches past the right edge. Its area happens to equal the
        // screen area, so only the containment check can catch it.
        let placements = vec![Placement {
            id: 1,
            cell: Rect::new(10, 0, 100, 100),
            rect: Rect::new(10, 0, 100, 100),
        }];
        let report = check(screen, &placements);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("escapes the screen")));
    }

    #[test]
    fn window_escaping_its_cell_is_caught() {
        let screen = Rect::new(0, 0, 100, 100);
        let placements = vec![
            Placement {
                id: 1,
                cell: Rect::new(0, 0, 50, 100),
                // The visible rect spills outside the cell it was assigned.
                rect: Rect::new(0, 0, 60, 100),
            },
            Placement {
                id: 2,
                cell: Rect::new(50, 0, 50, 100),
                rect: Rect::new(50, 0, 50, 100),
            },
        ];
        let report = check(screen, &placements);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|e| e.contains("escapes its cell")));
    }

    #[test]
    fn overshooting_cell_area_is_caught() {
        let screen = Rect::new(0, 0, 100, 100);
        // Cells cover more than the screen: the area sum exceeds the screen area.
        let placements = vec![
            Placement {
                id: 1,
                cell: Rect::new(0, 0, 60, 100),
                rect: Rect::new(0, 0, 60, 100),
            },
            Placement {
                id: 2,
                cell: Rect::new(40, 0, 60, 100),
                rect: Rect::new(40, 0, 60, 100),
            },
        ];
        let report = check(screen, &placements);
        assert!(!report.ok);
        // Both the area mismatch and the overlap should be reported.
        assert!(report.errors.iter().any(|e| e.contains("cell areas sum")));
        assert!(report.errors.iter().any(|e| e.contains("overlap")));
    }

    #[test]
    fn single_pixel_screen_holds() {
        let screen = Rect::new(0, 0, 1, 1);
        let placements = vec![Placement {
            id: 1,
            cell: screen,
            rect: screen,
        }];
        assert!(check(screen, &placements).ok);
    }

    #[test]
    fn zero_area_slivers_are_accepted() {
        // A degenerate split can leave a zero width cell. It contributes no area and
        // cannot overlap anything, so the partition still holds exactly.
        let screen = Rect::new(0, 0, 10, 10);
        let placements = vec![
            Placement {
                id: 1,
                cell: Rect::new(0, 0, 10, 10),
                rect: Rect::new(0, 0, 10, 10),
            },
            Placement {
                id: 2,
                cell: Rect::new(10, 0, 0, 10),
                rect: Rect::new(10, 0, 0, 10),
            },
        ];
        assert!(check(screen, &placements).ok);
    }
}
