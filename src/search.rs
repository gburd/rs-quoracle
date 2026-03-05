//! Heuristic search for optimal quorum systems
//!
//! This module provides algorithms to search for optimal quorum system
//! configurations by exploring the space of duplicate-free expressions.

use crate::distribution::Distribution;
use crate::error::{Error, Result};
use crate::expr::{choose, Element, Expr, Node};
use crate::quorum_system::{Objective, QuorumSystem, Strategy};
use std::time::{Duration, Instant};

/// Generate all partitionings of a list.
///
/// For example, `partitionings([1, 2, 3])` yields:
/// - `[[1], [2], [3]]`
/// - `[[1, 2], [3]]`
/// - `[[1, 3], [2]]`
/// - `[[2, 3], [1]]`
/// - `[[1, 2, 3]]`
fn partitionings_helper<T: Clone>(xs: &[T]) -> Vec<Vec<Vec<T>>> {
    if xs.is_empty() {
        return vec![vec![]];
    }

    let x = xs[0].clone();
    let rest = &xs[1..];
    let mut result = Vec::new();

    for partition in partitionings_helper(rest) {
        // Add x as a singleton partition
        let mut new_partition = vec![vec![x.clone()]];
        new_partition.extend(partition.clone());
        result.push(new_partition);

        // Add x to each existing partition
        for i in 0..partition.len() {
            let mut new_partition = Vec::new();
            for (j, part) in partition.iter().enumerate() {
                if i == j {
                    let mut new_part = vec![x.clone()];
                    new_part.extend(part.clone());
                    new_partition.push(new_part);
                } else {
                    new_partition.push(part.clone());
                }
            }
            result.push(new_partition);
        }
    }

    result
}

fn partitionings<T: Clone>(xs: &[T]) -> Vec<Vec<Vec<T>>> {
    if xs.is_empty() {
        return vec![];
    }
    partitionings_helper(xs)
}

/// Generate all duplicate-free expressions over nodes.
///
/// Yields expressions with height at most `max_height`.
/// If `max_height` is 0, there is no height limit.
fn dup_free_exprs<T: Element>(nodes: &[Node<T>], max_height: usize) -> Vec<Expr<T>> {
    assert!(!nodes.is_empty(), "nodes must not be empty");

    if nodes.len() == 1 {
        return vec![Expr::Node(nodes[0].clone())];
    }

    if max_height == 1 {
        let mut result = Vec::new();
        let node_exprs: Vec<Expr<T>> = nodes.iter().map(|n| Expr::Node(n.clone())).collect();
        for k in 1..=nodes.len() {
            if let Ok(expr) = choose(k, node_exprs.clone()) {
                result.push(expr);
            }
        }
        return result;
    }

    let mut result = Vec::new();
    let parts = partitionings(nodes);

    for partitioning in parts {
        // Skip partitioning with all nodes in single partition
        if partitioning.len() == 1 {
            continue;
        }

        // Generate subexpressions for each partition
        let mut subexpr_lists = Vec::new();
        for part in &partitioning {
            let subexprs = dup_free_exprs(part, max_height.saturating_sub(1));
            subexpr_lists.push(subexprs);
        }

        // Cartesian product of subexpressions
        let mut combinations = vec![vec![]];
        for subexprs in subexpr_lists {
            let mut new_combinations = Vec::new();
            for combo in &combinations {
                for expr in &subexprs {
                    let mut new_combo = combo.clone();
                    new_combo.push(expr.clone());
                    new_combinations.push(new_combo);
                }
            }
            combinations = new_combinations;
        }

        // Create choose expressions
        for subexprs in combinations {
            for k in 1..=subexprs.len() {
                if let Ok(expr) = choose(k, subexprs.clone()) {
                    result.push(expr);
                }
            }
        }
    }

    result
}

/// Configuration for heuristic search.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Optimization objective (Load, Network, or Latency).
    pub optimize: Objective,
    /// Minimum resilience requirement.
    pub resilience: i64,
    /// Optional load limit constraint.
    pub load_limit: Option<f64>,
    /// Optional network limit constraint.
    pub network_limit: Option<f64>,
    /// Optional latency limit constraint.
    pub latency_limit: Option<Duration>,
    /// Read fraction distribution.
    pub read_fraction: Option<Distribution>,
    /// Write fraction distribution.
    pub write_fraction: Option<Distribution>,
    /// F-resilience requirement.
    pub f: usize,
    /// Maximum search time.
    pub timeout: Duration,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            optimize: Objective::Load,
            resilience: 0,
            load_limit: None,
            network_limit: None,
            latency_limit: None,
            read_fraction: None,
            write_fraction: None,
            f: 0,
            timeout: Duration::from_secs(0),
        }
    }
}

/// Result of a successful search.
#[derive(Debug, Clone)]
pub struct SearchResult<T: Element> {
    /// The optimal quorum system found.
    pub quorum_system: QuorumSystem<T>,
    /// The optimal strategy for the system.
    pub strategy: Strategy<T>,
}

