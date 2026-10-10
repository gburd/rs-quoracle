//! Read-write quorum systems and strategies.
//!
//! A [`QuorumSystem`] pairs a read expression with a write expression such
//! that every read quorum intersects every write quorum. A [`Strategy`] is a
//! probability distribution over read quorums and over write quorums; it
//! determines per-node load, capacity, network cost, and latency.

use crate::distribution::{self, Canonical, Distribution};
use crate::error::{Error, Result};
use crate::expr::{minimize, Element, Expr, Node};
use good_lp::{
    default_solver, variable, Expression, ProblemVariables, Solution,
    SolverModel, Variable,
};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use rand::seq::IndexedRandom;
use std::collections::BTreeMap;
use std::time::Duration;

/// Optimization objective for strategy computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objective {
    /// Minimize the load of the busiest node (maximizes capacity).
    Load,
    /// Minimize the expected number of nodes contacted per operation.
    Network,
    /// Minimize the expected operation latency.
    Latency,
}

/// Optional upper bounds for strategy optimization.
///
/// The bound matching the objective being optimized must be `None`.
#[derive(Debug, Clone, Copy, Default)]
pub struct StrategyLimits {
    /// Maximum load (see [`Strategy::load`]).
    pub load: Option<f64>,
    /// Maximum network load (see [`Strategy::network_load`]).
    pub network: Option<f64>,
    /// Maximum latency (see [`Strategy::latency`]).
    pub latency: Option<Duration>,
}

impl StrategyLimits {
    /// Reject a limit on the metric being optimized.
    pub(crate) fn check(&self, objective: Objective) -> Result<()> {
        let conflict = match objective {
            Objective::Load => self.load.is_some(),
            Objective::Network => self.network.is_some(),
            Objective::Latency => self.latency.is_some(),
        };
        if conflict {
            return Err(Error::InvalidQuorumSystem(format!(
                "a {objective:?} limit cannot be set when optimizing for \
                 {objective:?}"
            )));
        }
        Ok(())
    }
}

type Quorums<T> = Vec<HashSet<T>>;

/// A quorum as a sorted, de-duplicated list of node identifiers.
pub type Quorum<T> = Vec<T>;

fn to_quorum<T: Element>(set: &HashSet<T>) -> Quorum<T> {
    let mut vec: Vec<T> = set.iter().cloned().collect();
    vec.sort();
    vec
}

fn to_set<T: Element>(quorum: &[T]) -> HashSet<T> {
    quorum.iter().cloned().collect()
}

#[expect(clippy::cast_precision_loss)]
fn len_f64(n: usize) -> f64 {
    n as f64
}

/// A read-write quorum system.
#[derive(Debug, Clone)]
pub struct QuorumSystem<T: Element> {
    reads: Expr<T>,
    writes: Expr<T>,
    x_to_node: HashMap<T, Node<T>>,
}

impl<T: Element> QuorumSystem<T> {
    /// Build a quorum system from reads only; writes are the dual.
    pub fn from_reads(reads: Expr<T>) -> Self {
        let writes = reads.dual();
        Self::build(reads, writes)
    }

    /// Build a quorum system from writes only; reads are the dual.
    pub fn from_writes(writes: Expr<T>) -> Self {
        let reads = writes.dual();
        Self::build(reads, writes)
    }

    /// Build a quorum system from both read and write expressions.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NonOverlappingQuorums`] unless every read quorum
    /// intersects every write quorum.
    pub fn new(reads: Expr<T>, writes: Expr<T>) -> Result<Self> {
        let optimal_writes = reads.dual();
        if !writes.quorums().all(|wq| optimal_writes.is_quorum(&wq)) {
            return Err(Error::NonOverlappingQuorums);
        }
        Ok(Self::build(reads, writes))
    }

    fn build(reads: Expr<T>, writes: Expr<T>) -> Self {
        let mut x_to_node = HashMap::new();
        for node in reads.nodes().into_iter().chain(writes.nodes()) {
            x_to_node.entry(node.x().clone()).or_insert(node);
        }
        Self { reads, writes, x_to_node }
    }

    /// The read expression.
    #[must_use]
    pub fn reads(&self) -> &Expr<T> {
        &self.reads
    }

    /// The write expression.
    #[must_use]
    pub fn writes(&self) -> &Expr<T> {
        &self.writes
    }

