//! Gate 2: tree consistency.
//!
//! After any operation the layout tree stays well formed: no dangling empty containers,
//! every ratio in range, and focus pointing at a window that really exists. Separately,
//! closing a window immediately after opening it returns to the exact prior layout.

mod common;

use common::{env_u64, env_usize, random_op, random_op_capped, Rng};
use pane::manager::Op;
use pane::{Rect, SplitDir, WindowManager};

#[test]
fn tree_stays_well_formed_under_fuzz() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let runs = env_usize("PANE_FUZZ_RUNS", 40);
    let cap = env_usize("PANE_FUZZ_MAX_WINDOWS", 256);
    let base = env_u64("PANE_FUZZ_SEED", 0x00C0_FFEE);
    let screen = Rect::new(0, 0, 1200, 800);

    for r in 0..runs as u64 {
        let seed = base.wrapping_add(r.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut wm = WindowManager::new(screen, 6);
        let mut rng = Rng::new(seed);
        for step in 0..ops {
            let op = random_op_capped(&mut rng, wm.active_window_count(), cap);
            wm.apply(op);
            let errors = wm.consistency_errors();
            assert!(
                errors.is_empty(),
                "seed {seed} step {step} op {op:?}: tree not well formed: {}",
                errors.join("; ")
            );
        }
    }
}

#[test]
fn close_after_open_restores_layout() {
    let screen = Rect::new(0, 0, 1200, 800);
    // Build an arbitrary base layout, snapshot its tiling, open then close, compare.
    let base = env_u64("PANE_FUZZ_SEED", 0xD00D);
    for r in 0..20u64 {
        let seed = base.wrapping_add(r.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut wm = WindowManager::new(screen, 4);
        let mut rng = Rng::new(seed);
        for _ in 0..30 {
            wm.apply(random_op(&mut rng));
        }
        // Ensure there is at least one tiled window to anchor the open.
        if wm.workspace().tree.is_none() {
            wm.apply(Op::Open(SplitDir::Vertical));
        }
        let before: Vec<_> = wm.frame().tiled.iter().map(|p| p.rect).collect();

        wm.apply(Op::Open(SplitDir::Horizontal));
        wm.apply(Op::Close);

        let after: Vec<_> = wm.frame().tiled.iter().map(|p| p.rect).collect();
        assert_eq!(
            before, after,
            "seed {seed}: close after open did not restore the layout"
        );
    }
}
