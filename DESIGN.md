# Pane design

This document explains how Pane is built and why its correctness gates prove what they claim. There are no em dashes and no semicolons in the prose by choice.

## Goals and non goals

Pane is the layout core of a tiling window manager and nothing else. It decides how a set of windows shares a rectangular screen. It does not open real windows, talk to a display server, or read input devices. A real backend would take the rectangles Pane produces and paint them. That separation is the whole point, because it makes the layout algorithm reusable, fast to compile, and provable in isolation.

The design is driven by one property above all others. For any sequence of operations the tiled windows must form an exact partition of the screen. Every structural choice below exists to make that property true and cheap to check.

## Architecture

The engine is a small stack of modules, each with one job.

- `geometry` holds the integer rectangle type and the exact split math.
- `tree` holds the binary space partition layout tree and its edits.
- `workspace` wraps one tree with its floating windows and focus.
- `manager` owns many workspaces over one screen and exposes the operation API.
- `invariant` checks the partition property on a concrete layout.
- `render` turns a layout snapshot into output through a trait, with a text renderer included.

Data flows one way. An `Op` is applied to the `WindowManager`, which mutates the active `Workspace` tree, and then the manager can produce a `Frame` for a `Renderer` or a `PartitionReport` for a test.

## Integers, not floats

Every coordinate is an `i64`. This is a deliberate correctness decision. The partition property is a statement about areas summing exactly to the screen area. With floating point that sum carries rounding error and the invariant can only be checked approximately. With integers the sum is exact, so the claim that the windows cover the screen is a precise equality. Ratios are still expressed as `f64` because a ratio is a preference, not a coordinate, but the moment a ratio becomes a position it is rounded to an integer split point once and both children take one side of that point.

## The layout tree

Tiled windows live in a binary tree. A leaf holds one window. An internal node is a split with an orientation and a ratio and two children. This is the binary space partition. A `Vertical` split places its two children left and right and so divides width. A `Horizontal` split stacks its children top and bottom and so divides height. The ratio is the share given to the first child and is kept inside the range 0.05 to 0.95 so no window can be squeezed to nothing.

Computing rectangles is a single recursive walk. The root receives the screen rectangle. Each split divides its incoming rectangle into two with the exact split math and hands one part to each child. Each leaf records the rectangle it received. Because a split always divides its region into exactly two touching parts that together cover it, the leaf rectangles always cover the root region with no overlap and no gap. The partition property is therefore a structural consequence of the tree, not something layered on top.

## The split and merge algorithm

Opening a window splits the focused leaf. The focused window keeps its place and becomes one child of a new split, and the new window becomes the other child, with the ratio starting at one half. If the tree is empty the new window becomes the whole tree. If focus is on a floating window the split is applied at the first tiled leaf instead. This is the split step.

Closing a window is the inverse merge. The leaf is removed and its sibling is promoted into the parent's place, so the space the pair occupied is handed entirely to the sibling. Because a fresh split created by opening a window has exactly two children, removing the newly opened window promotes its sibling back to where the single leaf used to be and drops the split entirely. That is why closing a window right after opening it restores the exact prior layout, which the gate checks directly.

Resizing walks from the focused leaf up to the nearest ancestor split whose orientation matches the requested axis and nudges that split's ratio, growing the focused window when it is the first child and shrinking it when it is the second, so a grow request always grows the focused window. Moving a window swaps the focused window's id with a neighbour chosen by geometry, which rearranges the layout without changing its shape. Directional focus picks the nearest window whose cell center lies on the correct side of the focused window's center, with the window id breaking ties so the choice is deterministic.

## The partition invariant and gaps

Without gaps the check has three parts. Every cell must lie inside the screen. No two cells may overlap. The cell areas must sum to the screen area. For axis aligned rectangles those three facts together force complete coverage with no holes, because disjoint rectangles that are all inside a container and whose areas already sum to the container area cannot leave any part of it uncovered. That is the whole proof and it is why the check does not need to rasterize anything.

