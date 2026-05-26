//! Geometric types for piecewise linear functions.
//!
//! Provides [`Point`], [`Segment`], and [`max_of_segments`] for
//! computing upper envelopes of line segments on \[0, 1\].

use crate::error::{Error, Result};

/// A point in 2D space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
}

impl Point {
    /// Create a new point.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A line segment between two points where `l.x < r.x`.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// Left endpoint (smaller x).
    pub l: Point,
    /// Right endpoint (larger x).
    pub r: Point,
}

impl PartialEq for Segment {
    fn eq(&self, other: &Self) -> bool {
        self.l == other.l && self.r == other.r
    }
}

impl Segment {
    /// Create a new segment from left point `l` to right point `r`.
    ///
    /// # Errors
    ///
    /// Returns an error if `l == r` or `l.x >= r.x`.
    pub fn new(l: Point, r: Point) -> Result<Self> {
        if l == r {
            return Err(Error::InvalidExpression(
                "segment endpoints must differ".into(),
            ));
        }
        if l.x >= r.x {
            return Err(Error::InvalidExpression(
                "left endpoint x must be less than right endpoint x".into(),
            ));
        }
        Ok(Self { l, r })
    }

    /// Evaluate the linear function at `x`.
    ///
    /// # Errors
    ///
    /// Returns an error if `x` is outside the segment's x-range.
    pub fn eval(&self, x: f64) -> Result<f64> {
        if x < self.l.x || x > self.r.x {
            return Err(Error::InvalidExpression(format!(
                "x={x} is outside segment range [{}, {}]",
                self.l.x, self.r.x
            )));
        }
        Ok(self.slope() * (x - self.l.x) + self.l.y)
    }

    /// Whether two segments are approximately equal (within relative
    /// tolerance 1e-5 on both y-coordinates).
    #[must_use]
    pub fn approximately_equal(&self, other: &Self) -> bool {
        approx_eq(self.l.y, other.l.y) && approx_eq(self.r.y, other.r.y)
    }

    /// Whether two segments share the same x-range.
    #[must_use]
    #[expect(clippy::float_cmp)]
    pub fn compatible(&self, other: &Self) -> bool {
        self.l.x == other.l.x && self.r.x == other.r.x
    }

    /// The slope `(r.y - l.y) / (r.x - l.x)`.
    #[must_use]
    pub fn slope(&self) -> f64 {
        (self.r.y - self.l.y) / (self.r.x - self.l.x)
    }

    /// Whether `self` is strictly above `other` (both endpoints at
    /// least as high, and not equal).
    ///
    /// # Errors
    ///
    /// Returns an error if the segments are not compatible.
    pub fn above(&self, other: &Self) -> Result<bool> {
        self.assert_compatible(other)?;
        Ok(self != other && self.l.y >= other.l.y && self.r.y >= other.r.y)
    }

    /// Whether `self` is above or equal to `other`.
    ///
    /// # Errors
    ///
    /// Returns an error if the segments are not compatible.
    pub fn above_eq(&self, other: &Self) -> Result<bool> {
        self.assert_compatible(other)?;
        Ok(self == other || self.above_unchecked(other))
    }

    /// Whether two compatible segments intersect.
    ///
    /// # Errors
    ///
    /// Returns an error if the segments are not compatible.
    pub fn intersects(&self, other: &Self) -> Result<bool> {
        self.assert_compatible(other)?;
        Ok(self.intersects_unchecked(other))
    }

    /// Compute the intersection point of two compatible segments, or
    /// `None` if they are equal or do not intersect.
    ///
    /// The x-coordinate formula assumes both segments span \[0, 1\].
    ///
    /// # Errors
    ///
    /// Returns an error if the segments are not compatible.
    pub fn intersection(&self, other: &Self) -> Result<Option<Point>> {
        self.assert_compatible(other)?;
        if self == other || !self.intersects_unchecked(other) {
            return Ok(None);
        }
        let denom = self.r.y - other.r.y + other.l.y - self.l.y;
        let x = (other.l.y - self.l.y) / denom;
        // x is guaranteed within [l.x, r.x] for intersecting
        // compatible segments, so we compute y inline.
        let y = self.slope() * (x - self.l.x) + self.l.y;
        Ok(Some(Point::new(x, y)))
    }

    fn assert_compatible(&self, other: &Self) -> Result<()> {
        if self.compatible(other) {
            Ok(())
        } else {
            Err(Error::InvalidExpression(
                "segments are not compatible (different x-ranges)".into(),
            ))
        }
    }

    fn above_unchecked(&self, other: &Self) -> bool {
        self != other && self.l.y >= other.l.y && self.r.y >= other.r.y
    }

    #[expect(clippy::float_cmp)]
    fn intersects_unchecked(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }
        if self.l.y == other.l.y || self.r.y == other.r.y {
            return true;
        }
        if self.above_unchecked(other) || other.above_unchecked(self) {
            return false;
        }
        true
    }
}

