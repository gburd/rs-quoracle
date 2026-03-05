//! Linear programming solver abstraction for optimization
//!
//! Provides LP-based algorithms used throughout the quoracle library:
//! - Minimum hitting set computation for resilience analysis
//! - Helper types for strategy optimization (used by `QuorumSystem`)

use crate::error::{Error, Result};
use good_lp::{
    constraint, default_solver, variable, Expression, ProblemVariables, Solution, SolverModel,
    Variable,
};
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

/// Compute the size of the minimum hitting set for a collection
/// of sets.
///
/// A hitting set is a set of elements that intersects every set in
/// the collection. The minimum hitting set is the smallest such set.
/// This is used to compute resilience: the resilience of an
/// expression is `min_hitting_set(quorums) - 1`.
///
/// Formulated as an integer linear program:
/// - Binary variable `x_i` for each distinct element
/// - For each set S: `sum(x_i for i in S) >= 1`
/// - Minimize `sum(x_i)`
///
/// Returns the size of the minimum hitting set (always >= 1 when
/// `sets` is non-empty).
///
/// # Errors
///
/// Returns `Error::LpError` if `sets` is empty or if the LP solver
/// fails to find a solution.
pub fn min_hitting_set<T, S>(sets: &[HashSet<T, S>]) -> Result<usize>
where
    T: Eq + std::hash::Hash + Clone + Ord,
    S: BuildHasher,
{
    if sets.is_empty() {
        return Err(Error::LpError(
            "cannot compute hitting set of empty collection".into(),
        ));
    }

    // Collect all distinct elements across all sets, sorted for
    // deterministic variable ordering.
    let mut all_elements: Vec<&T> = sets
        .iter()
        .flat_map(HashSet::iter)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    all_elements.sort();

    // Create one binary variable per element.
    let mut vars = ProblemVariables::new();
    let mut elem_to_var: HashMap<&T, Variable> = HashMap::new();
    for elem in &all_elements {
        let v = vars.add(variable().binary());
        elem_to_var.insert(*elem, v);
    }

    // Objective: minimize sum of all binary variables (i.e. the
    // number of selected elements).
    let objective: Expression = elem_to_var
        .values()
        .fold(Expression::default(), |acc, &v| acc + v);

    let mut problem = vars.minimise(objective).using(default_solver);

    // Constraint: each set must be "hit" by at least one selected
    // element.
    for set in sets {
        let hit_sum: Expression = set
            .iter()
            .filter_map(|elem| elem_to_var.get(elem))
            .fold(Expression::default(), |acc, &v| acc + v);
        problem = problem.with(hit_sum.geq(1));
    }

    let solution = problem
        .solve()
        .map_err(|e| Error::LpError(format!("min_hitting_set solver failed: {e}")))?;

    // Sum the solution values (each is 0.0 or 1.0 for binary vars).
    let count: f64 = elem_to_var.values().map(|v| solution.value(*v)).sum();

    // Round to nearest integer to handle floating-point imprecision.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let result = count.round() as usize;
    Ok(result)
}

/// What metric to optimize in the strategy LP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objective {
    /// Minimize the maximum load on any node.
    Load,
    /// Minimize the expected quorum size.
    Network,
    /// Minimize the expected quorum latency.
    Latency,
}

/// Configuration for the strategy optimization LP.
#[derive(Debug, Clone)]
pub struct StrategyLpConfig {
    /// Number of read quorums.
    pub n_read_quorums: usize,
    /// Number of write quorums.
    pub n_write_quorums: usize,
    /// For each node, the indices of read quorums containing it.
    pub node_read_quorum_indices: Vec<Vec<usize>>,
    /// For each node, the indices of write quorums containing it.
    pub node_write_quorum_indices: Vec<Vec<usize>>,
    /// Read capacity per node.
    pub read_capacities: Vec<f64>,
    /// Write capacity per node.
    pub write_capacities: Vec<f64>,
    /// Size of each read quorum.
    pub read_quorum_sizes: Vec<usize>,
    /// Size of each write quorum.
    pub write_quorum_sizes: Vec<usize>,
    /// Latency of each read quorum in seconds.
    pub read_quorum_latencies: Vec<f64>,
    /// Latency of each write quorum in seconds.
    pub write_quorum_latencies: Vec<f64>,
    /// Distribution of read fractions: `(fr, probability)` pairs.
    pub read_fraction: Vec<(f64, f64)>,
    /// What to optimize.
    pub optimize: Objective,
    /// Optional upper bound on load.
    pub load_limit: Option<f64>,
    /// Optional upper bound on network load.
    pub network_limit: Option<f64>,
    /// Optional upper bound on latency (seconds).
    pub latency_limit: Option<f64>,
}