    /// Iterate over all read quorums.
    pub fn read_quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        self.reads.quorums()
    }

    /// Iterate over all write quorums.
    pub fn write_quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        self.writes.quorums()
    }

    /// Whether `xs` contains a read quorum.
    #[must_use]
    pub fn is_read_quorum(&self, xs: &HashSet<T>) -> bool {
        self.reads.is_quorum(xs)
    }

    /// Whether `xs` contains a write quorum.
    #[must_use]
    pub fn is_write_quorum(&self, xs: &HashSet<T>) -> bool {
        self.writes.is_quorum(xs)
    }

    /// Look up a node by its identifier.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidQuorumSystem`] if `x` is not in the system.
    pub fn node(&self, x: &T) -> Result<&Node<T>> {
        self.x_to_node.get(x).ok_or_else(|| {
            Error::InvalidQuorumSystem(format!(
                "element {x} not found in quorum system"
            ))
        })
    }

    /// All nodes in the system.
    #[must_use]
    pub fn nodes(&self) -> HashSet<Node<T>> {
        self.x_to_node.values().cloned().collect()
    }

    /// All node identifiers in the system.
    #[must_use]
    pub fn elements(&self) -> HashSet<T> {
        self.x_to_node.keys().cloned().collect()
    }

    /// The resilience of the system: the minimum of read and write
    /// resilience.
    #[must_use]
    pub fn resilience(&self) -> usize {
        self.read_resilience().min(self.write_resilience())
    }

    /// The resilience of the read expression.
    #[must_use]
    pub fn read_resilience(&self) -> usize {
        self.reads.resilience()
    }

    /// The resilience of the write expression.
    #[must_use]
    pub fn write_resilience(&self) -> usize {
        self.writes.resilience()
    }

    /// Whether both read and write expressions are duplicate-free.
    #[must_use]
    pub fn dup_free(&self) -> bool {
        self.reads.dup_free() && self.writes.dup_free()
    }

    /// Read and write candidates excluding known failures, with resilience
    /// to `f` further failures.
    fn candidate_quorums(
        &self,
        f: usize,
        failed: &HashSet<T>,
    ) -> Result<(Quorums<T>, Quorums<T>)> {
        for x in failed {
            self.node(x)?;
        }
        let (rq, wq) = if f == 0 {
            (
                minimize(
                    self.read_quorums()
                        .filter(|q| q.is_disjoint(failed))
                        .collect(),
                ),
                minimize(
                    self.write_quorums()
                        .filter(|q| q.is_disjoint(failed))
                        .collect(),
                ),
            )
        } else {
            let mut xs: Vec<T> = self
                .elements()
                .into_iter()
                .filter(|x| !failed.contains(x))
                .collect();
            xs.sort();
            (
                f_resilient_quorums(f, &xs, &self.reads),
                f_resilient_quorums(f, &xs, &self.writes),
            )
        };
        if rq.is_empty() || wq.is_empty() {
            return Err(Error::NoStrategyFound);
        }
        Ok((rq, wq))
    }

    /// A strategy that picks uniformly among the minimal `f`-resilient
    /// quorums.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoStrategyFound`] if there are no `f`-resilient read
    /// or write quorums.
    pub fn uniform_strategy(&self, f: usize) -> Result<Strategy<T>> {
        let (rq, wq) = self.candidate_quorums(f, &HashSet::new())?;
        let uniform = |qs: Vec<HashSet<T>>| -> BTreeMap<Quorum<T>, f64> {
            let p = 1.0 / len_f64(qs.len());
            qs.iter().map(|q| (to_quorum(q), p)).collect()
        };
        Ok(Strategy::new(self, uniform(rq), uniform(wq)))
    }

    /// Build a strategy from explicit quorum weights.
    ///
    /// Each key is a list of node identifiers (order and duplicates do not
    /// matter); weights for the same set are added together, and weights
    /// are normalized to sum to 1. Zero-weight quorums are dropped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidQuorumSystem`] if a key is not a read (or
    /// write) quorum, a weight is negative or not finite, or all weights are
    /// zero.
    pub fn make_strategy(
        &self,
        sigma_r: BTreeMap<Quorum<T>, f64>,
        sigma_w: BTreeMap<Quorum<T>, f64>,
    ) -> Result<Strategy<T>> {
        let r = normalize("sigma_r", sigma_r, |q| self.is_read_quorum(q))?;
        let w = normalize("sigma_w", sigma_w, |q| self.is_write_quorum(q))?;
        Ok(Strategy::new(self, r, w))
    }

    /// Compute the optimal strategy by linear programming.
    ///
    /// Minimizes `objective` subject to `limits`, considering only
    /// `f`-resilient quorums (quorums that still contain a quorum after any
    /// `f` of their nodes fail). Exactly one of `read_fraction` and
    /// `write_fraction` must be `Some`.
    /// To exclude known failed nodes, use [`Self::strategy_with_failures`].
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidQuorumSystem`] if the limit matching `objective`
    ///   is set.
    /// - [`Error::InvalidDistribution`] for a bad read/write fraction.
    /// - [`Error::NoStrategyFound`] if the limits cannot be met or there
    ///   are no `f`-resilient quorums.
    /// - [`Error::LpError`] if the solver fails for another reason.
    pub fn strategy(
        &self,
        objective: Objective,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
        limits: &StrategyLimits,
        f: usize,
    ) -> Result<Strategy<T>> {
        self.strategy_with_failures(
            objective,
            read_fraction,
            write_fraction,
            limits,
            f,
            &HashSet::new(),
        )
    }

    /// Compute the optimal strategy excluding known failed nodes.
    ///
    /// Like [`Self::strategy`], but no selected quorum contacts a node in
    /// `failed`. The original read and write rules, node capacities, and
    /// latencies are preserved. `f` counts further failures among the
    /// selected nodes; known failures do not consume this allowance.
    /// An empty `failed` set is equivalent to [`Self::strategy`].
    ///
    /// Exactly one of `read_fraction` and `write_fraction` must be `Some`.
    /// Recompute the strategy when the known failed set changes.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidQuorumSystem`] if `failed` contains an unknown
    ///   node or the limit matching `objective` is set.
    /// - [`Error::InvalidDistribution`] for a bad read/write fraction.
    /// - [`Error::NoStrategyFound`] if the limits cannot be met or there
    ///   are no surviving `f`-resilient read or write quorums.
    /// - [`Error::LpError`] if the solver fails for another reason.
    pub fn strategy_with_failures(
        &self,
        objective: Objective,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
        limits: &StrategyLimits,
        f: usize,
        failed: &HashSet<T>,
    ) -> Result<Strategy<T>> {
        limits.check(objective)?;
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let (rq, wq) = self.candidate_quorums(f, failed)?;
        self.lp_optimal_strategy(&rq, &wq, &d, objective, limits)
    }

    /// Latency of a quorum: the time until enough of its nodes (fastest
    /// first) have responded to form a quorum of `expr`.
    fn quorum_latency(&self, quorum: &HashSet<T>, expr: &Expr<T>) -> Duration {
        quorum_latency(&self.x_to_node, quorum, expr)
    }

    fn lp_optimal_strategy(
        &self,
        read_quorums: &[HashSet<T>],
        write_quorums: &[HashSet<T>],
        read_fraction: &Canonical,
        objective: Objective,
        limits: &StrategyLimits,
    ) -> Result<Strategy<T>> {
        let mut vars = ProblemVariables::new();
        let r: Vec<Variable> = read_quorums
            .iter()
            .map(|_| vars.add(variable().min(0.0).max(1.0)))
            .collect();
        let w: Vec<Variable> = write_quorums
            .iter()
            .map(|_| vars.add(variable().min(0.0).max(1.0)))
            .collect();
        // One load variable per read fraction: the max node load at that fr.
        let loads: Vec<(f64, f64, Variable)> = read_fraction
            .iter()
            .map(|(fr, &p)| (fr.0, p, vars.add(variable().min(0.0))))
            .collect();

        let avg_fr: f64 = read_fraction.iter().map(|(k, &p)| k.0 * p).sum();
        let weighted = |coeffs: &[f64], vs: &[Variable]| -> Expression {
            coeffs.iter().zip(vs).map(|(&c, &v)| c * v).sum()
        };
        let rq_sizes: Vec<f64> =
            read_quorums.iter().map(|q| len_f64(q.len())).collect();
        let wq_sizes: Vec<f64> =
            write_quorums.iter().map(|q| len_f64(q.len())).collect();
        let network = avg_fr * weighted(&rq_sizes, &r)
            + (1.0 - avg_fr) * weighted(&wq_sizes, &w);
        let latency = {
            let rl: Vec<f64> = read_quorums
                .iter()
                .map(|q| self.quorum_latency(q, &self.reads).as_secs_f64())
                .collect();
            let wl: Vec<f64> = write_quorums
                .iter()
                .map(|q| self.quorum_latency(q, &self.writes).as_secs_f64())
                .collect();
            avg_fr * weighted(&rl, &r) + (1.0 - avg_fr) * weighted(&wl, &w)
        };
        let load: Expression = loads.iter().map(|&(_, p, l)| p * l).sum();

        let objective_expr = match objective {
            Objective::Load => load.clone(),
            Objective::Network => network.clone(),
            Objective::Latency => latency.clone(),
        };
        let mut problem = vars.minimise(objective_expr).using(default_solver);
        let r_sum: Expression = r.iter().copied().sum();
        let w_sum: Expression = w.iter().copied().sum();
        problem = problem.with(r_sum.eq(1.0)).with(w_sum.eq(1.0));

        // Per-node load at each read fraction is at most that fraction's
        // load variable.
        let mut x_to_r: HashMap<&T, Vec<Variable>> = HashMap::new();
        for (q, &v) in read_quorums.iter().zip(&r) {
            for x in q {
                x_to_r.entry(x).or_default().push(v);
            }
        }
        let mut x_to_w: HashMap<&T, Vec<Variable>> = HashMap::new();
        for (q, &v) in write_quorums.iter().zip(&w) {
            for x in q {
                x_to_w.entry(x).or_default().push(v);
            }
        }
        for &(fr, _, l) in &loads {
            for node in self.x_to_node.values() {
                let sum = |m: &HashMap<&T, Vec<Variable>>| -> Expression {
                    m.get(node.x()).into_iter().flatten().copied().sum()
                };
                let node_load = (fr / node.read_capacity()) * sum(&x_to_r)
                    + ((1.0 - fr) / node.write_capacity()) * sum(&x_to_w);
                problem = problem.with(node_load.leq(l));
            }
        }

        if let Some(limit) = limits.load {
            problem = problem.with(load.leq(limit));
        }
        if let Some(limit) = limits.network {
            problem = problem.with(network.leq(limit));
        }
        if let Some(limit) = limits.latency {
            problem = problem.with(latency.leq(limit.as_secs_f64()));
        }

        let solution = problem.solve()?;
        let pick = |qs: &[HashSet<T>], vs: &[Variable]| {
            qs.iter()
                .zip(vs)
                .map(|(q, &v)| (to_quorum(q), solution.value(v)))
                .filter(|&(_, p)| p > 1e-10)
                .collect::<BTreeMap<_, _>>()
        };
        let sigma_r = pick(read_quorums, &r);
        let sigma_w = pick(write_quorums, &w);
        // LP solutions satisfy sum == 1 only up to solver tolerance.
        let renorm = |m: BTreeMap<Quorum<T>, f64>| {
            let total: f64 = m.values().sum();
            m.into_iter().map(|(q, p)| (q, p / total)).collect()
        };
        Ok(Strategy::new(self, renorm(sigma_r), renorm(sigma_w)))
    }
}