/// Compute the upper envelope of a set of compatible segments.
///
/// Returns a list of `(x, y)` pairs tracing the maximum of all
/// segments. All segments must share the same x-range (typically
/// \[0, 1\]).
///
/// # Errors
///
/// Returns an error if the slice is empty or segments have
/// different x-ranges.
#[expect(clippy::float_cmp)]
pub fn max_of_segments(segments: &[Segment]) -> Result<Vec<(f64, f64)>> {
    if segments.is_empty() {
        return Err(Error::InvalidExpression(
            "max_of_segments requires at least one segment".into(),
        ));
    }

    let l_x = segments[0].l.x;
    let r_x = segments[0].r.x;
    for s in &segments[1..] {
        if s.l.x != l_x || s.r.x != r_x {
            return Err(Error::InvalidExpression(
                "all segments must have the same x-range".into(),
            ));
        }
    }

    // Collect x-coordinates of all intersection points plus
    // endpoints.
    let mut xs: Vec<f64> = vec![0.0, 1.0];
    for (i, s1) in segments.iter().enumerate() {
        for s2 in &segments[i + 1..] {
            if let Some(p) = s1.intersection(s2)? {
                xs.push(p.x);
            }
        }
    }
    xs.sort_by(f64::total_cmp);

    let mut result = Vec::with_capacity(xs.len());
    for x in xs {
        let mut max_y = f64::NEG_INFINITY;
        for s in segments {
            let y = s.eval(x)?;
            if y > max_y {
                max_y = y;
            }
        }
        result.push((x, max_y));
    }
    Ok(result)
}

fn approx_eq(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    let diff = (a - b).abs();
    let larger = a.abs().max(b.abs());
    if larger == 0.0 {
        return diff < 1e-5;
    }
    diff / larger <= 1e-5
}