/// Solution from the strategy optimization LP.
#[derive(Debug, Clone)]
pub struct StrategyLpSolution {
    /// Probability weights for each read quorum.
    pub read_weights: Vec<f64>,
    /// Probability weights for each write quorum.
    pub write_weights: Vec<f64>,
}

/// Build an LP to find the optimal strategy over quorum
/// probability distributions.
///
/// This is a helper for `QuorumSystem` strategy optimization.
/// It constructs the LP problem with variables for each quorum's
/// probability weight and returns the solved variable values.
///
/// # Errors
///
/// Returns `Error::NoStrategyFound` if the LP is infeasible (no
/// strategy satisfies the given constraints).
pub fn solve_strategy_lp(cfg: &StrategyLpConfig) -> Result<StrategyLpSolution> {
    let mut vars = ProblemVariables::new();

    let r_vars: Vec<Variable> = (0..cfg.n_read_quorums)
        .map(|_| vars.add(variable().min(0).max(1)))
        .collect();
    let w_vars: Vec<Variable> = (0..cfg.n_write_quorums)
        .map(|_| vars.add(variable().min(0).max(1)))
        .collect();

    let avg_fr: f64 = cfg.read_fraction.iter().map(|&(fr, p)| fr * p).sum();

    let network_expr = build_network_expr(&r_vars, &w_vars, cfg, avg_fr);
    let latency_expr = build_latency_expr(&r_vars, &w_vars, cfg, avg_fr);

    let mut load_cap_vars: Vec<(f64, f64, Variable)> = Vec::new();
    for &(fr, p) in &cfg.read_fraction {
        let l_var = vars.add(variable().min(0).max(1));
        load_cap_vars.push((fr, p, l_var));
    }

    let load_expr: Expression = load_cap_vars
        .iter()
        .fold(Expression::default(), |acc, &(_, p, l)| acc + p * l);

    let objective: Expression = match cfg.optimize {
        Objective::Load => load_expr.clone(),
        Objective::Network => network_expr.clone(),
        Objective::Latency => latency_expr.clone(),
    };

    let mut problem = vars.minimise(objective).using(default_solver);

    let r_sum: Expression = r_vars.iter().fold(Expression::default(), |acc, &v| acc + v);
    problem = problem.with(constraint!(r_sum == 1));

    let w_sum: Expression = w_vars.iter().fold(Expression::default(), |acc, &v| acc + v);
    problem = problem.with(constraint!(w_sum == 1));

    problem = add_load_constraints(problem, &load_cap_vars, &r_vars, &w_vars, cfg);

    if let Some(limit) = cfg.load_limit {
        problem = problem.with(load_expr.leq(limit));
    }
    if let Some(limit) = cfg.network_limit {
        problem = problem.with(network_expr.leq(limit));
    }
    if let Some(limit) = cfg.latency_limit {
        problem = problem.with(latency_expr.leq(limit));
    }

    let solution = problem.solve().map_err(|_| Error::NoStrategyFound)?;

    Ok(StrategyLpSolution {
        read_weights: r_vars.iter().map(|v| solution.value(*v)).collect(),
        write_weights: w_vars.iter().map(|v| solution.value(*v)).collect(),
    })
}

fn build_network_expr(
    r_vars: &[Variable],
    w_vars: &[Variable],
    cfg: &StrategyLpConfig,
    avg_fr: f64,
) -> Expression {
    let reads: Expression = r_vars
        .iter()
        .zip(&cfg.read_quorum_sizes)
        .fold(Expression::default(), |acc, (&v, &sz)| {
            acc + (avg_fr * to_f64(sz)) * v
        });
    let writes: Expression = w_vars
        .iter()
        .zip(&cfg.write_quorum_sizes)
        .fold(Expression::default(), |acc, (&v, &sz)| {
            acc + ((1.0 - avg_fr) * to_f64(sz)) * v
        });
    reads + writes
}

fn build_latency_expr(
    r_vars: &[Variable],
    w_vars: &[Variable],
    cfg: &StrategyLpConfig,
    avg_fr: f64,
) -> Expression {
    let reads: Expression = r_vars
        .iter()
        .zip(&cfg.read_quorum_latencies)
        .fold(Expression::default(), |acc, (&v, &lat)| {
            acc + (avg_fr * lat) * v
        });
    let writes: Expression = w_vars
        .iter()
        .zip(&cfg.write_quorum_latencies)
        .fold(Expression::default(), |acc, (&v, &lat)| {
            acc + ((1.0 - avg_fr) * lat) * v
        });
    reads + writes
}

