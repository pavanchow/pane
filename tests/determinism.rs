//! Gate 3: determinism.
//!
//! The same operation sequence must produce identical window rectangles every run. Two
//! independent managers driven by the same seed must render byte for byte identical
//! output.

mod common;

use common::{env_u64, env_usize, random_op_capped, Rng};
use pane::render::{Renderer, TextRenderer};
use pane::{Rect, WindowManager};

fn render_run(seed: u64, ops: usize) -> String {
    let cap = env_usize("PANE_FUZZ_MAX_WINDOWS", 256);
    let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
    let mut rng = Rng::new(seed);
    let mut renderer = TextRenderer::new();
    for _ in 0..ops {
        wm.apply(random_op_capped(&mut rng, wm.active_window_count(), cap));
        renderer.render(&wm.frame());
    }
    renderer.buffer
}

#[test]
fn same_seed_same_output() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let seed = env_u64("PANE_FUZZ_SEED", 0x5EED);
    let a = render_run(seed, ops);
    let b = render_run(seed, ops);
    assert_eq!(a, b, "identical seeds must produce identical output");
}

#[test]
fn different_seeds_diverge() {
    let ops = env_usize("PANE_FUZZ_OPS", 400);
    let a = render_run(1, ops);
    let b = render_run(2, ops);
    assert_ne!(a, b, "different seeds should generally produce different layouts");
}