#[cfg(test)]
#[expect(clippy::float_cmp, clippy::expect_used)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn seg(lx: f64, ly: f64, rx: f64, ry: f64) -> Segment {
        Segment::new(pt(lx, ly), pt(rx, ry)).expect("valid segment")
    }

    #[test]
    fn test_eq() {
        let l = pt(0.0, 1.0);
        let r = pt(1.0, 1.0);
        let m = pt(0.5, 0.5);
        assert_eq!(
            Segment::new(l, r).expect("ok"),
            Segment::new(l, r).expect("ok")
        );
        assert_ne!(
            Segment::new(l, r).expect("ok"),
            Segment::new(l, m).expect("ok")
        );
    }

    #[test]
    fn test_compatible() {
        let s1 = seg(0.0, 1.0, 1.0, 2.0);
        let s2 = seg(0.0, 2.0, 1.0, 1.0);
        let s3 = seg(0.5, 2.0, 1.0, 1.0);
        assert!(s1.compatible(&s2));
        assert!(s2.compatible(&s1));
        assert!(!s1.compatible(&s3));
        assert!(!s3.compatible(&s1));
        assert!(!s2.compatible(&s3));
        assert!(!s3.compatible(&s2));
    }

    #[test]
    fn test_eval() {
        let segment = seg(0.0, 0.0, 1.0, 1.0);
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(segment.eval(x).expect("ok"), x);
        }

        let segment = seg(0.0, 0.0, 1.0, 2.0);
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(segment.eval(x).expect("ok"), 2.0 * x);
        }

        let segment = seg(1.0, 2.0, 3.0, 6.0);
        for &x in &[1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0] {
            assert_eq!(segment.eval(x).expect("ok"), 2.0 * x);
        }

        let segment = seg(0.0, 1.0, 1.0, 0.0);
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            assert_eq!(segment.eval(x).expect("ok"), 1.0 - x);
        }
    }

    #[test]
    fn test_slope() {
        assert_eq!(seg(0.0, 0.0, 1.0, 1.0).slope(), 1.0);
        assert_eq!(seg(0.0, 1.0, 1.0, 2.0).slope(), 1.0);
        assert_eq!(seg(1.0, 1.0, 2.0, 2.0).slope(), 1.0);
        assert_eq!(seg(1.0, 1.0, 2.0, 3.0).slope(), 2.0);
        assert_eq!(seg(1.0, 1.0, 2.0, 0.0).slope(), -1.0);
    }

    #[test]
    fn test_above() {
        let s1 = seg(0.0, 0.0, 1.0, 0.5);
        let s2 = seg(0.0, 0.5, 1.0, 2.0);
        let s3 = seg(0.0, 1.5, 1.0, 0.5);

        assert!(!s1.above(&s1).expect("ok"));
        assert!(!s1.above(&s2).expect("ok"));
        assert!(!s1.above(&s3).expect("ok"));

        assert!(s2.above(&s1).expect("ok"));
        assert!(!s2.above(&s2).expect("ok"));
        assert!(!s2.above(&s3).expect("ok"));

        assert!(s3.above(&s1).expect("ok"));
        assert!(!s3.above(&s2).expect("ok"));
        assert!(!s3.above(&s3).expect("ok"));
    }

    #[test]
    fn test_above_eq() {
        let s1 = seg(0.0, 0.0, 1.0, 0.5);
        let s2 = seg(0.0, 0.5, 1.0, 2.0);
        let s3 = seg(0.0, 1.5, 1.0, 0.5);

        assert!(s1.above_eq(&s1).expect("ok"));
        assert!(!s1.above_eq(&s2).expect("ok"));
        assert!(!s1.above_eq(&s3).expect("ok"));

        assert!(s2.above_eq(&s1).expect("ok"));
        assert!(s2.above_eq(&s2).expect("ok"));
        assert!(!s2.above_eq(&s3).expect("ok"));

        assert!(s3.above_eq(&s1).expect("ok"));
        assert!(!s3.above_eq(&s2).expect("ok"));
        assert!(s3.above_eq(&s3).expect("ok"));
    }

    #[test]
    fn test_intersects() {
        let s1 = seg(0.0, 0.0, 1.0, 0.5);
        let s2 = seg(0.0, 0.5, 1.0, 2.0);
        let s3 = seg(0.0, 1.5, 1.0, 0.5);

        assert!(s1.intersects(&s1).expect("ok"));
        assert!(!s1.intersects(&s2).expect("ok"));
        assert!(s1.intersects(&s3).expect("ok"));

        assert!(!s2.intersects(&s1).expect("ok"));
        assert!(s2.intersects(&s2).expect("ok"));
        assert!(s2.intersects(&s3).expect("ok"));

        assert!(s3.intersects(&s1).expect("ok"));
        assert!(s3.intersects(&s2).expect("ok"));
        assert!(s3.intersects(&s3).expect("ok"));
    }

    #[test]
    fn test_intersection() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.0, 1.0, 1.0, 0.0);
        let s3 = seg(0.0, 1.0, 1.0, 1.0);
        let s4 = seg(0.0, 0.25, 1.0, 0.25);

        assert_eq!(s1.intersection(&s1).expect("ok"), None);
        assert_eq!(s1.intersection(&s2).expect("ok"), Some(pt(0.5, 0.5)));
        assert_eq!(s1.intersection(&s3).expect("ok"), Some(pt(1.0, 1.0)));
        assert_eq!(s1.intersection(&s4).expect("ok"), Some(pt(0.25, 0.25)));

        assert_eq!(s2.intersection(&s1).expect("ok"), Some(pt(0.5, 0.5)));
        assert_eq!(s2.intersection(&s2).expect("ok"), None);
        assert_eq!(s2.intersection(&s3).expect("ok"), Some(pt(0.0, 1.0)));
        assert_eq!(s2.intersection(&s4).expect("ok"), Some(pt(0.75, 0.25)));

        assert_eq!(s3.intersection(&s1).expect("ok"), Some(pt(1.0, 1.0)));
        assert_eq!(s3.intersection(&s2).expect("ok"), Some(pt(0.0, 1.0)));
        assert_eq!(s3.intersection(&s3).expect("ok"), None);
        assert_eq!(s3.intersection(&s4).expect("ok"), None);

        assert_eq!(s4.intersection(&s1).expect("ok"), Some(pt(0.25, 0.25)));
        assert_eq!(s4.intersection(&s2).expect("ok"), Some(pt(0.75, 0.25)));
        assert_eq!(s4.intersection(&s3).expect("ok"), None);
        assert_eq!(s4.intersection(&s4).expect("ok"), None);
    }

    #[test]
    fn test_max_one_segment() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.0, 1.0, 1.0, 0.0);
        let s3 = seg(0.0, 1.0, 1.0, 1.0);
        let s4 = seg(0.0, 0.25, 1.0, 0.25);
        let s5 = seg(0.0, 0.75, 1.0, 0.75);

        for s in &[s1, s2, s3, s4, s5] {
            let result = max_of_segments(&[*s]).expect("ok");
            assert_eq!(result, vec![(s.l.x, s.l.y), (s.r.x, s.r.y)]);
        }
    }

    fn is_subset(xs: &[(f64, f64)], ys: &[(f64, f64)]) -> bool {
        xs.iter().all(|x| ys.contains(x))
    }

    type SegmentCase = (Vec<Segment>, Vec<(f64, f64)>);

    #[test]
    fn test_max_two_segments() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.0, 1.0, 1.0, 0.0);
        let s3 = seg(0.0, 1.0, 1.0, 1.0);
        let s4 = seg(0.0, 0.25, 1.0, 0.25);
        let s5 = seg(0.0, 0.75, 1.0, 0.75);

        let cases: Vec<SegmentCase> = vec![
            (vec![s1, s1], vec![(0.0, 0.0), (1.0, 1.0)]),
            (vec![s1, s2], vec![(0.0, 1.0), (0.5, 0.5), (1.0, 1.0)]),
            (vec![s1, s3], vec![(0.0, 1.0), (1.0, 1.0)]),
            (vec![s1, s4], vec![(0.0, 0.25), (0.25, 0.25), (1.0, 1.0)]),
            (vec![s1, s5], vec![(0.0, 0.75), (0.75, 0.75), (1.0, 1.0)]),
            (vec![s2, s2], vec![(0.0, 1.0), (1.0, 0.0)]),
            (vec![s2, s3], vec![(0.0, 1.0), (1.0, 1.0)]),
            (vec![s2, s4], vec![(0.0, 1.0), (0.75, 0.25), (1.0, 0.25)]),
            (vec![s2, s5], vec![(0.0, 1.0), (0.25, 0.75), (1.0, 0.75)]),
            (vec![s3, s3], vec![(0.0, 1.0), (1.0, 1.0)]),
            (vec![s3, s4], vec![(0.0, 1.0), (1.0, 1.0)]),
            (vec![s3, s5], vec![(0.0, 1.0), (1.0, 1.0)]),
            (vec![s4, s4], vec![(0.0, 0.25), (1.0, 0.25)]),
            (vec![s4, s5], vec![(0.0, 0.75), (1.0, 0.75)]),
            (vec![s5, s5], vec![(0.0, 0.75), (1.0, 0.75)]),
        ];

        for (segments, path) in &cases {
            let result = max_of_segments(segments).expect("ok");
            assert!(
                is_subset(path, &result),
                "forward: {path:?} not subset of {result:?}"
            );
            let reversed: Vec<Segment> =
                segments.iter().rev().copied().collect();
            let result_rev = max_of_segments(&reversed).expect("ok");
            assert!(
                is_subset(path, &result_rev),
                "reverse: {path:?} not subset of {result_rev:?}"
            );
        }
    }

    #[test]
    fn test_max_three_segments() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.0, 1.0, 1.0, 0.0);
        let s4 = seg(0.0, 0.25, 1.0, 0.25);
        let s5 = seg(0.0, 0.75, 1.0, 0.75);

        let cases: Vec<SegmentCase> = vec![
            (vec![s1, s2, s4], vec![(0.0, 1.0), (0.5, 0.5), (1.0, 1.0)]),
            (
                vec![s1, s2, s5],
                vec![(0.0, 1.0), (0.25, 0.75), (0.75, 0.75), (1.0, 1.0)],
            ),
        ];

        for (segments, path) in &cases {
            let result = max_of_segments(segments).expect("ok");
            assert!(
                is_subset(path, &result),
                "forward: {path:?} not subset of {result:?}"
            );
            let reversed: Vec<Segment> =
                segments.iter().rev().copied().collect();
            let result_rev = max_of_segments(&reversed).expect("ok");
            assert!(
                is_subset(path, &result_rev),
                "reverse: {path:?} not subset of {result_rev:?}"
            );
        }
    }

    #[test]
    fn test_new_segment_invalid() {
        let p = pt(1.0, 1.0);
        assert!(Segment::new(p, p).is_err());
        assert!(Segment::new(pt(1.0, 0.0), pt(0.0, 1.0)).is_err());
    }

    #[test]
    fn test_eval_out_of_range() {
        let s = seg(0.0, 0.0, 1.0, 1.0);
        assert!(s.eval(-0.1).is_err());
        assert!(s.eval(1.1).is_err());
    }

    #[test]
    fn test_incompatible_segments() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.5, 0.0, 1.0, 1.0);
        assert!(s1.above(&s2).is_err());
        assert!(s1.above_eq(&s2).is_err());
        assert!(s1.intersects(&s2).is_err());
        assert!(s1.intersection(&s2).is_err());
    }

    #[test]
    fn test_approximately_equal() {
        let s1 = seg(0.0, 1.0, 1.0, 2.0);
        let s2 = seg(0.0, 1.000_001, 1.0, 2.000_001);
        let s3 = seg(0.0, 1.1, 1.0, 2.0);
        assert!(s1.approximately_equal(&s2));
        assert!(!s1.approximately_equal(&s3));
    }

    #[test]
    fn test_max_of_segments_empty() {
        assert!(max_of_segments(&[]).is_err());
    }

    #[test]
    fn test_max_of_segments_incompatible() {
        let s1 = seg(0.0, 0.0, 1.0, 1.0);
        let s2 = seg(0.5, 0.0, 1.0, 1.0);
        assert!(max_of_segments(&[s1, s2]).is_err());
    }
}