/// The `good_lp` `SolverModel` type is not nameable directly, so
/// we use a generic parameter. This trait alias constrains it.
fn add_load_constraints<M: SolverModel>(
    mut problem: M,
    load_cap_vars: &[(f64, f64, Variable)],
    r_vars: &[Variable],
    w_vars: &[Variable],
    cfg: &StrategyLpConfig,
) -> M {
    for &(fr, _, l_var) in load_cap_vars {
        let fw = 1.0 - fr;
        for node_idx in 0..cfg.read_capacities.len() {
            let rcap = cfg.read_capacities[node_idx];
            let wcap = cfg.write_capacities[node_idx];

            let mut node_load = Expression::default();

            if rcap > 0.0 {
                for &rq_idx in &cfg.node_read_quorum_indices[node_idx] {
                    node_load += (fr / rcap) * r_vars[rq_idx];
                }
            }

            if wcap > 0.0 {
                for &wq_idx in &cfg.node_write_quorum_indices[node_idx] {
                    node_load += (fw / wcap) * w_vars[wq_idx];
                }
            }

            problem = problem.with(node_load.leq(l_var));
        }
    }
    problem
}

/// Convert `usize` to `f64`, accepting precision loss for quorum
/// sizes which are always small.
#[allow(clippy::cast_precision_loss)]
fn to_f64(n: usize) -> f64 {
    n as f64
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sets_from_vecs<T: Eq + std::hash::Hash + Clone + Ord>(vecs: &[Vec<T>]) -> Vec<HashSet<T>> {
        vecs.iter().map(|v| v.iter().cloned().collect()).collect()
    }

    #[test]
    fn hitting_set_single_element() {
        let sets = sets_from_vecs(&[vec!['a']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 1);
    }

    #[test]
    fn hitting_set_disjoint() {
        let sets = sets_from_vecs(&[vec!['a'], vec!['b']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 2);
    }

    #[test]
    fn hitting_set_overlapping() {
        let sets = sets_from_vecs(&[vec!['a', 'b'], vec!['b', 'c']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 1);
    }

    #[test]
    fn hitting_set_empty_input() {
        let sets: Vec<HashSet<char>> = vec![];
        assert!(min_hitting_set(&sets).is_err());
    }

    #[test]
    fn hitting_set_or_expression() {
        // Quorums of (a + b + c): {a}, {b}, {c}
        // Each singleton must be hit, so minimum is 3.
        let sets = sets_from_vecs(&[vec!['a'], vec!['b'], vec!['c']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 3);
    }

    #[test]
    fn hitting_set_and_expression() {
        // Quorums of (a * b * c): {a, b, c}
        // Only one set, so hitting set size = 1.
        let sets = sets_from_vecs(&[vec!['a', 'b', 'c']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 1);
    }

    #[test]
    fn hitting_set_grid_quorum() {
        // (a + b) * (c + d): {a,c}, {a,d}, {b,c}, {b,d}
        let sets = sets_from_vecs(&[
            vec!['a', 'c'],
            vec!['a', 'd'],
            vec!['b', 'c'],
            vec!['b', 'd'],
        ]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 2);
    }

    #[test]
    fn hitting_set_choose_2_of_3() {
        // choose(2, [a, b, c]): {a,b}, {a,c}, {b,c}
        let sets = sets_from_vecs(&[vec!['a', 'b'], vec!['a', 'c'], vec!['b', 'c']]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 2);
    }

    #[test]
    fn hitting_set_choose_2_of_5() {
        // choose(2, [a,b,c,d,e]): all pairs, need 4 to hit all.
        let elements = ['a', 'b', 'c', 'd', 'e'];
        let mut sets = Vec::new();
        for i in 0..elements.len() {
            for j in (i + 1)..elements.len() {
                sets.push(vec![elements[i], elements[j]]);
            }
        }
        let sets = sets_from_vecs(&sets);
        assert_eq!(min_hitting_set(&sets).unwrap(), 4);
    }

    #[test]
    fn hitting_set_resilience_values() {
        // resilience = min_hitting_set - 1

        // a + b: {a}, {b} => hitting=2, resilience=1
        let sets = sets_from_vecs(&[vec!['a'], vec!['b']]);
        assert_eq!(min_hitting_set(&sets).unwrap() - 1, 1);

        // a * b: {a,b} => hitting=1, resilience=0
        let sets = sets_from_vecs(&[vec!['a', 'b']]);
        assert_eq!(min_hitting_set(&sets).unwrap() - 1, 0);

        // (a + b) * (c + d) => hitting=2, resilience=1
        let sets = sets_from_vecs(&[
            vec!['a', 'c'],
            vec!['a', 'd'],
            vec!['b', 'c'],
            vec!['b', 'd'],
        ]);
        assert_eq!(min_hitting_set(&sets).unwrap() - 1, 1);
    }

    #[test]
    fn hitting_set_complex() {
        // a*b + b*c + a*d + a*d*e => {a,b}, {b,c}, {a,d}, {a,d,e}
        let sets = sets_from_vecs(&[
            vec!['a', 'b'],
            vec!['b', 'c'],
            vec!['a', 'd'],
            vec!['a', 'd', 'e'],
        ]);
        assert_eq!(min_hitting_set(&sets).unwrap(), 2);
    }

    #[test]
    fn strategy_lp_single_quorum() {
        // Single node, single quorum for reads and writes.
        let cfg = StrategyLpConfig {
            n_read_quorums: 1,
            n_write_quorums: 1,
            node_read_quorum_indices: vec![vec![0]],
            node_write_quorum_indices: vec![vec![0]],
            read_capacities: vec![1.0],
            write_capacities: vec![1.0],
            read_quorum_sizes: vec![1],
            write_quorum_sizes: vec![1],
            read_quorum_latencies: vec![1.0],
            write_quorum_latencies: vec![1.0],
            read_fraction: vec![(0.5, 1.0)],
            optimize: Objective::Load,
            load_limit: None,
            network_limit: None,
            latency_limit: None,
        };
        let sol = solve_strategy_lp(&cfg).unwrap();
        assert!((sol.read_weights[0] - 1.0).abs() < 1e-6);
        assert!((sol.write_weights[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn strategy_lp_grid_uniform() {
        // Grid: reads = {a,b}, {c,d}
        //        writes = {a,c}, {a,d}, {b,c}, {b,d}
        let cfg = StrategyLpConfig {
            n_read_quorums: 2,
            n_write_quorums: 4,
            node_read_quorum_indices: vec![
                vec![0], // a in rq0
                vec![0], // b in rq0
                vec![1], // c in rq1
                vec![1], // d in rq1
            ],
            node_write_quorum_indices: vec![
                vec![0, 1], // a in wq0, wq1
                vec![2, 3], // b in wq2, wq3
                vec![0, 2], // c in wq0, wq2
                vec![1, 3], // d in wq1, wq3
            ],
            read_capacities: vec![1.0, 1.0, 1.0, 1.0],
            write_capacities: vec![1.0, 1.0, 1.0, 1.0],
            read_quorum_sizes: vec![2, 2],
            write_quorum_sizes: vec![2, 2, 2, 2],
            read_quorum_latencies: vec![1.0, 1.0],
            write_quorum_latencies: vec![1.0, 1.0, 1.0, 1.0],
            read_fraction: vec![(0.5, 1.0)],
            optimize: Objective::Load,
            load_limit: None,
            network_limit: None,
            latency_limit: None,
        };
        let sol = solve_strategy_lp(&cfg).unwrap();

        let r_sum: f64 = sol.read_weights.iter().sum();
        let w_sum: f64 = sol.write_weights.iter().sum();
        assert!((r_sum - 1.0).abs() < 1e-6);
        assert!((w_sum - 1.0).abs() < 1e-6);

        // Symmetric system => reads must be 0.5/0.5 (only two
        // read quorums). Write weights can vary as long as the
        // per-node load is balanced.
        assert!((sol.read_weights[0] - 0.5).abs() < 1e-6);
        assert!((sol.read_weights[1] - 0.5).abs() < 1e-6);

        // Verify all write weights are non-negative and sum to 1.
        for &w in &sol.write_weights {
            assert!(w >= -1e-10);
        }

        // Verify per-node load is balanced: each node should have
        // the same load under the optimal solution. For fr=0.5 with
        // uniform capacities, node load = 0.5 * (sum of read quorum
        // vars containing it) + 0.5 * (sum of write quorum vars
        // containing it). By symmetry, optimal load = 0.5.
        let node_loads: Vec<f64> = (0..4)
            .map(|i| {
                let r_load: f64 = cfg.node_read_quorum_indices[i]
                    .iter()
                    .map(|&j| sol.read_weights[j])
                    .sum();
                let w_load: f64 = cfg.node_write_quorum_indices[i]
                    .iter()
                    .map(|&j| sol.write_weights[j])
                    .sum();
                0.5 * r_load + 0.5 * w_load
            })
            .collect();
        let max_load = node_loads.iter().copied().fold(0.0_f64, f64::max);
        assert!(
            (max_load - 0.5).abs() < 1e-6,
            "optimal load should be 0.5, got {max_load}"
        );
    }
}
