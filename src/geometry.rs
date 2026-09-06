//! Integer geometry primitives.
//!
//! Every coordinate is an `i64`. Using integers instead of floating point is what
//! lets the partition invariant be asserted exactly: a sum of integer areas has no
//! rounding error, so "these rectangles cover the screen" is a precise equality
//! rather than an approximate one.

/// A window identifier. Assigned by a monotonic counter so layouts are deterministic.
pub type WindowId = u64;

/// The orientation of a split node.
///
/// `Vertical` places its two children side by side (a vertical divider line) and so
/// divides the parent's width. `Horizontal` stacks its children top over bottom (a
/// horizontal divider line) and so divides the parent's height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDir {
    /// Children are left and right, width is divided.
    Vertical,
    /// Children are top and bottom, height is divided.
    Horizontal,
}

impl SplitDir {
    /// The opposite orientation, used when alternating splits.
    #[must_use]
    pub fn flip(self) -> SplitDir {
        match self {
            SplitDir::Vertical => SplitDir::Horizontal,
            SplitDir::Horizontal => SplitDir::Vertical,
        }
    }
}

/// A compass direction for focus movement and window moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// An axis aligned rectangle with integer coordinates.
///
/// `x`, `y` are the top left corner. `w`, `h` are width and height and are always
/// non negative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
}

impl Rect {
    /// Construct a rectangle, clamping width and height to be non negative.
    pub fn new(x: i64, y: i64, w: i64, h: i64) -> Rect {
        Rect {
            x,
            y,
            w: w.max(0),
            h: h.max(0),
        }
    }

    /// The area of the rectangle. Never negative.
    pub fn area(self) -> i64 {
        self.w * self.h
    }

    /// The x coordinate just past the right edge.
    pub fn right(self) -> i64 {
        self.x + self.w
    }

    /// The y coordinate just past the bottom edge.
    pub fn bottom(self) -> i64 {
        self.y + self.h
    }

    /// The integer center point, used for directional navigation.
    pub fn center(self) -> (i64, i64) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    /// True when this rectangle sits entirely inside `outer`.
    pub fn within(self, outer: Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }

    /// True when the two rectangles share any area. Touching edges do not count.
    pub fn overlaps(self, other: Rect) -> bool {
        let x_overlap = self.x < other.right() && other.x < self.right();
        let y_overlap = self.y < other.bottom() && other.y < self.bottom();
        x_overlap && y_overlap && self.area() > 0 && other.area() > 0
    }

    /// Shrink the rectangle inward by `gap` on every side.
    ///
    /// This is how a gap or border is applied to a tiled cell. The result always stays
    /// inside the original rectangle, so a set of non overlapping cells produces a set
    /// of non overlapping windows.
    #[must_use]
    pub fn inset(self, gap: i64) -> Rect {
        if gap <= 0 {
            return self;
        }
        // Cap the inset at half the extent so a cell smaller than the gap collapses to a
        // zero size rectangle that still sits inside the cell rather than spilling past
        // its far edge.
        let gx = gap.min(self.w / 2);
        let gy = gap.min(self.h / 2);
        Rect::new(self.x + gx, self.y + gy, self.w - 2 * gx, self.h - 2 * gy)
    }

    /// Divide the rectangle into two child rectangles along `dir` at `ratio`.
    ///
    /// The two results always cover the parent exactly with no overlap and no gap: the
    /// split point is computed once and each child takes one side of it, so the child
    /// extents sum to the parent extent with zero rounding loss. `ratio` is the share
    /// given to the first (left or top) child.
    pub fn split(self, dir: SplitDir, ratio: f64) -> (Rect, Rect) {
        match dir {
            SplitDir::Vertical => {
                let lw = split_point(self.w, ratio);
                let left = Rect::new(self.x, self.y, lw, self.h);
                let right = Rect::new(self.x + lw, self.y, self.w - lw, self.h);
                (left, right)
            }
            SplitDir::Horizontal => {
                let th = split_point(self.h, ratio);
                let top = Rect::new(self.x, self.y, self.w, th);
                let bottom = Rect::new(self.x, self.y + th, self.w, self.h - th);
                (top, bottom)
            }
        }
    }
}

/// Pick the integer split offset for an extent of `len` at `ratio`.
///
/// When `len` is at least two the offset is clamped to `[1, len - 1]` so both children
/// keep positive size. When `len` is smaller there is no way to give both children
/// positive size, so the first child takes everything and the second takes zero. Either
/// way the two extents sum to `len` exactly.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
fn split_point(len: i64, ratio: f64) -> i64 {
    if len < 2 {
        return len.max(0);
    }
    // `len` is a pixel extent that comfortably fits in an f64 mantissa, and the product is
    // clamped to `[1, len - 1]` right after, so neither the widening nor the truncation can
    // move the result outside the legal range.
    let raw = (len as f64 * ratio).round() as i64;
    raw.clamp(1, len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_covers_parent_exactly_vertical() {
        let r = Rect::new(0, 0, 100, 50);
        let (a, b) = r.split(SplitDir::Vertical, 0.5);
        assert_eq!(a.area() + b.area(), r.area());
        assert!(!a.overlaps(b));
        assert_eq!(a.right(), b.x);
    }

    #[test]
    fn split_covers_parent_exactly_horizontal() {
        let r = Rect::new(10, 20, 101, 51);
        let (a, b) = r.split(SplitDir::Horizontal, 0.37);
        assert_eq!(a.area() + b.area(), r.area());
        assert!(!a.overlaps(b));
        assert_eq!(a.bottom(), b.y);
    }

    #[test]
    fn split_keeps_both_children_positive() {
        let r = Rect::new(0, 0, 100, 100);
        let (a, b) = r.split(SplitDir::Vertical, 0.001);
        assert!(a.w >= 1 && b.w >= 1);
    }

    #[test]
    fn inset_stays_inside() {
        let r = Rect::new(0, 0, 40, 30);
        let i = r.inset(5);
        assert!(i.within(r));
        assert_eq!(i, Rect::new(5, 5, 30, 20));
    }

    #[test]
    fn inset_clamps_when_too_small() {
        let r = Rect::new(0, 0, 6, 6);
        let i = r.inset(5);
        assert!(i.within(r));
        assert_eq!(i.w, 0);
        assert_eq!(i.h, 0);
    }

    #[test]
    fn overlap_edges_do_not_count() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(10, 0, 10, 10);
        assert!(!a.overlaps(b));
    }
}
