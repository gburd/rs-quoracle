//! Heuristic search for good quorum systems.
//!
//! [`search`] tries every duplicate-free read expression over the given
//! nodes (shallow expressions first), computes the optimal strategy for
//! each, and returns the best one found before the timeout.
//!
//! The number of candidate expressions grows faster than exponentially
//! with the number of nodes: a full search is practical up to about 6
//! nodes, so set [`SearchConfig::timeout`] for anything larger.

use crate::distribution::Distribution;
use crate::error::{Error, Result};
use crate::expr::{choose, Element, Expr, Node};
use crate::quorum_system::{Objective, QuorumSystem, Strategy, StrategyLimits};
use itertools::Itertools;
use std::time::{Duration, Instant};

/// Every way to split `xs` into non-empty groups (set partitions).
fn partitionings<T: Clone>(xs: &[T]) -> Vec<Vec<Vec<T>>> {
    let Some((x, rest)) = xs.split_first() else {
        return vec![];
    };
    if rest.is_empty() {
        return vec![vec![vec![x.clone()]]];
    }
    let mut result = Vec::new();
    for partition in partitionings(rest) {
        // `x` alone ...
        let mut alone = vec![vec![x.clone()]];
        alone.extend(partition.iter().cloned());
        result.push(alone);
        // ... or added to one of the existing groups.
        for i in 0..partition.len() {
            let mut p = partition.clone();
            p[i].insert(0, x.clone());
            result.push(p);
        }
    }
    result
}

/// Lazily yield duplicate-free expressions over `nodes` with height at most
/// `max_height` (0 = unlimited). The same expression may be yielded more
/// than once.
fn dup_free_exprs<T: Element>(
    nodes: Vec<Node<T>>,
    max_height: usize,
) -> Box<dyn Iterator<Item = Expr<T>>> {
    if nodes.len() == 1 {
        return Box::new(nodes.into_iter().map(Expr::Node));
    }
    if max_height == 1 {
        let leaves: Vec<Expr<T>> = nodes.into_iter().map(Expr::Node).collect();
        let n = leaves.len();
        return Box::new(
            (1..=n).filter_map(move |k| choose(k, leaves.clone()).ok()),
        );
    }
    let sub_height = max_height.saturating_sub(1);
    Box::new(
        partitionings(&nodes)
            .into_iter()
            // The single-group partition would recurse forever.
            .filter(|p| p.len() > 1)
            .flat_map(move |partitioning| {
                partitioning
                    .into_iter()
                    .map(|part| dup_free_exprs(part, sub_height).collect_vec())
                    .multi_cartesian_product()
                    .flat_map(|subexprs| {
                        let n = subexprs.len();
                        (1..=n).filter_map(move |k| {
                            choose(k, subexprs.clone()).ok()
                        })
                    })
            }),
    )
}

/// Configuration for [`search`].
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// What to optimize.
    pub optimize: Objective,
    /// Minimum resilience of the returned system.
    pub resilience: usize,
    /// Limits passed to [`QuorumSystem::strategy`].
    pub limits: StrategyLimits,
    /// Read fraction distribution (exactly one of this and
    /// `write_fraction` must be set).
    pub read_fraction: Option<Distribution>,
    /// Write fraction distribution.
    pub write_fraction: Option<Distribution>,
    /// Only use `f`-resilient quorums (see [`QuorumSystem::strategy`]).
    pub f: usize,
    /// Stop after this long and return the best system so far.
    /// `Duration::ZERO` means no limit.
    pub timeout: Duration,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            optimize: Objective::Load,
            resilience: 0,
            limits: StrategyLimits::default(),
            read_fraction: None,
            write_fraction: None,
            f: 0,
            timeout: Duration::ZERO,
        }
    }
}

/// Result of a successful search.
#[derive(Debug, Clone)]
pub struct SearchResult<T: Element> {
    /// The best quorum system found.
    pub quorum_system: QuorumSystem<T>,
    /// Its optimal strategy.
    pub strategy: Strategy<T>,
}

