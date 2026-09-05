//! Gate 1: the partition invariant.
//!
//! For any random sequence of operations the tiled window rectangles must exactly tile
//! the screen. With a gap enabled the covered area plus the gap area must equal the
//! screen area exactly. The invariant is asserted after every single operation, not just
//! at the end, so a violation is caught the instant it appears.
//!
//! Bounded for CI. Override with PANE_FUZZ_OPS, PANE_FUZZ_SEED, PANE_FUZZ_RUNS.

mod common;

use common::{env_u64, env_usize, random_op, Rng};
use pane::{Rect, WindowManager};

fn run_one(seed: u64, ops: usize, gap: i64) {
    let screen = Rect::new(0, 0, 1200, 800);
    let mut wm = WindowManager::new(screen, gap);
    let mut rng = Rng::new(seed);

    for step in 0..ops {
        let op = random_op(&mut rng);
        wm.apply(op);

        let report = wm.report();
        assert!(
            report.ok,
            "seed {seed} step {step} op {op:?} gap {gap}: invariant violated: {}",
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

#[test]
fn partition_holds_no_gap() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let base = env_u64("PANE_FUZZ_SEED", 0xA11CE);
    for r in 0..runs as u64 {
        run_one(base.wrapping_add(r.wrapping_mul(0x9E3779B97F4A7C15)), ops, 0);
    }
}

#[test]
fn partition_holds_with_gap() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let base = env_u64("PANE_FUZZ_SEED", 0xB0B);
    for r in 0..runs as u64 {
        run_one(base.wrapping_add(r.wrapping_mul(0x9E3779B97F4A7C15)), ops, 8);
    }
}