/// Search for an optimal quorum system using heuristic search.
///
/// This function explores the space of duplicate-free expressions,
/// first with height ≤ 2 (quick pass), then with unlimited height,
/// until timeout is reached or all expressions are exhausted.
///
/// # Arguments
///
/// * `nodes` - The nodes to use in the search
/// * `config` - Search configuration
///
/// # Returns
///
/// The best quorum system and strategy found.
///
/// # Errors
///
/// Returns `Error::NoQuorumSystemFound` if no valid system satisfying
/// the constraints is found within the timeout.
pub fn search<T: Element>(nodes: &[Node<T>], config: &SearchConfig) -> Result<SearchResult<T>> {
    let start_time = Instant::now();

    let mut opt_qs: Option<QuorumSystem<T>> = None;
    let mut opt_strategy: Option<Strategy<T>> = None;
    let mut opt_metric: Option<f64> = None;

    let metric = |strategy: &Strategy<T>| -> Result<f64> {
        match config.optimize {
            Objective::Load => strategy.load(
                config.read_fraction.as_ref(),
                config.write_fraction.as_ref(),
            ),
            Objective::Network => strategy.network_load(
                config.read_fraction.as_ref(),
                config.write_fraction.as_ref(),
            ),
            Objective::Latency => {
                let duration = strategy.latency(
                    config.read_fraction.as_ref(),
                    config.write_fraction.as_ref(),
                )?;
                Ok(duration.as_secs_f64())
            }
        }
    };

    let mut do_search = |exprs: Vec<Expr<T>>| -> bool {
        for reads in exprs {
            let qs = QuorumSystem::from_reads(reads);

            if qs.resilience() < config.resilience {
                continue;
            }

            match qs.strategy(
                config.optimize,
                config.read_fraction.as_ref(),
                config.write_fraction.as_ref(),
                config.load_limit,
                config.network_limit,
                config.latency_limit,
                config.f,
            ) {
                Ok(strategy) => {
                    if let Ok(strategy_metric) = metric(&strategy) {
                        let is_better = match opt_metric {
                            None => true,
                            Some(current) => strategy_metric < current,
                        };
                        if is_better {
                            opt_qs = Some(qs);
                            opt_strategy = Some(strategy);
                            opt_metric = Some(strategy_metric);
                        }
                    }
                }
                Err(Error::NoStrategyFound | _) => continue,
            }

            // Check timeout
            if config.timeout != Duration::from_secs(0) && start_time.elapsed() >= config.timeout {
                return true; // Timed out
            }
        }
        false // Not timed out
    };

    // Quick pass with height ≤ 2
    let exprs_h2 = dup_free_exprs(nodes, 2);
    if do_search(exprs_h2) {
        // Timed out during height 2 search
        return match (opt_qs, opt_strategy) {
            (Some(qs), Some(strategy)) => Ok(SearchResult {
                quorum_system: qs,
                strategy,
            }),
            _ => Err(Error::NoQuorumSystemFound),
        };
    }

    // Full search with unlimited height
    let exprs = dup_free_exprs(nodes, 0);
    do_search(exprs);

    match (opt_qs, opt_strategy) {
        (Some(qs), Some(strategy)) => Ok(SearchResult {
            quorum_system: qs,
            strategy,
        }),
        _ => Err(Error::NoQuorumSystemFound),
    }
}

#[cfg(test)]
#[allow(clippy::cloned_ref_to_slice_refs)]
mod tests {
    use super::*;

    #[test]
    fn test_partitionings_empty() {
        let result = partitionings::<i32>(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_partitionings_single() {
        let result = partitionings(&[1]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![vec![1]]);
    }

    #[test]
    fn test_partitionings_two() {
        let result = partitionings(&[1, 2]);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&vec![vec![1], vec![2]]));
        assert!(result.contains(&vec![vec![1, 2]]));
    }

    #[test]
    fn test_partitionings_three() {
        let result = partitionings(&[1, 2, 3]);
        assert_eq!(result.len(), 5);
        assert!(result.contains(&vec![vec![1], vec![2], vec![3]]));
        assert!(result.contains(&vec![vec![1, 2], vec![3]]));
        assert!(result.contains(&vec![vec![2], vec![1, 3]]));
        assert!(result.contains(&vec![vec![1], vec![2, 3]]));
        assert!(result.contains(&vec![vec![1, 2, 3]]));
    }

    #[test]
    fn test_dup_free_exprs_single() {
        let a = Node::new('a');
        let exprs = dup_free_exprs(&[a.clone()], 0);
        assert_eq!(exprs.len(), 1);
    }

    #[test]
    fn test_dup_free_exprs_two_height_one() {
        let a = Node::new('a');
        let b = Node::new('b');
        let exprs = dup_free_exprs(&[a, b], 1);
        // Should generate choose(1, [a,b]) and choose(2, [a,b])
        assert_eq!(exprs.len(), 2);
    }

    #[test]
    fn test_search_simple() {
        let a = Node::new('a');
        let b = Node::new('b');
        let c = Node::new('c');

        let config = SearchConfig {
            optimize: Objective::Load,
            resilience: 0, // Most single-node expressions have resilience 0
            read_fraction: Some(Distribution::Fixed(0.5)),
            timeout: Duration::from_secs(5),
            ..Default::default()
        };

        let nodes = vec![a, b, c];
        let result = search(&nodes, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_no_solution() {
        let a = Node::new('a');

        let config = SearchConfig {
            optimize: Objective::Load,
            resilience: 10, // Impossible resilience
            timeout: Duration::from_secs(1),
            ..Default::default()
        };

        let nodes = vec![a];
        let result = search(&nodes, &config);
        assert!(result.is_err());
    }
}