/// Validate, merge, and normalize user-supplied quorum weights.
fn normalize<T: Element>(
    name: &str,
    sigma: BTreeMap<Quorum<T>, f64>,
    is_quorum: impl Fn(&HashSet<T>) -> bool,
) -> Result<BTreeMap<Quorum<T>, f64>> {
    let invalid =
        |msg: &str| Error::InvalidQuorumSystem(format!("{name} {msg}"));
    let mut merged: BTreeMap<Quorum<T>, f64> = BTreeMap::new();
    for (q, weight) in sigma {
        if !weight.is_finite() || weight < 0.0 {
            return Err(invalid("has negative or non-finite weights"));
        }
        let set = to_set(&q);
        if !is_quorum(&set) {
            return Err(invalid(&format!(
                "has non-quorum {:?}",
                to_quorum(&set)
            )));
        }
        *merged.entry(to_quorum(&set)).or_default() += weight;
    }
    let total: f64 = merged.values().sum();
    if !(total.is_finite() && total > 0.0) {
        return Err(invalid("must have a positive total weight"));
    }
    Ok(merged
        .into_iter()
        .filter(|&(_, w)| w > 0.0)
        .map(|(q, w)| (q, w / total))
        .collect())
}

/// All minimal sets over `xs` that remain quorums of `expr` after any `f`
/// of their elements fail.
fn f_resilient_quorums<T: Element>(
    f: usize,
    xs: &[T],
    expr: &Expr<T>,
) -> Vec<HashSet<T>> {
    let mut results = Vec::new();
    f_resilient_helper(f, xs, expr, &mut HashSet::new(), 0, &mut results);
    minimize(results)
}

fn f_resilient_helper<T: Element>(
    f: usize,
    xs: &[T],
    expr: &Expr<T>,
    current: &mut HashSet<T>,
    start: usize,
    results: &mut Vec<HashSet<T>>,
) {
    let resilient =
        current.iter().combinations(f.min(current.len())).all(|failed| {
            let alive: HashSet<T> = current
                .iter()
                .filter(|x| !failed.contains(x))
                .cloned()
                .collect();
            expr.is_quorum(&alive)
        });
    if resilient {
        results.push(current.clone());
        return;
    }
    for j in start..xs.len() {
        current.insert(xs[j].clone());
        f_resilient_helper(f, xs, expr, current, j + 1, results);
        current.remove(&xs[j]);
    }
}

fn quorum_latency<T: Element>(
    x_to_node: &HashMap<T, Node<T>>,
    quorum: &HashSet<T>,
    expr: &Expr<T>,
) -> Duration {
    let mut nodes: Vec<&Node<T>> =
        quorum.iter().filter_map(|x| x_to_node.get(x)).collect();
    nodes.sort_by_key(|n| n.latency());
    let mut seen = HashSet::new();
    for node in nodes {
        seen.insert(node.x().clone());
        if expr.is_quorum(&seen) {
            return node.latency();
        }
    }
    // Unreachable for quorums of `expr`; strategies only contain quorums.
    Duration::ZERO
}

impl<T: Element> std::fmt::Display for QuorumSystem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuorumSystem(reads={}, writes={})", self.reads, self.writes)
    }
}

/// A probability distribution over read quorums and over write quorums.
///
/// Built by [`QuorumSystem::strategy`],
/// [`QuorumSystem::strategy_with_failures`],
/// [`QuorumSystem::uniform_strategy`], or [`QuorumSystem::make_strategy`];
/// probabilities always sum to 1.
#[derive(Debug, Clone)]
pub struct Strategy<T: Element> {
    sigma_r: BTreeMap<Quorum<T>, f64>,
    sigma_w: BTreeMap<Quorum<T>, f64>,
    /// Per-node: (node, P(node in read quorum), P(node in write quorum)).
    node_probs: Vec<(Node<T>, f64, f64)>,
    qs: QuorumSystem<T>,
}

impl<T: Element> Strategy<T> {
    fn new(
        qs: &QuorumSystem<T>,
        sigma_r: BTreeMap<Quorum<T>, f64>,
        sigma_w: BTreeMap<Quorum<T>, f64>,
    ) -> Self {
        let mut x_read: HashMap<T, f64> = HashMap::new();
        for (q, &p) in &sigma_r {
            for x in q {
                *x_read.entry_ref(x).or_default() += p;
            }
        }
        let mut x_write: HashMap<T, f64> = HashMap::new();
        for (q, &p) in &sigma_w {
            for x in q {
                *x_write.entry_ref(x).or_default() += p;
            }
        }
        let mut node_probs: Vec<(Node<T>, f64, f64)> = qs
            .x_to_node
            .values()
            .map(|n| {
                let rp = x_read.get(n.x()).copied().unwrap_or(0.0);
                let wp = x_write.get(n.x()).copied().unwrap_or(0.0);
                (n.clone(), rp, wp)
            })
            .collect();
        node_probs.sort_by(|a, b| a.0.cmp(&b.0));
        Self { sigma_r, sigma_w, node_probs, qs: qs.clone() }
    }

    /// The quorum system this strategy is for.
    #[must_use]
    pub fn quorum_system(&self) -> &QuorumSystem<T> {
        &self.qs
    }

