//! Gate 1: the partition invariant.
//!
//! For any random sequence of operations the tiled window rectangles must exactly tile
//! the screen. With a gap enabled the covered area plus the gap area must equal the
//! screen area exactly. The invariant is asserted after every single operation, not just
//! at the end, so a violation is caught the instant it appears.
//!
//! The fuzz caps the live window count so the quadratic partition check stays cheap, which
//! lets it scale to very large operation counts. Bounded for CI. Override with
//! `PANE_FUZZ_OPS`, `PANE_FUZZ_SEED`, `PANE_FUZZ_RUNS`, `PANE_FUZZ_MAX_WINDOWS`.

mod common;

use common::{env_u64, env_usize, random_op_capped, Rng};
use pane::{Rect, WindowManager};

fn run_one(seed: u64, ops: usize, screen: Rect, gap: i64, cap: usize) {
    let mut wm = WindowManager::new(screen, gap);
    let mut rng = Rng::new(seed);

    for step in 0..ops {
        let op = random_op_capped(&mut rng, wm.active_window_count(), cap);
        wm.apply(op);

        let report = wm.report();
        assert!(
            report.ok,
            "seed {seed} step {step} op {op:?} screen {screen:?} gap {gap}: invariant violated: {}",
            report.errors.join("; ")
        );
        assert_eq!(
            report.covered + report.gap_area,
            screen.area(),
            "seed {seed} step {step}: windows plus gaps must equal the screen"
        );
        // With no gap and at least one tiled window, the windows must fill the screen
        // leaving zero uncovered area. An empty tiling (everything closed or floating)
        // legitimately leaves the whole screen uncovered.
        if gap == 0 && !wm.frame().tiled.is_empty() {
            assert_eq!(
                report.gap_area, 0,
                "seed {seed} step {step}: no gap requested so windows must fill the screen"
            );
        }
    }
}

fn cap() -> usize {
    env_usize("PANE_FUZZ_MAX_WINDOWS", 256)
}

#[test]
fn partition_holds_no_gap() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let base = env_u64("PANE_FUZZ_SEED", 0xA11CE);
    let screen = Rect::new(0, 0, 1200, 800);
    for r in 0..runs as u64 {
        run_one(
            base.wrapping_add(r.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
            ops,
            screen,
            0,
            cap(),
        );
    }
}

#[test]
fn partition_holds_with_gap() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let base = env_u64("PANE_FUZZ_SEED", 0xB0B);
    let screen = Rect::new(0, 0, 1200, 800);
    for r in 0..runs as u64 {
        run_one(
            base.wrapping_add(r.wrapping_mul(0x9E37_79B9_7F4A_7C15)),
            ops,
            screen,
            8,
            cap(),
        );
    }
}

/// Adversarial and boundary screens: a one pixel screen, a one dimensional strip, a tiny
/// square smaller than the gap, and a large odd sized screen that never divides evenly.
/// Each is fuzzed with and without a gap. Degenerate cells and cells smaller than the gap
/// must still satisfy the invariant exactly.
#[test]
fn partition_holds_on_extreme_screens() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let base = env_u64("PANE_FUZZ_SEED", 0x5CA1E);
    let screens = [
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 3, 3),
        Rect::new(0, 0, 1, 600),
        Rect::new(0, 0, 600, 1),
        Rect::new(0, 0, 7, 5),
        Rect::new(0, 0, 1001, 997),
    ];
    for (i, screen) in screens.into_iter().enumerate() {
        for gap in [0, 1, 4, 20] {
            for r in 0..runs as u64 {
                let seed = base
                    .wrapping_add((i as u64).wrapping_mul(0x1000))
                    .wrapping_add(u64::try_from(gap).unwrap_or(0).wrapping_mul(0x100))
                    .wrapping_add(r.wrapping_mul(0x9E37_79B9_7F4A_7C15));
                run_one(seed, ops, screen, gap, cap());
            }
        }
    }
}
