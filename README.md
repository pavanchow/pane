<img src="docs/logo.svg" alt="Pane logo" width="96">

# Pane

A dependency free tiling window manager engine in pure Rust. Zero external crates, edition 2021.

Pane does not bind to a display server. It treats the screen as an abstract rectangle and computes exactly where every window goes. That single choice makes the whole engine headless and fully testable, and it lets the core correctness property be proven rather than hoped for. For any sequence of window operations the tiled windows always form an exact partition of the screen.

Live playground: https://pavanchow.github.io/pane/

## The gap it fills

Most tiling window managers weld their layout logic to X11, Wayland, or a specific OS. The interesting part, the algorithm that decides how windows share the screen, is buried under platform glue and cannot be reused or tested in isolation.

Pane pulls that algorithm out on its own. A person building a real window manager can use it as the layout core and write only a thin backend that draws the rectangles Pane returns. An AI agent that manages panes, viewports, or any rectangular space can drive it through a tiny operation API and trust the partition invariant to keep the space consistent. Because there are no dependencies and no I/O, it compiles fast, runs anywhere Rust runs, and is trivial to embed or audit.

## Quick start

```rust
use pane::{WindowManager, Op, Rect, SplitDir, Dir};

let mut wm = WindowManager::new(Rect::new(0, 0, 1200, 800), 8);
wm.apply(Op::Open(SplitDir::Vertical));
wm.apply(Op::Open(SplitDir::Horizontal));
wm.apply(Op::Focus(Dir::Left));

let report = wm.report();
assert!(report.ok);
assert_eq!(report.covered + report.gap_area, wm.screen().area());

for placement in wm.frame().tiled {
    println!("window {} at {:?}", placement.id, placement.rect);
}
```

## Command line

The `pane` binary applies a scripted sequence of operations and prints the resulting tiling.

```
pane demo
pane --gap 8 'open; open h; focus up; open v; resize right'
pane --screen 1920x1080 --gap 4 'open; open v; open h'
echo 'open; open; open' | pane
```

Operations: `open [h|v]`, `close`, `focus <dir>`, `move <dir>`, `resize <dir>`, `float`, `monocle`, `workspace <n>`, where a direction is `left`, `right`, `up`, or `down`.

## Monocle mode

`monocle` toggles a fullscreen zoom of the active workspace. While it is on, only one tiled window is shown and it fills the whole tiling region. The layout tree is left untouched, so leaving monocle restores the exact previous tiling. In monocle mode `focus` and `move` cycle through the windows in tree order rather than by geometry, since there is only one visible cell. The partition invariant still holds in monocle mode with a single cell equal to the screen.

## API

- `Rect` and `SplitDir` and `Dir` in [`geometry`], the integer geometry and exact split math.
- `Node` in [`tree`], the binary space partition layout tree.
- `Workspace` in [`workspace`], one tree plus floating windows and focus.
- `WindowManager` and `Op` in [`manager`], the operation interface over multiple workspaces.
- `check` and `PartitionReport` in [`invariant`], the partition invariant checker.
- `Renderer` and `TextRenderer` and `Frame` in [`render`], the headless output abstraction.

## The correctness gate

Correctness is proven by tests that run as part of `cargo test`.

1. **Partition invariant.** For any random sequence of operations the tiled window cells exactly tile the screen, that is their union equals the whole screen area with zero overlaps and zero uncovered space. With gaps enabled the covered area plus the gap area equals the screen area exactly. The property is asserted after every single operation. The fuzz exercises every operation the engine exposes, including monocle toggles and workspace switches, and a separate case runs the same fuzz on adversarial screens such as one pixel squares, one dimensional strips, and screens smaller than the gap. See `tests/partition.rs`.
2. **Tree consistency.** After any operation the tree stays well formed, with no dangling empty containers, every ratio in range, and focus pointing at a real window. Closing a window right after opening it returns to the exact prior layout. See `tests/tree_consistency.rs`.
3. **Determinism.** The same operation sequence yields identical rectangles every run. See `tests/determinism.rs`.

The invariant checker itself is unit tested against known bad layouts (overlaps, uncovered gaps, cells that escape the screen, windows that escape their cell, and area mismatches) so the gate cannot silently pass a broken layout.

The fuzz gates are bounded for CI and tunable through environment variables. The partition check is quadratic in the window count, so `PANE_FUZZ_MAX_WINDOWS` caps the live window count and keeps each per operation check cheap. This is what lets the stress reach millions of operations while still asserting the invariant after every one.

```
PANE_FUZZ_OPS=2000 PANE_FUZZ_RUNS=50 PANE_FUZZ_SEED=42 cargo test --release

# heavy stress, ten million operations on the partition gate
PANE_FUZZ_OPS=1000000 PANE_FUZZ_RUNS=10 PANE_FUZZ_MAX_WINDOWS=128 \
    cargo test --release --test partition partition_holds_no_gap
```

## Build and test

```
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
```

## Design

See [DESIGN.md](DESIGN.md) for the architecture, the split and merge algorithm, the partition invariant and gaps, workspaces and focus, and why each gate proves its claim.

[`geometry`]: src/geometry.rs
[`tree`]: src/tree.rs
[`workspace`]: src/workspace.rs
[`manager`]: src/manager.rs
[`invariant`]: src/invariant.rs
[`render`]: src/render.rs