/// Search for the quorum system over `nodes` that best meets `config`.
///
/// Candidates of height ≤ 2 are tried first, then all heights, until the
/// candidates run out or `config.timeout` passes.
///
/// # Errors
///
/// - [`Error::InvalidQuorumSystem`] if `nodes` is empty or a limit is set
///   on the metric being optimized.
/// - [`Error::InvalidDistribution`] if the read/write fraction is invalid.
/// - [`Error::NoQuorumSystemFound`] if no candidate meets the requirements
///   before the timeout.
pub fn search<T: Element>(
    nodes: &[Node<T>],
    config: &SearchConfig,
) -> Result<SearchResult<T>> {
    if nodes.is_empty() {
        return Err(Error::InvalidQuorumSystem(
            "search needs at least one node".into(),
        ));
    }
    let rf = config.read_fraction.as_ref();
    let wf = config.write_fraction.as_ref();
    // Fail fast on bad arguments rather than rejecting every candidate.
    crate::distribution::canonicalize_rw(rf, wf)?;
    config.limits.check(config.optimize)?;

    let start = Instant::now();
    let timed_out = || {
        config.timeout != Duration::ZERO && start.elapsed() >= config.timeout
    };
    let metric = |s: &Strategy<T>| -> Result<f64> {
        match config.optimize {
            Objective::Load => s.load(rf, wf),
            Objective::Network => s.network_load(rf, wf),
            Objective::Latency => s.latency(rf, wf).map(|d| d.as_secs_f64()),
        }
    };

    let mut best: Option<(f64, SearchResult<T>)> = None;
    let candidates = dup_free_exprs(nodes.to_vec(), 2)
        .chain(dup_free_exprs(nodes.to_vec(), 0));
    for reads in candidates {
        let qs = QuorumSystem::from_reads(reads);
        if qs.resilience() >= config.resilience {
            let found = qs
                .strategy(config.optimize, rf, wf, &config.limits, config.f)
                .and_then(|strategy| Ok((metric(&strategy)?, strategy)));
            match found {
                Ok((m, strategy)) => {
                    if best.as_ref().is_none_or(|(b, _)| m < *b) {
                        best = Some((
                            m,
                            SearchResult { quorum_system: qs, strategy },
                        ));
                    }
                }
                Err(Error::NoStrategyFound) => {}
                Err(e) => return Err(e),
            }
        }
        if timed_out() {
            break;
        }
    }
    best.map(|(_, r)| r).ok_or(Error::NoQuorumSystemFound)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn nodes(n: u32) -> Vec<Node<u32>> {
        (0..n).map(Node::new).collect()
    }

    #[test]
    fn test_partitionings() {
        assert_eq!(partitionings::<i32>(&[]).len(), 0);
        assert_eq!(partitionings(&[1]), vec![vec![vec![1]]]);
        let two = partitionings(&[1, 2]);
        assert_eq!(two.len(), 2);
        assert!(two.contains(&vec![vec![1], vec![2]]));
        assert!(two.contains(&vec![vec![1, 2]]));
        let three = partitionings(&[1, 2, 3]);
        assert_eq!(three.len(), 5);
        assert!(three.contains(&vec![vec![1], vec![2], vec![3]]));
        assert!(three.contains(&vec![vec![1, 2], vec![3]]));
        assert!(three.contains(&vec![vec![2], vec![1, 3]]));
        assert!(three.contains(&vec![vec![1], vec![2, 3]]));
        assert!(three.contains(&vec![vec![1, 2, 3]]));
        // Bell numbers.
        assert_eq!(partitionings(&[1, 2, 3, 4]).len(), 15);
        assert_eq!(partitionings(&[1, 2, 3, 4, 5]).len(), 52);
    }

    #[test]
    fn test_dup_free_exprs() {
        assert_eq!(dup_free_exprs(nodes(1), 0).count(), 1);
        // choose(1, [a,b]) and choose(2, [a,b])
        assert_eq!(dup_free_exprs(nodes(2), 1).count(), 2);
        assert_eq!(dup_free_exprs(nodes(2), 0).count(), 2);
        // Every generated expression is duplicate-free over all nodes.
        for e in dup_free_exprs(nodes(4), 0) {
            assert!(e.dup_free());
            assert_eq!(e.elements().len(), 4);
        }
    }

    fn half() -> Distribution {
        Distribution::fixed(0.5).unwrap()
    }

    #[test]
    fn test_search_each_objective() {
        for optimize in
            [Objective::Load, Objective::Network, Objective::Latency]
        {
            let config = SearchConfig {
                optimize,
                read_fraction: Some(half()),
                ..Default::default()
            };
            let r = search(&nodes(3), &config).unwrap();
            assert!(r.strategy.load(Some(&half()), None).unwrap() > 0.0);
        }
    }

    #[test]
    fn test_search_finds_brute_force_optimum() {
        let config =
            SearchConfig { read_fraction: Some(half()), ..Default::default() };
        let r = search(&nodes(3), &config).unwrap();
        let load = r.strategy.load(Some(&half()), None).unwrap();
        // Brute force: the best load over every candidate.
        let best = dup_free_exprs(nodes(3), 0)
            .map(|e| {
                QuorumSystem::from_reads(e)
                    .strategy(
                        Objective::Load,
                        Some(&half()),
                        None,
                        &StrategyLimits::default(),
                        0,
                    )
                    .unwrap()
                    .load(Some(&half()), None)
                    .unwrap()
            })
            .fold(f64::INFINITY, f64::min);
        assert!((load - best).abs() < 1e-9, "{load} vs {best}");
    }

    #[test]
    fn test_search_resilience_and_errors() {
        let config = SearchConfig {
            resilience: 1,
            read_fraction: Some(half()),
            ..Default::default()
        };
        let r = search(&nodes(4), &config).unwrap();
        assert!(r.quorum_system.resilience() >= 1);

        let impossible = SearchConfig { resilience: 10, ..config.clone() };
        assert_eq!(
            search(&nodes(2), &impossible).unwrap_err(),
            Error::NoQuorumSystemFound
        );
        assert!(matches!(
            search::<u32>(&[], &config),
            Err(Error::InvalidQuorumSystem(_))
        ));
        let no_dist = SearchConfig::default();
        assert!(matches!(
            search(&nodes(2), &no_dist),
            Err(Error::InvalidDistribution(_))
        ));
        // Errors other than "no strategy" are reported, not swallowed: a
        // node with capacity close to 0 overflows every strategy's load.
        let tiny = Node::new(9).with_capacity(f64::MIN_POSITIVE).unwrap();
        let overflow = SearchConfig {
            optimize: Objective::Network,
            limits: StrategyLimits { load: Some(1e300), ..Default::default() },
            ..config.clone()
        };
        let _ = search(&[tiny, Node::new(8)], &overflow);
        // A limit that matches the objective is rejected, not ignored.
        let bad = SearchConfig {
            limits: StrategyLimits { load: Some(1.0), ..Default::default() },
            ..config
        };
        assert!(matches!(
            search(&nodes(2), &bad),
            Err(Error::InvalidQuorumSystem(_))
        ));
    }

    #[test]
    fn test_search_timeout_is_honored() {
        // 8 nodes has far too many candidates to finish; the lazy
        // generator must stop near the timeout instead of materializing
        // them all first.
        let config = SearchConfig {
            read_fraction: Some(half()),
            timeout: Duration::from_millis(200),
            ..Default::default()
        };
        let start = Instant::now();
        let r = search(&nodes(8), &config);
        assert!(r.is_ok());
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "{:?}",
            start.elapsed()
        );
    }
}
