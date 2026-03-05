//! Workload distribution types for modeling read/write ratios.
//!
//! A `Distribution` describes the probability distribution over
//! read fractions in a workload. A read fraction `fr` means that
//! `fr` of the workload is reads and `1 - fr` is writes.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Wrapper for `f64` that implements `Eq` and `Hash` via bit
/// representation, enabling use as a `HashMap` key.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct OrderedFloat(pub f64);

impl Eq for OrderedFloat {}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl From<f64> for OrderedFloat {
    fn from(v: f64) -> Self {
        Self(v)
    }
}

impl std::fmt::Display for OrderedFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A canonicalized distribution mapping read fractions to
/// probabilities. All probabilities sum to 1.0 and all
/// fractions are in [0, 1].
pub type Canonical = HashMap<OrderedFloat, f64>;

/// Represents a distribution of read fractions in a workload.
#[derive(Debug, Clone, PartialEq)]
pub enum Distribution {
    /// A single fixed read fraction (e.g. 0.5 means 50% reads).
    Fixed(f64),

    /// A weighted distribution over multiple read fractions.
    /// Maps `read_fraction -> weight` (not yet normalized).
    Weighted(HashMap<OrderedFloat, f64>),
}

impl Distribution {
    /// Create a fixed distribution with a single read fraction.
    ///
    /// # Errors
    ///
    /// Returns an error if `read_fraction` is not in [0.0, 1.0].
    pub fn fixed(read_fraction: f64) -> Result<Self> {
        validate_fraction(read_fraction)?;
        Ok(Self::Fixed(read_fraction))
    }

    /// Create a weighted distribution from pairs of
    /// `(read_fraction, weight)`. Validates but does not
    /// normalize; call [`Distribution::canonicalize`] for normalization.
    ///
    /// # Errors
    ///
    /// Returns an error if any fraction is not in [0.0, 1.0], any weight
    /// is negative, or the weights slice is empty.
    pub fn weighted(weights: &[(f64, f64)]) -> Result<Self> {
        if weights.is_empty() {
            return Err(Error::InvalidDistribution(
                "distribution cannot be empty".into(),
            ));
        }
        for &(frac, weight) in weights {
            validate_fraction(frac)?;
            if weight < 0.0 {
                return Err(Error::InvalidDistribution(format!(
                    "weight must be non-negative, \
                         got {weight} for fraction {frac}"
                )));
            }
        }
        let total: f64 = weights.iter().map(|(_, w)| w).sum();
        if total == 0.0 {
            return Err(Error::InvalidDistribution(
                "total weight must be positive".into(),
            ));
        }
        let mapped: HashMap<OrderedFloat, f64> =
            weights.iter().map(|&(k, v)| (OrderedFloat(k), v)).collect();
        Ok(Self::Weighted(mapped))
    }

    /// Return the distinct read fractions in this distribution.
    #[must_use]
    pub fn fractions(&self) -> Vec<f64> {
        match self {
            Self::Fixed(f) => vec![*f],
            Self::Weighted(map) => map.keys().map(|k| k.0).collect(),
        }
    }

    /// Canonicalize this distribution into a map of
    /// `read_fraction -> probability` where probabilities
    /// sum to 1.0. Zero-weight entries are excluded.
    ///
    /// # Errors
    ///
    /// Returns an error if the total weight is zero (for weighted distributions).
    pub fn canonicalize(&self) -> Result<Canonical> {
        match self {
            Self::Fixed(f) => {
                let mut m = HashMap::with_capacity(1);
                m.insert(OrderedFloat(*f), 1.0);
                Ok(m)
            }
            Self::Weighted(weights) => {
                let total: f64 = weights.values().sum();
                if total == 0.0 {
                    return Err(Error::InvalidDistribution(
                        "total weight must be positive".into(),
                    ));
                }
                let m: Canonical = weights
                    .iter()
                    .filter(|(_, &w)| w > 0.0)
                    .map(|(k, &w)| (*k, w / total))
                    .collect();
                Ok(m)
            }
        }
    }
}

/// Convenience conversions so callers can pass a plain `f64`.
impl TryFrom<f64> for Distribution {
    type Error = Error;

    fn try_from(value: f64) -> Result<Self> {
        Self::fixed(value)
    }
}

impl TryFrom<i32> for Distribution {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        Self::fixed(f64::from(value))
    }
}

/// Resolve the `read_fraction` / `write_fraction` pair into a
/// canonical distribution. Exactly one must be `Some`.
///
/// When `write_fraction` is provided, each fraction `fw` is
/// converted to a read fraction as `1.0 - fw`.
///
/// # Errors
///
/// Returns an error if both parameters are `None`, both are `Some`,
/// or if canonicalization of the provided distribution fails.
pub fn canonicalize_rw(
    read_fraction: Option<&Distribution>,
    write_fraction: Option<&Distribution>,
) -> Result<Canonical> {
    match (read_fraction, write_fraction) {
        (None, None) => Err(Error::InvalidDistribution(
            "either read_fraction or write_fraction \
             must be provided"
                .into(),
        )),
        (Some(_), Some(_)) => Err(Error::InvalidDistribution(
            "only one of read_fraction or \
             write_fraction can be provided"
                .into(),
        )),
        (Some(d), None) => d.canonicalize(),
        (None, Some(d)) => {
            let canon = d.canonicalize()?;
            let flipped: Canonical = canon
                .into_iter()
                .map(|(k, p)| (OrderedFloat(1.0 - k.0), p))
                .collect();
            Ok(flipped)
        }
    }
}

