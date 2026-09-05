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
/// and the invariant is exercised on non trivial layouts.
pub fn random_op(rng: &mut Rng) -> Op {
    match rng.below(10) {
        0..=3 => Op::Open(if rng.below(2) == 0 {
            SplitDir::Vertical
        } else {
            SplitDir::Horizontal
        }),
        4 => Op::Close,
        5 | 6 => Op::Focus(random_dir(rng)),
        7 => Op::Move(random_dir(rng)),
        8 => Op::Resize(random_dir(rng)),
        _ => Op::Float,
    }
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