    /// Read quorum probabilities (they sum to 1).
    #[must_use]
    pub fn sigma_r(&self) -> &BTreeMap<Quorum<T>, f64> {
        &self.sigma_r
    }

    /// Write quorum probabilities (they sum to 1).
    #[must_use]
    pub fn sigma_w(&self) -> &BTreeMap<Quorum<T>, f64> {
        &self.sigma_w
    }

    /// Sample a read quorum according to the strategy.
    #[must_use]
    pub fn get_read_quorum(&self) -> HashSet<T> {
        sample_quorum(&self.sigma_r)
    }

    /// Sample a write quorum according to the strategy.
    #[must_use]
    pub fn get_write_quorum(&self) -> HashSet<T> {
        sample_quorum(&self.sigma_w)
    }

    /// Expected load of the busiest node, in fractions of its capacity
    /// per operation. Capacity is `1 / load`.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn load(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter().map(|(fr, &p)| p * self.load_at(fr.0)).sum())
    }

    /// Expected capacity: operations per unit time the system can serve
    /// before its busiest node saturates (`1 / load` at each fraction).
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn capacity(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter().map(|(fr, &p)| p / self.load_at(fr.0)).sum())
    }

    /// Expected number of nodes contacted per operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn network_load(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let fr = avg_read_fraction(read_fraction, write_fraction)?;
        let expected_size = |sigma: &BTreeMap<Quorum<T>, f64>| -> f64 {
            sigma.iter().map(|(q, &p)| p * len_f64(q.len())).sum()
        };
        Ok(fr * expected_size(&self.sigma_r)
            + (1.0 - fr) * expected_size(&self.sigma_w))
    }

    /// Expected operation latency: for each quorum, the time until its
    /// fastest nodes form a quorum, weighted by the strategy.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn latency(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<Duration> {
        let fr = avg_read_fraction(read_fraction, write_fraction)?;
        let expected = |sigma: &BTreeMap<Quorum<T>, f64>, e: &Expr<T>| -> f64 {
            sigma
                .iter()
                .map(|(q, &p)| {
                    p * self.qs.quorum_latency(&to_set(q), e).as_secs_f64()
                })
                .sum()
        };
        let secs = fr * expected(&self.sigma_r, &self.qs.reads)
            + (1.0 - fr) * expected(&self.sigma_w, &self.qs.writes);
        Ok(Duration::from_secs_f64(secs))
    }

    fn probs(&self, node: &Node<T>) -> (f64, f64) {
        self.node_probs
            .binary_search_by(|(n, _, _)| n.cmp(node))
            .map_or((0.0, 0.0), |i| {
                (self.node_probs[i].1, self.node_probs[i].2)
            })
    }

    /// Expected load on `node`.
    ///
    /// Capacities come from the quorum system's copy of the node, so
    /// passing `Node::new(id)` works. Nodes not in the system have load 0.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn node_load(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let node = self.qs.x_to_node.get(node.x()).unwrap_or(node);
        Ok(d.iter().map(|(fr, &p)| p * self.node_load_at(node, fr.0)).sum())
    }

    /// Utilization of `node`: its load relative to the busiest node.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn node_utilization(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let node = self.qs.x_to_node.get(node.x()).unwrap_or(node);
        Ok(d.iter()
            .map(|(fr, &p)| {
                p * self.node_load_at(node, fr.0) / self.load_at(fr.0)
            })
            .sum())
    }

    /// Requests per unit time handled by `node` when the system runs at
    /// full capacity.
    ///
    /// # Errors
    ///
    /// Returns an error if the distribution is invalid.
    pub fn node_throughput(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let (rp, wp) = self.probs(node);
        Ok(d.iter()
            .map(|(fr, &p)| {
                let cap = 1.0 / self.load_at(fr.0);
                p * cap * (fr.0 * rp + (1.0 - fr.0) * wp)
            })
            .sum())
    }

    fn load_at(&self, fr: f64) -> f64 {
        self.node_probs
            .iter()
            .map(|(n, rp, wp)| node_load(n, *rp, *wp, fr))
            .fold(0.0_f64, f64::max)
    }

    fn node_load_at(&self, node: &Node<T>, fr: f64) -> f64 {
        let (rp, wp) = self.probs(node);
        node_load(node, rp, wp, fr)
    }
}

fn node_load<T: Element>(node: &Node<T>, rp: f64, wp: f64, fr: f64) -> f64 {
    fr * rp / node.read_capacity() + (1.0 - fr) * wp / node.write_capacity()
}

fn avg_read_fraction(
    read_fraction: Option<&Distribution>,
    write_fraction: Option<&Distribution>,
) -> Result<f64> {
    let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
    Ok(d.iter().map(|(k, &p)| k.0 * p).sum())
}

impl<T: Element> std::fmt::Display for Strategy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let show = |sigma: &BTreeMap<Quorum<T>, f64>| -> String {
            sigma
                .iter()
                .map(|(q, p)| format!("{{{}}}: {p:.4}", q.iter().join(", ")))
                .join(", ")
        };
        write!(
            f,
            "Strategy(reads=[{}], writes=[{}])",
            show(&self.sigma_r),
            show(&self.sigma_w)
        )
    }
}

/// Sample a quorum. Strategies are never empty and weights are positive,
/// so this always returns a quorum.
fn sample_quorum<T: Element>(sigma: &BTreeMap<Quorum<T>, f64>) -> HashSet<T> {
    let entries: Vec<(&Quorum<T>, &f64)> = sigma.iter().collect();
    entries
        .choose_weighted(&mut rand::rng(), |(_, &w)| w)
        .map(|(q, _)| to_set(q))
        .unwrap_or_default()
}

