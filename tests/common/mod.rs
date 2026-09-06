//! Shared helpers for the integration tests: a dependency free deterministic RNG and a
//! random operation generator.

use pane::manager::Op;
use pane::{Dir, SplitDir};

/// A tiny xorshift64 pseudo random generator. Deterministic given a seed, which is what
/// lets the fuzz and determinism tests be reproducible without any external crate.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng {
            state: seed | 1, // avoid the all zero state
        }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

fn random_dir(rng: &mut Rng) -> Dir {
    match rng.below(4) {
        0 => Dir::Left,
        1 => Dir::Right,
        2 => Dir::Up,
        _ => Dir::Down,
    }
}

/// Produce one random operation. Open is weighted more heavily than close so trees grow
/// and the invariant is exercised on non trivial layouts. Every verb the engine exposes
/// is reachable, including monocle toggles and workspace switches, so the fuzz covers the
/// whole operation surface.
pub fn random_op(rng: &mut Rng) -> Op {
    match rng.below(16) {
        0..=4 => Op::Open(if rng.below(2) == 0 {
            SplitDir::Vertical
        } else {
            SplitDir::Horizontal
        }),
        5 | 6 => Op::Close,
        7..=9 => Op::Focus(random_dir(rng)),
        10 => Op::Move(random_dir(rng)),
        11 => Op::Resize(random_dir(rng)),
        12 => Op::Float,
        13 => Op::Monocle,
        _ => Op::Workspace(usize::try_from(rng.below(3)).unwrap_or(0)),
    }
}

/// Produce one random operation, forcing a close whenever the active workspace already
/// holds `cap` or more windows.
///
/// The partition check is quadratic in the window count, so an unbounded fuzz would spend
/// almost all its time re-checking huge layouts. Capping the live window count keeps every
/// per operation check cheap, which is what lets the stress reach hundreds of thousands of
/// operations while still asserting the invariant after every single one.
pub fn random_op_capped(rng: &mut Rng, live: usize, cap: usize) -> Op {
    if live >= cap {
        return Op::Close;
    }
    random_op(rng)
}

/// Read a positive integer environment variable, falling back to `default`.
pub fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(default)
}

/// Read an integer environment variable, falling back to `default`.
pub fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}