fn validate_fraction(f: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&f) {
        return Err(Error::InvalidDistribution(format!(
            "fraction must be in [0, 1], got {f}"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::used_underscore_binding
)]
mod tests {
    use super::*;

    // ---- OrderedFloat -----------------------------------------

    #[test]
    fn ordered_float_eq_and_hash() {
        use std::collections::HashSet;
        let a = OrderedFloat(0.5);
        let b = OrderedFloat(0.5);
        assert_eq!(a, b);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn ordered_float_display() {
        assert_eq!(format!("{}", OrderedFloat(0.25)), "0.25");
    }

    #[test]
    fn ordered_float_from_f64() {
        let of: OrderedFloat = 0.75.into();
        assert_eq!(of.0, 0.75);
    }

    // ---- Distribution::fixed ----------------------------------

    #[test]
    fn fixed_valid() {
        let d = Distribution::fixed(0.0);
        assert!(d.is_ok());
        let d = Distribution::fixed(0.5);
        assert!(d.is_ok());
        let d = Distribution::fixed(1.0);
        assert!(d.is_ok());
    }

    #[test]
    fn fixed_out_of_range() {
        assert!(Distribution::fixed(-0.1).is_err());
        assert!(Distribution::fixed(1.1).is_err());
    }

    #[test]
    fn fixed_fractions() {
        let d = Distribution::fixed(0.3).expect("valid");
        assert_eq!(d.fractions(), vec![0.3]);
    }

    #[test]
    fn fixed_canonicalize() {
        let d = Distribution::fixed(0.8).expect("valid");
        let c = d.canonicalize().expect("valid");
        assert_eq!(c.len(), 1);
        assert!((c[&OrderedFloat(0.8)] - 1.0).abs() < f64::EPSILON);
    }

    // ---- Distribution::weighted -------------------------------

    #[test]
    fn weighted_valid() {
        let d = Distribution::weighted(&[(0.25, 1.0), (0.8, 2.0)]);
        assert!(d.is_ok());
    }

    #[test]
    fn weighted_empty() {
        assert!(Distribution::weighted(&[]).is_err());
    }

    #[test]
    fn weighted_negative_weight() {
        assert!(Distribution::weighted(&[(0.5, -1.0)]).is_err());
    }

    #[test]
    fn weighted_zero_total_weight() {
        assert!(Distribution::weighted(&[(0.5, 0.0)]).is_err());
    }

    #[test]
    fn weighted_fraction_out_of_range() {
        assert!(Distribution::weighted(&[(1.5, 1.0)]).is_err());
    }

    #[test]
    fn weighted_canonicalize_normalizes() {
        let d = Distribution::weighted(&[(0.25, 1.0), (0.8, 2.0)]).expect("valid");
        let c = d.canonicalize().expect("valid");

        assert_eq!(c.len(), 2);
        let p_25 = c[&OrderedFloat(0.25)];
        let p_80 = c[&OrderedFloat(0.8)];
        assert!((p_25 - 1.0 / 3.0).abs() < 1e-10);
        assert!((p_80 - 2.0 / 3.0).abs() < 1e-10);
        assert!((p_25 + p_80 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn weighted_canonicalize_excludes_zero_weight() {
        let d = Distribution::weighted(&[(0.1, 0.0), (0.9, 3.0)]).expect("valid");
        let c = d.canonicalize().expect("valid");
        assert_eq!(c.len(), 1);
        assert!((c[&OrderedFloat(0.9)] - 1.0).abs() < f64::EPSILON);
    }

    // ---- TryFrom conversions ----------------------------------

    #[test]
    fn try_from_f64() {
        let d: Distribution = (0.5_f64).try_into().expect("valid");
        assert_eq!(d, Distribution::Fixed(0.5));
    }

    #[test]
    fn try_from_f64_invalid() {
        let d: std::result::Result<Distribution, _> = (2.0_f64).try_into();
        assert!(d.is_err());
    }

    #[test]
    fn try_from_i32() {
        let d: Distribution = 1_i32.try_into().expect("valid");
        assert_eq!(d, Distribution::Fixed(1.0));
    }

    #[test]
    fn try_from_i32_invalid() {
        let d: std::result::Result<Distribution, _> = (-1_i32).try_into();
        assert!(d.is_err());
    }

    // ---- canonicalize_rw --------------------------------------

    #[test]
    fn canonicalize_rw_read_fraction() {
        let d = Distribution::fixed(0.6).expect("valid");
        let c = canonicalize_rw(Some(&d), None).expect("valid");
        assert_eq!(c.len(), 1);
        assert!((c[&OrderedFloat(0.6)] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn canonicalize_rw_write_fraction() {
        let d = Distribution::fixed(0.3).expect("valid");
        let c = canonicalize_rw(None, Some(&d)).expect("valid");
        assert_eq!(c.len(), 1);
        // write_fraction 0.3 -> read_fraction 0.7
        assert!((c[&OrderedFloat(0.7)] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn canonicalize_rw_write_fraction_weighted() {
        let d = Distribution::weighted(&[(0.2, 1.0), (0.5, 1.0)]).expect("valid");
        let c = canonicalize_rw(None, Some(&d)).expect("valid");
        assert_eq!(c.len(), 2);
        // write 0.2 -> read 0.8, write 0.5 -> read 0.5
        assert!((c[&OrderedFloat(0.8)] - 0.5).abs() < 1e-10);
        assert!((c[&OrderedFloat(0.5)] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn canonicalize_rw_both_none() {
        assert!(canonicalize_rw(None, None).is_err());
    }

    #[test]
    fn canonicalize_rw_both_some() {
        let d = Distribution::fixed(0.5).expect("valid");
        assert!(canonicalize_rw(Some(&d), Some(&d)).is_err());
    }
}