A gap or border is applied by shrinking each cell inward to produce the visible window rectangle. The shrink is capped at half the cell extent so a cell smaller than the gap collapses to a zero size rectangle that still sits inside its cell rather than spilling past its edge. Because every window stays inside its own cell and the cells are disjoint, the windows are disjoint too. The gap area is then computed two independent ways, once as the screen area minus the total window area and once as the sum over cells of the cell area minus its window area, and the two must agree. The visible windows plus the gap area must reconstruct the screen area exactly. This is what the readout means when it prints screen area equals windows plus gaps.

Floating windows are deliberately excluded from the partition. A floating window is lifted out of the tree and given an explicit overlay rectangle. It does not consume tiled space and it is not part of the invariant, which is the documented and intended behavior. Toggling float moves a window between the tree and the floating list, and toggling twice returns it to the tiling.

## Monocle mode

Monocle is a fullscreen zoom of the active workspace. When it is on, the layout walk is bypassed and a single placement is produced whose cell is the whole tiling region, so the one visible window fills the screen. The window shown is the focused one when it is a tiled leaf, otherwise the first tiled leaf. The layout tree is not modified while monocle is on, which is why leaving monocle restores the exact previous tiling with no bookkeeping. The partition invariant still holds because a single cell equal to the screen is a trivial exact partition of it, and the fuzz asserts this after monocle toggles like any other operation. Because there is only one visible cell, directional focus and move fall back to cycling through the windows in tree order rather than choosing by geometry, so both verbs stay useful in monocle mode.

## Workspaces and focus

A workspace is one independent layout tree plus its floating windows and its focused window id. The manager holds a list of workspaces and an active index and switches between them, creating a workspace on demand when a higher index is requested. Because each workspace owns its own tree, building a layout on one workspace and switching away leaves it untouched, and switching back restores it exactly.

Focus is a single window id per workspace and may point at a tiled window or a floating one. After a close the focus falls back to the first tiled window, then to a floating window, then to nothing when the workspace is empty. Every operation keeps focus pointing at a window that really exists, which the consistency gate verifies.

## Why each gate proves its claim

The partition gate applies long random sequences of operations and asserts the invariant after every single operation, not only at the end, so the first operation that could produce a hole or an overlap is caught immediately. It runs both with and without gaps, and with no gap it additionally asserts that the gap area is zero whenever any window is tiled, which is the strongest possible form of full coverage. The random operation generator reaches every verb the engine exposes, including monocle toggles and workspace switches, so no operation escapes the gate. A separate case reruns the same fuzz on adversarial screens, a one pixel square, one dimensional strips, a tiny square smaller than the gap, and a large odd sized screen that never divides evenly, so degenerate cells and cells smaller than the gap are exercised rather than assumed safe.

The partition check is quadratic in the window count, so an unbounded fuzz would spend nearly all its time re-checking enormous layouts and its memory would grow without limit. The generator therefore caps the live window count through `PANE_FUZZ_MAX_WINDOWS`, forcing a close once the cap is reached. This keeps every per operation check cheap and lets the same gate scale from a quick continuous integration run to a stress of ten million operations while still asserting the invariant after every one. The sequences are bounded for continuous integration and can be scaled up through environment variables.

The invariant checker is itself a piece of code that could be wrong, so it is unit tested against layouts that are known to be bad, an overlap, an uncovered gap, a cell that reaches past the screen edge, a window that spills outside its own cell, and cells whose areas do not sum to the screen. Each must be rejected. This closes the loop where a weak checker could rubber stamp a broken layout.

The consistency gate asserts after every operation that no ratio has drifted out of range, that no window id appears twice, and that focus points at a real window, which together mean the tree is well formed and has no dangling empty containers. It also builds an arbitrary layout, snapshots the tiling, opens then closes a window, and asserts the tiling is byte for byte the same, which proves the split and merge steps are true inverses.

The determinism gate runs the same seeded sequence twice through two independent managers and asserts the rendered output is identical, which proves the engine has no hidden nondeterminism such as iteration order over a hash map. A second check confirms that different seeds generally diverge, so the first check is meaningful rather than trivially satisfied by constant output.

Together the three gates cover the claim end to end. The layout is always a valid partition, the tree is always well formed and reversible, and the result is always reproducible.