#[cfg(test)]
#[expect(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn n(x: &str) -> Expr<String> {
        Expr::Node(Node::new(x.to_string()))
    }

    fn node(x: &str) -> Node<String> {
        Node::new(x.to_string())
    }

    fn node_with(x: &str, rc: f64, wc: f64, lat: u64) -> Node<String> {
        Node::new(x.to_string())
            .with_read_write_capacity(rc, wc)
            .expect("valid capacity")
            .with_latency(Duration::from_secs(lat))
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn quorum(items: &[&str]) -> Quorum<String> {
        let mut v: Vec<String> =
            items.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v
    }

    fn quorum_set(
        qs: impl Iterator<Item = HashSet<String>>,
    ) -> HashSet<Vec<String>> {
        qs.map(|q| {
            let mut v: Vec<String> = q.into_iter().collect();
            v.sort();
            v
        })
        .collect()
    }

    // -- Constructor tests --

    #[test]
    fn from_reads_generates_dual_writes() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let r = quorum_set(qs.read_quorums());
        let w = quorum_set(qs.write_quorums());
        assert!(r.contains(&vec!["a".to_string()]));
        assert!(r.contains(&vec!["b".to_string()]));
        assert!(w.contains(&vec!["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn from_writes_generates_dual_reads() {
        let qs = QuorumSystem::from_writes(n("a") + n("b"));
        let r = quorum_set(qs.read_quorums());
        let w = quorum_set(qs.write_quorums());
        assert!(w.contains(&vec!["a".to_string()]));
        assert!(w.contains(&vec!["b".to_string()]));
        assert!(r.contains(&vec!["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn new_with_valid_overlap() {
        let qs = QuorumSystem::new(n("a") + n("b"), n("a") * n("b") * n("c"));
        assert!(qs.is_ok());
    }

    #[test]
    fn new_with_no_overlap_fails() {
        let qs = QuorumSystem::new(n("a") + n("b"), n("a"));
        assert_eq!(qs.unwrap_err(), Error::NonOverlappingQuorums);
    }

    // -- Basic methods --

    #[test]
    fn elements_returns_all() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let elems = qs.elements();
        assert!(elems.contains("a"));
        assert!(elems.contains("b"));
    }

    #[test]
    fn nodes_returns_all() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let nodes = qs.nodes();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn is_read_quorum_and_is_write_quorum() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        assert!(qs.is_read_quorum(&set(&["a"])));
        assert!(qs.is_read_quorum(&set(&["b"])));
        assert!(!qs.is_read_quorum(&set(&["c"])));
        assert!(qs.is_write_quorum(&set(&["a", "b"])));
        assert!(!qs.is_write_quorum(&set(&["a"])));
    }

    // -- Resilience --

    #[test]
    fn resilience_simple() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        assert_eq!(qs.resilience(), 0);
        assert_eq!(qs.read_resilience(), 1);
        assert_eq!(qs.write_resilience(), 0);
    }

    #[test]
    fn dup_free_check() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        assert!(qs.dup_free());
    }

    // -- uniform_strategy --

    #[test]
    fn uniform_strategy_single_node() {
        let qs = QuorumSystem::from_reads(n("a"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r().len(), 1);
        assert!((sigma.sigma_r()[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
        assert_eq!(sigma.sigma_w().len(), 1);
        assert!((sigma.sigma_w()[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn uniform_strategy_two_nodes() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r().len(), 2);
        assert!((sigma.sigma_r()[&quorum(&["a"])] - 0.5).abs() < 1e-10);
        assert!((sigma.sigma_r()[&quorum(&["b"])] - 0.5).abs() < 1e-10);
        assert_eq!(sigma.sigma_w().len(), 1);
        assert!(
            (sigma.sigma_w()[&quorum(&["a", "b"])] - 1.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn uniform_strategy_grid() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r().len(), 2);
        assert!((sigma.sigma_r()[&quorum(&["a", "b"])] - 0.5).abs() < 1e-10);
        assert!((sigma.sigma_r()[&quorum(&["c", "d"])] - 0.5).abs() < 1e-10);
        assert_eq!(sigma.sigma_w().len(), 4);
        for wq in &[
            quorum(&["a", "c"]),
            quorum(&["a", "d"]),
            quorum(&["b", "c"]),
            quorum(&["b", "d"]),
        ] {
            assert!((sigma.sigma_w()[wq] - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn uniform_strategy_minimizes() {
        // a + a*b should reduce to just {a}
        let qs = QuorumSystem::from_reads(n("a") + n("a") * n("b"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r().len(), 1);
        assert!((sigma.sigma_r()[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
    }

    // -- make_strategy --

    #[test]
    fn make_strategy_normalizes() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a", "b"]), 25.0);
        sigma_r.insert(quorum(&["c", "d"]), 75.0);
        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 1.0);
        sigma_w.insert(quorum(&["a", "d"]), 1.0);
        sigma_w.insert(quorum(&["b", "c"]), 1.0);
        sigma_w.insert(quorum(&["b", "d"]), 1.0);

        let sigma = qs.make_strategy(sigma_r, sigma_w).expect("ok");
        assert!((sigma.sigma_r()[&quorum(&["a", "b"])] - 0.25).abs() < 1e-10);
        assert!((sigma.sigma_r()[&quorum(&["c", "d"])] - 0.75).abs() < 1e-10);
        for wq in &[
            quorum(&["a", "c"]),
            quorum(&["a", "d"]),
            quorum(&["b", "c"]),
            quorum(&["b", "d"]),
        ] {
            assert!((sigma.sigma_w()[wq] - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn make_strategy_negative_weights_fail() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a", "b"]), -1.0);
        sigma_r.insert(quorum(&["c", "d"]), 1.0);
        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 1.0);

        assert!(qs.make_strategy(sigma_r, sigma_w).is_err());
    }

    #[test]
    fn make_strategy_non_quorum_fails() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a"]), 1.0); // not a read quorum
        sigma_r.insert(quorum(&["c", "d"]), 1.0);
        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 1.0);

        assert!(qs.make_strategy(sigma_r, sigma_w).is_err());
    }

    // -- Strategy load/capacity tests --

    #[test]
    fn strategy_load_and_capacity() {
        let a = node_with("a", 50.0, 10.0, 1);
        let b = node_with("b", 60.0, 20.0, 2);
        let c = node_with("c", 70.0, 30.0, 3);
        let d = node_with("d", 80.0, 40.0, 4);

        let reads = Expr::Node(a.clone()) * Expr::Node(b.clone())
            + Expr::Node(c.clone()) * Expr::Node(d.clone());
        let qs = QuorumSystem::from_reads(reads);

        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a", "b"]), 0.75);
        sigma_r.insert(quorum(&["c", "d"]), 0.25);

        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 0.1);
        sigma_w.insert(quorum(&["a", "d"]), 0.2);
        sigma_w.insert(quorum(&["b", "c"]), 0.3);
        sigma_w.insert(quorum(&["b", "d"]), 0.4);

        let sigma = qs.make_strategy(sigma_r, sigma_w).expect("ok");

        let fr08 = Distribution::fixed(0.8).expect("ok");

        // node loads at fr=0.8
        let la = 0.8 / 50.0 * 0.75 + 0.2 / 10.0 * (0.1 + 0.2);
        let lb = 0.8 / 60.0 * 0.75 + 0.2 / 20.0 * (0.3 + 0.4);
        let lc = 0.8 / 70.0 * 0.25 + 0.2 / 30.0 * (0.1 + 0.3);
        let ld = 0.8 / 80.0 * 0.25 + 0.2 / 40.0 * (0.2 + 0.4);

        let load_08 = [la, lb, lc, ld].iter().copied().fold(0.0_f64, f64::max);

        let got_load = sigma.load(Some(&fr08), None).expect("ok");
        assert!(
            (got_load - load_08).abs() < 1e-10,
            "load mismatch: {got_load} vs {load_08}"
        );

        let got_cap = sigma.capacity(Some(&fr08), None).expect("ok");
        let expected_cap = 1.0 / load_08;
        assert!(
            (got_cap - expected_cap).abs() < 1e-10,
            "capacity mismatch: {got_cap} vs {expected_cap}"
        );

        // Check node_load for node a
        let got_node_load = sigma.node_load(&a, Some(&fr08), None).expect("ok");
        assert!(
            (got_node_load - la).abs() < 1e-10,
            "node load mismatch: {got_node_load} vs {la}"
        );
    }

    #[test]
    fn strategy_network_load() {
        let a = node("a");
        let b = node("b");
        let c = node("c");
        let d = node("d");
        let e_node = node("e");

        let reads = Expr::Node(a) * Expr::Node(b)
            + Expr::Node(c) * Expr::Node(d) * Expr::Node(e_node);
        let qs = QuorumSystem::from_reads(reads);

        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a", "b"]), 75.0);
        sigma_r.insert(quorum(&["c", "d", "e"]), 25.0);

        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 5.0);
        sigma_w.insert(quorum(&["a", "d"]), 10.0);
        sigma_w.insert(quorum(&["a", "e"]), 15.0);
        sigma_w.insert(quorum(&["b", "c"]), 20.0);
        sigma_w.insert(quorum(&["b", "d"]), 25.0);
        sigma_w.insert(quorum(&["b", "e"]), 25.0);

        let sigma = qs.make_strategy(sigma_r, sigma_w).expect("ok");
        let fr08 = Distribution::fixed(0.8).expect("ok");

        let expected = 0.8 * 0.75 * 2.0 + 0.8 * 0.25 * 3.0 + 0.2 * 2.0;
        let got = sigma.network_load(Some(&fr08), None).expect("ok");
        assert!(
            (got - expected).abs() < 1e-10,
            "network load mismatch: {got} vs {expected}"
        );
    }

    #[test]
    #[expect(clippy::many_single_char_names)]
    fn strategy_latency() {
        let a = node_with("a", 1.0, 1.0, 1);
        let b = node_with("b", 1.0, 1.0, 2);
        let c = node_with("c", 1.0, 1.0, 3);
        let d = node_with("d", 1.0, 1.0, 4);
        let e = node_with("e", 1.0, 1.0, 5);

        let reads = Expr::Node(a) * Expr::Node(b.clone())
            + Expr::Node(c.clone())
                * Expr::Node(d.clone())
                * Expr::Node(e.clone());
        let qs = QuorumSystem::from_reads(reads);

        let mut sigma_r = BTreeMap::new();
        sigma_r.insert(quorum(&["a", "b"]), 10.0);
        sigma_r.insert(quorum(&["a", "b", "c"]), 20.0);
        sigma_r.insert(quorum(&["c", "d", "e"]), 30.0);
        sigma_r.insert(quorum(&["c", "d", "e", "a"]), 40.0);

        let mut sigma_w = BTreeMap::new();
        sigma_w.insert(quorum(&["a", "c"]), 5.0);
        sigma_w.insert(quorum(&["a", "d"]), 10.0);
        sigma_w.insert(quorum(&["a", "e"]), 15.0);
        sigma_w.insert(quorum(&["b", "c"]), 20.0);
        sigma_w.insert(quorum(&["b", "d"]), 25.0);
        sigma_w.insert(quorum(&["b", "e"]), 25.0);

        let sigma = qs.make_strategy(sigma_r, sigma_w).expect("ok");
        let fr08 = Distribution::fixed(0.8).expect("ok");

        let expected_secs = 0.8 * 0.10 * 2.0
            + 0.8 * 0.20 * 2.0
            + 0.8 * 0.30 * 5.0
            + 0.8 * 0.40 * 5.0
            + 0.2 * 0.05 * 3.0
            + 0.2 * 0.10 * 4.0
            + 0.2 * 0.15 * 5.0
            + 0.2 * 0.20 * 3.0
            + 0.2 * 0.25 * 4.0
            + 0.2 * 0.25 * 5.0;

        let got = sigma.latency(Some(&fr08), None).expect("ok").as_secs_f64();
        assert!(
            (got - expected_secs).abs() < 1e-10,
            "latency mismatch: {got} vs {expected_secs}"
        );
    }

    // -- minimize --

    #[test]
    fn minimize_removes_supersets() {
        let sets = vec![
            set(&["a"]),
            set(&["a", "b"]),
            set(&["c"]),
            set(&["a", "b", "c"]),
        ];
        let result = minimize(sets);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&set(&["a"])));
        assert!(result.contains(&set(&["c"])));
    }

    // -- Display --

    #[test]
    fn quorum_system_display() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let s = format!("{qs}");
        assert!(s.contains("QuorumSystem"));
    }

    // -- Parity with the Python reference (tests/test_quorum_system.py) --

    fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    fn fixed(fr: f64) -> Distribution {
        Distribution::fixed(fr).expect("valid")
    }

    fn opt(
        qs: &QuorumSystem<String>,
        objective: Objective,
        fr: f64,
        limits: StrategyLimits,
        f: usize,
    ) -> Result<Strategy<String>> {
        qs.strategy(objective, Some(&fixed(fr)), None, &limits, f)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn python_grid() -> QuorumSystem<String> {
        let a = node_with("a", 2.0, 1.0, 1);
        let b = node_with("b", 2.0, 1.0, 2);
        let c = node_with("c", 2.0, 1.0, 3);
        let d = node_with("d", 2.0, 1.0, 4);
        QuorumSystem::from_reads(
            Expr::Node(a) * Expr::Node(b) + Expr::Node(c) * Expr::Node(d),
        )
    }

    #[test]
    fn python_parity_load_optimized() {
        let qs = python_grid();
        let none = StrategyLimits::default();
        let net2 = StrategyLimits { network: Some(2.0), ..none };
        let lat4 = StrategyLimits { latency: Some(s(4)), ..none };
        for limits in [none, net2, lat4] {
            for (fr, load) in [(1.0, 0.25), (0.0, 0.5)] {
                let sigma = opt(&qs, Objective::Load, fr, limits, 0).unwrap();
                let d = fixed(fr);
                assert!(close(sigma.load(Some(&d), None).unwrap(), load));
                let cap = sigma.capacity(Some(&d), None).unwrap();
                assert!(close(cap, 1.0 / load));
            }
        }
    }

    #[test]
    fn python_parity_network_and_latency_optimized() {
        let qs = python_grid();
        let none = StrategyLimits::default();
        for (fr, limits) in [
            (1.0, none),
            (0.0, none),
            (1.0, StrategyLimits { load: Some(0.25), ..none }),
            (0.0, StrategyLimits { load: Some(0.5), ..none }),
            (1.0, StrategyLimits { latency: Some(s(2)), ..none }),
            (0.0, StrategyLimits { latency: Some(s(3)), ..none }),
        ] {
            let sigma = opt(&qs, Objective::Network, fr, limits, 0).unwrap();
            let net = sigma.network_load(Some(&fixed(fr)), None).unwrap();
            assert!(close(net, 2.0), "fr={fr} {limits:?}: {net}");
        }
        for (fr, lat, limits) in [
            (1.0, 2, none),
            (0.0, 3, none),
            (1.0, 2, StrategyLimits { load: Some(1.0), ..none }),
            (0.0, 3, StrategyLimits { load: Some(1.0), ..none }),
            (1.0, 2, StrategyLimits { network: Some(2.0), ..none }),
            (0.0, 3, StrategyLimits { network: Some(2.0), ..none }),
        ] {
            let sigma = opt(&qs, Objective::Latency, fr, limits, 0).unwrap();
            let got = sigma.latency(Some(&fixed(fr)), None).unwrap();
            assert!(close(got.as_secs_f64(), s(lat).as_secs_f64()));
        }
    }

    #[test]
    fn python_parity_one_resilient() {
        let qs = python_grid();
        let none = StrategyLimits::default();
        for (fr, load) in [(1.0, 0.5), (0.0, 1.0)] {
            let sigma = opt(&qs, Objective::Load, fr, none, 1).unwrap();
            assert!(close(sigma.load(Some(&fixed(fr)), None).unwrap(), load));
        }
        for fr in [1.0, 0.0] {
            let sigma = opt(&qs, Objective::Network, fr, none, 1).unwrap();
            let net = sigma.network_load(Some(&fixed(fr)), None).unwrap();
            assert!(close(net, 4.0));
        }
        for (fr, lat) in [(1.0, 2.0), (0.0, 3.0)] {
            let sigma = opt(&qs, Objective::Latency, fr, none, 1).unwrap();
            let got = sigma.latency(Some(&fixed(fr)), None).unwrap();
            assert!(close(got.as_secs_f64(), lat));
        }
    }

    #[test]
    fn python_parity_illegal_and_unsatisfiable() {
        let qs = python_grid();
        let none = StrategyLimits::default();
        for (objective, limits) in [
            (Objective::Load, StrategyLimits { load: Some(1.0), ..none }),
            (Objective::Network, StrategyLimits { network: Some(2.0), ..none }),
            (
                Objective::Latency,
                StrategyLimits { latency: Some(s(5)), ..none },
            ),
        ] {
            assert!(matches!(
                opt(&qs, objective, 0.1, limits, 0),
                Err(Error::InvalidQuorumSystem(_))
            ));
        }
        for (objective, fr, limits) in [
            (
                Objective::Load,
                0.0,
                StrategyLimits { network: Some(1.5), ..none },
            ),
            (
                Objective::Load,
                0.0,
                StrategyLimits { latency: Some(s(1)), ..none },
            ),
            (
                Objective::Network,
                1.0,
                StrategyLimits {
                    load: Some(0.25),
                    latency: Some(s(2)),
                    ..none
                },
            ),
        ] {
            assert_eq!(
                opt(&qs, objective, fr, limits, 0).unwrap_err(),
                Error::NoStrategyFound
            );
        }
    }

    #[test]
    fn python_parity_uniform_one_resilient() {
        // From test_uniform_strategy: reads = a*b + c*d + e*f, f = 1.
        let qs = QuorumSystem::from_reads(
            n("a") * n("b") + n("c") * n("d") + n("e") * n("f"),
        );
        let sigma = qs.uniform_strategy(1).unwrap();
        let want_r = [
            quorum(&["a", "b", "c", "d"]),
            quorum(&["a", "b", "e", "f"]),
            quorum(&["c", "d", "e", "f"]),
        ];
        assert_eq!(sigma.sigma_r().len(), want_r.len());
        for q in &want_r {
            assert!(close(sigma.sigma_r()[q], 1.0 / 3.0));
        }
        // Same as Python: the only 1-resilient write quorum is all nodes.
        assert_eq!(sigma.sigma_w().len(), 1);
        assert!(close(
            sigma.sigma_w()[&quorum(&["a", "b", "c", "d", "e", "f"])],
            1.0
        ));
    }

    // -- Known failures --

    #[test]
    fn known_failures_preserve_quorum_rules() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let fr = fixed(0.75);
        let failed = set(&["a"]);
        let sigma = qs
            .strategy_with_failures(
                Objective::Load,
                Some(&fr),
                None,
                &StrategyLimits::default(),
                0,
                &failed,
            )
            .unwrap();
        assert!(close(sigma.load(Some(&fr), None).unwrap(), 0.875));
        assert!(close(
            sigma.node_load(&node("a"), Some(&fr), None).unwrap(),
            0.0
        ));
        assert!(close(sigma.sigma_r()[&quorum(&["c", "d"])], 1.0));
        assert!(close(sigma.sigma_w()[&quorum(&["b", "c"])], 0.5));
        assert!(close(sigma.sigma_w()[&quorum(&["b", "d"])], 0.5));
        assert_eq!(
            quorum_set(sigma.quorum_system().read_quorums()),
            quorum_set(qs.read_quorums())
        );
        assert_eq!(
            quorum_set(sigma.quorum_system().write_quorums()),
            quorum_set(qs.write_quorums())
        );
        for (weights, expr, opposite) in [
            (sigma.sigma_r(), qs.reads(), qs.writes()),
            (sigma.sigma_w(), qs.writes(), qs.reads()),
        ] {
            assert!(close(weights.values().sum(), 1.0));
            for q in weights.keys().map(|q| to_set(q)) {
                assert!(q.is_disjoint(&failed));
                assert!(expr.is_quorum(&q));
                assert!(opposite.quorums().all(|other| !q.is_disjoint(&other)));
            }
        }
    }

    #[test]
    fn known_failures_match_threshold_bounds() {
        let reads =
            crate::expr::majority(vec![n("a"), n("b"), n("c"), n("d"), n("e")])
                .unwrap();
        let qs = QuorumSystem::from_reads(reads);
        let fr = fixed(0.5);
        for failed_nodes in ["a", "b", "c", "d", "e"].into_iter().powerset() {
            let failed = set(&failed_nodes);
            let survivors = 5 - failed.len();
            for f in 0..=2 {
                let result = qs.strategy_with_failures(
                    Objective::Load,
                    Some(&fr),
                    None,
                    &StrategyLimits::default(),
                    f,
                    &failed,
                );
                let size = 3 + f;
                if survivors < size {
                    assert_eq!(result.unwrap_err(), Error::NoStrategyFound);
                    continue;
                }
                let sigma = result.unwrap();
                let load = len_f64(size) / len_f64(survivors);
                assert!(close(sigma.load(Some(&fr), None).unwrap(), load));
                for (weights, expr) in [
                    (sigma.sigma_r(), qs.reads()),
                    (sigma.sigma_w(), qs.writes()),
                ] {
                    assert!(close(weights.values().sum(), 1.0));
                    for q in weights.keys().map(|q| to_set(q)) {
                        assert_eq!(q.len(), size);
                        assert!(q.is_disjoint(&failed));
                        for further in q.iter().combinations(f) {
                            let alive = q
                                .iter()
                                .filter(|x| !further.contains(x))
                                .cloned()
                                .collect();
                            assert!(expr.is_quorum(&alive));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn known_failures_reject_unknown_nodes() {
        let qs = python_grid();
        assert!(matches!(
            qs.strategy_with_failures(
                Objective::Load,
                Some(&fixed(0.5)),
                None,
                &StrategyLimits::default(),
                0,
                &set(&["missing"]),
            ),
            Err(Error::InvalidQuorumSystem(_))
        ));
    }

    #[test]
    fn known_failures_report_unavailable_quorums() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        for failed in [set(&["a"]), set(&["a", "b"])] {
            for f in [0, 1] {
                assert_eq!(
                    qs.strategy_with_failures(
                        Objective::Load,
                        Some(&fixed(0.5)),
                        None,
                        &StrategyLimits::default(),
                        f,
                        &failed,
                    )
                    .unwrap_err(),
                    Error::NoStrategyFound
                );
            }
        }
    }

    #[test]
    fn known_failures_respect_metrics_and_limits() {
        let qs = python_grid();
        let fw = fixed(0.5);
        let failed = set(&["a"]);
        let none = StrategyLimits::default();
        for (objective, load, latency, limits) in [
            (Objective::Load, 0.5, 3.75, none),
            (
                Objective::Network,
                0.5,
                3.75,
                StrategyLimits { load: Some(0.5), ..none },
            ),
            (Objective::Latency, 0.75, 3.5, none),
        ] {
            let sigma = qs
                .strategy_with_failures(
                    objective,
                    None,
                    Some(&fw),
                    &limits,
                    0,
                    &failed,
                )
                .unwrap();
            assert!(close(sigma.load(None, Some(&fw)).unwrap(), load));
            assert!(close(sigma.network_load(None, Some(&fw)).unwrap(), 2.0));
            assert!(close(
                sigma.latency(None, Some(&fw)).unwrap().as_secs_f64(),
                latency
            ));
        }
        assert_eq!(
            qs.strategy_with_failures(
                Objective::Network,
                None,
                Some(&fw),
                &StrategyLimits { load: Some(0.4), ..none },
                0,
                &failed,
            )
            .unwrap_err(),
            Error::NoStrategyFound
        );
    }

    // -- Regression tests for 2.0 fixes --

    #[test]
    fn f_resilient_impossible_is_no_strategy() {
        let qs = QuorumSystem::from_reads(n("a") * n("b"));
        assert_eq!(qs.uniform_strategy(1).unwrap_err(), Error::NoStrategyFound);
        assert_eq!(
            opt(&qs, Objective::Load, 0.5, StrategyLimits::default(), 1)
                .unwrap_err(),
            Error::NoStrategyFound
        );
    }

    #[test]
    fn make_strategy_merges_unsorted_and_duplicate_keys() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let mut r = BTreeMap::new();
        r.insert(vec!["a".to_string(), "a".to_string()], 1.0);
        r.insert(quorum(&["a"]), 1.0);
        r.insert(quorum(&["b"]), 2.0);
        r.insert(quorum(&["b", "a"]).into_iter().rev().collect(), 0.0);
        let mut w = BTreeMap::new();
        w.insert(vec!["b".to_string(), "a".to_string()], 1.0);
        let sigma = qs.make_strategy(r, w).unwrap();
        assert_eq!(sigma.sigma_r().len(), 2);
        assert!(close(sigma.sigma_r()[&quorum(&["a"])], 0.5));
        let load_a = sigma.node_load(&node("a"), Some(&fixed(1.0)), None);
        assert!(close(load_a.unwrap(), 0.5));
        assert_eq!(sigma.sigma_w().keys().next(), Some(&quorum(&["a", "b"])));
    }

    #[test]
    fn make_strategy_rejects_bad_weights() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let one = |q: &[&str], w: f64| {
            let mut m = BTreeMap::new();
            m.insert(quorum(q), w);
            m
        };
        let w = || one(&["a", "b"], 1.0);
        assert!(qs.make_strategy(BTreeMap::new(), w()).is_err());
        assert!(qs.make_strategy(one(&["a"], 0.0), w()).is_err());
        assert!(qs.make_strategy(one(&["a"], f64::NAN), w()).is_err());
        assert!(qs.make_strategy(one(&["a"], f64::INFINITY), w()).is_err());
        assert!(qs.make_strategy(one(&["a"], 1.0), one(&["a"], 1.0)).is_err());
        assert!(qs
            .make_strategy(one(&["a"], 1.0), one(&["a", "b"], -1.0))
            .is_err());
    }

    #[test]
    fn node_metrics_use_system_capacities() {
        let a = node_with("a", 2.0, 2.0, 1);
        let qs = QuorumSystem::from_reads(Expr::Node(a) + n("b"));
        let sigma = qs.uniform_strategy(0).unwrap();
        let fr = fixed(1.0);
        // Pass a bare Node: capacity 2 must still be used for "a".
        let la = sigma.node_load(&node("a"), Some(&fr), None).unwrap();
        assert!(close(la, 0.25));
        let lb = sigma.node_load(&node("b"), Some(&fr), None).unwrap();
        assert!(close(lb, 0.5));
        let ua = sigma.node_utilization(&node("a"), Some(&fr), None).unwrap();
        assert!(close(ua, 0.5));
        let tb = sigma.node_throughput(&node("b"), Some(&fr), None).unwrap();
        assert!(close(tb, 1.0));
        assert!(close(
            sigma.node_load(&node("zz"), Some(&fr), None).unwrap(),
            0.0
        ));
        assert!(sigma.load(None, None).is_err());
        assert!(sigma.capacity(Some(&fr), Some(&fr)).is_err());
        assert!(sigma.network_load(None, None).is_err());
        assert!(sigma.latency(None, None).is_err());
        assert!(sigma.node_load(&node("a"), None, None).is_err());
        assert!(sigma.node_utilization(&node("a"), None, None).is_err());
        assert!(sigma.node_throughput(&node("a"), None, None).is_err());
    }

    #[test]
    fn accessors_and_sampling() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c"));
        assert_eq!(qs.reads().to_string(), "((a * b) + c)");
        assert_eq!(qs.writes().to_string(), "((a + b) * c)");
        assert_eq!(qs.node(&"a".to_string()).unwrap().x(), "a");
        assert!(qs.node(&"zz".to_string()).is_err());
        let sigma = qs.uniform_strategy(0).unwrap();
        assert_eq!(sigma.quorum_system().elements(), qs.elements());
        for _ in 0..20 {
            assert!(qs.is_read_quorum(&sigma.get_read_quorum()));
            assert!(qs.is_write_quorum(&sigma.get_write_quorum()));
        }
        let shown = sigma.to_string();
        assert!(shown.contains("{a, b}: 0.5000"), "{shown}");
    }

    #[test]
    fn strategy_with_write_fraction_and_weighted() {
        let qs = QuorumSystem::from_reads(n("a") + n("b") + n("c"));
        let none = StrategyLimits::default();
        let w = fixed(0.25);
        let by_w =
            qs.strategy(Objective::Load, None, Some(&w), &none, 0).unwrap();
        let by_r = opt(&qs, Objective::Load, 0.75, none, 0).unwrap();
        let l1 = by_w.load(None, Some(&w)).unwrap();
        let l2 = by_r.load(Some(&fixed(0.75)), None).unwrap();
        assert!(close(l1, l2));
        let d = Distribution::weighted(&[(0.0, 1.0), (1.0, 1.0)]).unwrap();
        let sigma =
            qs.strategy(Objective::Load, Some(&d), None, &none, 0).unwrap();
        assert!(sigma.load(Some(&d), None).unwrap() > 0.0);
        assert!(qs.strategy(Objective::Load, None, None, &none, 0).is_err());
    }
}
