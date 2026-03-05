//! Quorum system types for modeling read-write distributed systems.
//!
//! A [`QuorumSystem`] pairs a read expression with a write expression
//! such that every read quorum intersects every write quorum. Given
//! a quorum system, one can compute resilience, enumerate quorums,
//! build strategies, and optimize for load/network/latency.

use crate::distribution::{self, Canonical, Distribution, OrderedFloat};
use crate::error::{Error, Result};
use crate::expr::{Element, Expr, Node};
use itertools::Itertools;
use rand::seq::SliceRandom;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

/// Optimization objective for strategy computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objective {
    /// Minimize the maximum load on any node.
    Load,
    /// Minimize expected quorum size.
    Network,
    /// Minimize expected quorum latency.
    Latency,
}

/// A quorum represented as a sorted vector of elements.
/// Used as a key in strategy probability maps.
type Quorum<T> = Vec<T>;

/// Convert a HashSet to a sorted Quorum (Vec).
fn to_quorum<T: Element>(set: HashSet<T>) -> Quorum<T> {
    let mut vec: Vec<T> = set.into_iter().collect();
    vec.sort();
    vec
}

/// Convert a Quorum (Vec) back to a HashSet.
fn from_quorum<T: Element>(quorum: Quorum<T>) -> HashSet<T> {
    quorum.into_iter().collect()
}

/// A read-write quorum system.
#[derive(Debug, Clone)]
pub struct QuorumSystem<T: Element> {
    /// Read quorum expression.
    pub reads: Expr<T>,
    /// Write quorum expression.
    pub writes: Expr<T>,
    x_to_node: HashMap<T, Node<T>>,
}

impl<T: Element> QuorumSystem<T> {
    /// Build a quorum system from reads only; writes are the dual.
    pub fn from_reads(reads: Expr<T>) -> Self {
        let writes = reads.dual();
        let x_to_node = Self::build_node_map(&reads, &writes);
        Self {
            reads,
            writes,
            x_to_node,
        }
    }

    /// Build a quorum system from writes only; reads are the dual.
    pub fn from_writes(writes: Expr<T>) -> Self {
        let reads = writes.dual();
        let x_to_node = Self::build_node_map(&reads, &writes);
        Self {
            reads,
            writes,
            x_to_node,
        }
    }

    /// Build a quorum system from both read and write expressions.
    ///
    /// Validates that every read quorum intersects every write
    /// quorum.
    pub fn new(reads: Expr<T>, writes: Expr<T>) -> Result<Self> {
        let optimal_writes = reads.dual();
        for wq in writes.quorums() {
            if !optimal_writes.is_quorum(&wq) {
                return Err(Error::InvalidQuorumSystem(
                    "not all read quorums intersect all \
                     write quorums"
                        .into(),
                ));
            }
        }
        let x_to_node = Self::build_node_map(&reads, &writes);
        Ok(Self {
            reads,
            writes,
            x_to_node,
        })
    }

    /// Return an iterator over all read quorums.
    pub fn read_quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        self.reads.quorums()
    }

    /// Return an iterator over all write quorums.
    pub fn write_quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        self.writes.quorums()
    }

    /// Check if a set of elements forms a read quorum.
    #[must_use]
    pub fn is_read_quorum(&self, xs: &HashSet<T>) -> bool {
        self.reads.is_quorum(xs)
    }

    /// Check if a set of elements forms a write quorum.
    #[must_use]
    pub fn is_write_quorum(&self, xs: &HashSet<T>) -> bool {
        self.writes.is_quorum(xs)
    }

    /// Look up a node by its element identifier.
    pub fn node(&self, x: &T) -> Result<&Node<T>> {
        self.x_to_node.get(x).ok_or_else(|| {
            Error::InvalidQuorumSystem(format!("element {x} not found in quorum system"))
        })
    }

    /// Return the set of all nodes in the system.
    #[must_use]
    pub fn nodes(&self) -> HashSet<Node<T>> {
        let mut nodes = self.reads.nodes();
        nodes.extend(self.writes.nodes());
        nodes
    }

    /// Return the set of all element identifiers in the system.
    #[must_use]
    pub fn elements(&self) -> HashSet<T> {
        self.nodes().into_iter().map(|n| n.x.clone()).collect()
    }

    /// Return the resilience of the system: the minimum of
    /// read and write resilience.
    #[must_use]
    pub fn resilience(&self) -> i64 {
        std::cmp::min(self.read_resilience(), self.write_resilience())
    }

    /// Return the resilience of the read expression.
    #[must_use]
    pub fn read_resilience(&self) -> i64 {
        self.reads.resilience()
    }

    /// Return the resilience of the write expression.
    #[must_use]
    pub fn write_resilience(&self) -> i64 {
        self.writes.resilience()
    }

    /// Check if both read and write expressions are duplicate-free.
    #[must_use]
    pub fn dup_free(&self) -> bool {
        self.reads.dup_free() && self.writes.dup_free()
    }

    /// Build a uniform strategy: equal probability for each
    /// minimal quorum.
    pub fn uniform_strategy(&self, f: usize) -> Result<Strategy<T>> {
        let (read_quorums, write_quorums) = if f == 0 {
            (
                self.read_quorums().collect::<Vec<_>>(),
                self.write_quorums().collect::<Vec<_>>(),
            )
        } else {
            let xs: Vec<T> = self.elements().into_iter().collect();
            let rq: Vec<HashSet<T>> = self.f_resilient_quorums(f, &xs, &self.reads);
            let wq: Vec<HashSet<T>> = self.f_resilient_quorums(f, &xs, &self.writes);
            if rq.is_empty() {
                return Err(Error::NoStrategyFound);
            }
            if wq.is_empty() {
                return Err(Error::NoStrategyFound);
            }
            (rq, wq)
        };

        let read_quorums = minimize(read_quorums);
        let write_quorums = minimize(write_quorums);

        let rn = read_quorums.len() as f64;
        let wn = write_quorums.len() as f64;

        let sigma_r: BTreeMap<Quorum<T>, f64> = read_quorums
            .into_iter()
            .map(|q| (to_quorum(q), 1.0 / rn))
            .collect();
        let sigma_w: BTreeMap<Quorum<T>, f64> = write_quorums
            .into_iter()
            .map(|q| (to_quorum(q), 1.0 / wn))
            .collect();

        Ok(Strategy::new(self, sigma_r, sigma_w))
    }

    /// Build a strategy from explicit quorum probability maps.
    /// Weights are normalized to sum to 1.
    pub fn make_strategy(
        &self,
        sigma_r: BTreeMap<Quorum<T>, f64>,
        sigma_w: BTreeMap<Quorum<T>, f64>,
    ) -> Result<Strategy<T>> {
        if sigma_r.values().any(|&w| w < 0.0) {
            return Err(Error::InvalidQuorumSystem(
                "sigma_r has negative weights".into(),
            ));
        }
        if sigma_w.values().any(|&w| w < 0.0) {
            return Err(Error::InvalidQuorumSystem(
                "sigma_w has negative weights".into(),
            ));
        }
        for rq in sigma_r.keys() {
            if !self.is_read_quorum(&from_quorum(rq.clone())) {
                return Err(Error::InvalidQuorumSystem(
                    "sigma_r has non-read quorums".into(),
                ));
            }
        }
        for wq in sigma_w.keys() {
            if !self.is_write_quorum(&from_quorum(wq.clone())) {
                return Err(Error::InvalidQuorumSystem(
                    "sigma_w has non-write quorums".into(),
                ));
            }
        }

        let r_total: f64 = sigma_r.values().sum();
        let w_total: f64 = sigma_w.values().sum();
        let normalized_r: BTreeMap<Quorum<T>, f64> =
            sigma_r.into_iter().map(|(q, w)| (q, w / r_total)).collect();
        let normalized_w: BTreeMap<Quorum<T>, f64> =
            sigma_w.into_iter().map(|(q, w)| (q, w / w_total)).collect();

        Ok(Strategy::new(self, normalized_r, normalized_w))
    }

    /// Compute the optimal strategy via linear programming.
    pub fn strategy(
        &self,
        objective: Objective,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
        load_limit: Option<f64>,
        network_limit: Option<f64>,
        latency_limit: Option<Duration>,
        f: usize,
    ) -> Result<Strategy<T>> {
        if objective == Objective::Load && load_limit.is_some() {
            return Err(Error::InvalidQuorumSystem(
                "a load limit cannot be set when \
                 optimizing for load"
                    .into(),
            ));
        }
        if objective == Objective::Network && network_limit.is_some() {
            return Err(Error::InvalidQuorumSystem(
                "a network limit cannot be set when \
                 optimizing for network"
                    .into(),
            ));
        }
        if objective == Objective::Latency && latency_limit.is_some() {
            return Err(Error::InvalidQuorumSystem(
                "a latency limit cannot be set when \
                 optimizing for latency"
                    .into(),
            ));
        }

        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;

        let (read_quorums, write_quorums) = if f == 0 {
            (
                self.read_quorums().collect::<Vec<_>>(),
                self.write_quorums().collect::<Vec<_>>(),
            )
        } else {
            let xs: Vec<T> = self.elements().into_iter().collect();
            let rq = self.f_resilient_quorums(f, &xs, &self.reads);
            let wq = self.f_resilient_quorums(f, &xs, &self.writes);
            if rq.is_empty() || wq.is_empty() {
                return Err(Error::NoStrategyFound);
            }
            (rq, wq)
        };

        self.lp_optimal_strategy(
            &read_quorums,
            &write_quorums,
            &d,
            objective,
            load_limit,
            network_limit,
            latency_limit,
        )
    }

    /// Compute the latency of a quorum using the earliest point
    /// at which the quorum condition is met (nodes sorted by
    /// latency).
    fn quorum_latency(
        &self,
        quorum: &HashSet<T>,
        is_quorum: &dyn Fn(&HashSet<T>) -> bool,
    ) -> Duration {
        let mut nodes: Vec<&Node<T>> = quorum
            .iter()
            .filter_map(|x| self.x_to_node.get(x))
            .collect();
        nodes.sort_by_key(|n| n.latency);

        let mut seen = HashSet::new();
        for node in nodes {
            seen.insert(node.x.clone());
            if is_quorum(&seen) {
                return node.latency;
            }
        }
        Duration::ZERO
    }

    fn read_quorum_latency(&self, quorum: &HashSet<T>) -> Duration {
        self.quorum_latency(quorum, &|xs| self.reads.is_quorum(xs))
    }

    fn write_quorum_latency(&self, quorum: &HashSet<T>) -> Duration {
        self.quorum_latency(quorum, &|xs| self.writes.is_quorum(xs))
    }

    /// Find all f-resilient quorums: quorums that remain valid
    /// even after removing any f elements.
    fn f_resilient_quorums(&self, f: usize, xs: &[T], expr: &Expr<T>) -> Vec<HashSet<T>> {
        let mut results = Vec::new();
        let mut current = HashSet::new();
        Self::f_resilient_helper(f, xs, expr, &mut current, 0, &mut results);
        results
    }

    fn f_resilient_helper(
        f: usize,
        xs: &[T],
        expr: &Expr<T>,
        current: &mut HashSet<T>,
        start: usize,
        results: &mut Vec<HashSet<T>>,
    ) {
        let check_size = std::cmp::min(f, current.len());
        let is_resilient = if check_size == 0 {
            expr.is_quorum(current)
        } else {
            let elems: Vec<T> = current.iter().cloned().collect();
            elems.iter().combinations(check_size).all(|failure| {
                let remaining: HashSet<T> = current
                    .iter()
                    .filter(|x| !failure.contains(x))
                    .cloned()
                    .collect();
                expr.is_quorum(&remaining)
            })
        };

        if is_resilient {
            results.push(current.clone());
            return;
        }

        for j in start..xs.len() {
            current.insert(xs[j].clone());
            Self::f_resilient_helper(f, xs, expr, current, j + 1, results);
            current.remove(&xs[j]);
        }
    }

    /// Solve the LP to find an optimal strategy.
    fn lp_optimal_strategy(
        &self,
        read_quorums: &[HashSet<T>],
        write_quorums: &[HashSet<T>],
        read_fraction: &Canonical,
        objective: Objective,
        load_limit: Option<f64>,
        network_limit: Option<f64>,
        latency_limit: Option<Duration>,
    ) -> Result<Strategy<T>> {
        use good_lp::*;

        let mut vars = ProblemVariables::new();

        // Create variables for read/write quorum probabilities.
        let r_vars: Vec<Variable> = (0..read_quorums.len())
            .map(|_| vars.add(variable().min(0.0).max(1.0)))
            .collect();
        let w_vars: Vec<Variable> = (0..write_quorums.len())
            .map(|_| vars.add(variable().min(0.0).max(1.0)))
            .collect();

        // Map each element to its read/write quorum variables.
        let mut x_to_r_vars: HashMap<T, Vec<Variable>> = HashMap::new();
        for (i, rq) in read_quorums.iter().enumerate() {
            for x in rq {
                x_to_r_vars.entry(x.clone()).or_default().push(r_vars[i]);
            }
        }
        let mut x_to_w_vars: HashMap<T, Vec<Variable>> = HashMap::new();
        for (i, wq) in write_quorums.iter().enumerate() {
            for x in wq {
                x_to_w_vars.entry(x.clone()).or_default().push(w_vars[i]);
            }
        }

        // Weighted average fr for network/latency expressions.
        let avg_fr: f64 = read_fraction.iter().map(|(k, &p)| k.0 * p).sum();

        // Build network load expression.
        let network_expr = |r: &[Variable], w: &[Variable]| -> Expression {
            let read_part: Expression = read_quorums
                .iter()
                .zip(r.iter())
                .map(|(rq, &v)| Expression::from(v) * rq.len() as f64)
                .sum();
            let write_part: Expression = write_quorums
                .iter()
                .zip(w.iter())
                .map(|(wq, &v)| Expression::from(v) * wq.len() as f64)
                .sum();
            avg_fr * read_part + (1.0 - avg_fr) * write_part
        };

        // Build latency expression.
        let latency_expr = |r: &[Variable], w: &[Variable]| -> Expression {
            let read_part: Expression = read_quorums
                .iter()
                .zip(r.iter())
                .map(|(rq, &v)| {
                    let lat = self.read_quorum_latency(rq).as_secs_f64();
                    Expression::from(v) * lat
                })
                .sum();
            let write_part: Expression = write_quorums
                .iter()
                .zip(w.iter())
                .map(|(wq, &v)| {
                    let lat = self.write_quorum_latency(wq).as_secs_f64();
                    Expression::from(v) * lat
                })
                .sum();
            avg_fr * read_part + (1.0 - avg_fr) * write_part
        };

        // Build load variables upfront.
        let mut load_info: Vec<(OrderedFloat, f64, Variable)> = Vec::new();
        for (&fr_key, &p) in read_fraction {
            let l = vars.add(variable().min(0.0));
            load_info.push((fr_key, p, l));
        }

        // Build objective expression.
        let obj: Expression = match objective {
            Objective::Load => load_info
                .iter()
                .map(|&(_, p, l)| Expression::from(l) * p)
                .sum(),
            Objective::Network => network_expr(&r_vars, &w_vars),
            Objective::Latency => latency_expr(&r_vars, &w_vars),
        };

        let mut problem = vars.minimise(obj).using(default_solver);

        // Constraint: read/write probabilities sum to 1.
        let r_sum: Expression = r_vars.iter().copied().sum();
        problem = problem.with(r_sum.eq(1.0));
        let w_sum: Expression = w_vars.iter().copied().sum();
        problem = problem.with(w_sum.eq(1.0));

        // Add load constraints for each fr.
        let all_nodes: Vec<Node<T>> = self.nodes().into_iter().collect();
        for &(fr_key, _, l) in &load_info {
            let fr = fr_key.0;
            for node in &all_nodes {
                let x = &node.x;
                let mut x_load = Expression::from(0.0);
                if let Some(vs) = x_to_r_vars.get(x) {
                    let rsum: Expression = vs.iter().copied().sum();
                    x_load += rsum * (fr / node.read_capacity);
                }
                if let Some(vs) = x_to_w_vars.get(x) {
                    let wsum: Expression = vs.iter().copied().sum();
                    x_load += wsum * ((1.0 - fr) / node.write_capacity);
                }
                problem = problem.with(x_load.leq(l));
            }
        }

        // Add optional constraints.
        if let Some(ll) = load_limit {
            let load_expr: Expression = load_info
                .iter()
                .map(|&(_, p, l)| Expression::from(l) * p)
                .sum();
            problem = problem.with(load_expr.leq(ll));
        }
        if let Some(nl) = network_limit {
            let ne = network_expr(&r_vars, &w_vars);
            problem = problem.with(ne.leq(nl));
        }
        if let Some(ll) = latency_limit {
            let le = latency_expr(&r_vars, &w_vars);
            problem = problem.with(le.leq(ll.as_secs_f64()));
        }

        // Solve.
        let solution = problem
            .solve()
            .map_err(|e| Error::LpError(format!("{e}")))?;

        // Extract non-zero quorum probabilities.
        let sigma_r: BTreeMap<Quorum<T>, f64> = read_quorums
            .iter()
            .zip(r_vars.iter())
            .filter_map(|(rq, &v)| {
                let val = solution.value(v);
                if val > 1e-10 {
                    Some((to_quorum(rq.clone()), val))
                } else {
                    None
                }
            })
            .collect();
        let sigma_w: BTreeMap<Quorum<T>, f64> = write_quorums
            .iter()
            .zip(w_vars.iter())
            .filter_map(|(wq, &v)| {
                let val = solution.value(v);
                if val > 1e-10 {
                    Some((to_quorum(wq.clone()), val))
                } else {
                    None
                }
            })
            .collect();

        Ok(Strategy::new(self, sigma_r, sigma_w))
    }

    fn build_node_map(reads: &Expr<T>, writes: &Expr<T>) -> HashMap<T, Node<T>> {
        let mut map = HashMap::new();
        for node in reads.nodes() {
            map.insert(node.x.clone(), node);
        }
        for node in writes.nodes() {
            map.entry(node.x.clone()).or_insert(node);
        }
        map
    }
}

impl<T: Element> std::fmt::Display for QuorumSystem<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "QuorumSystem(reads={}, writes={})",
            self.reads, self.writes
        )
    }
}

/// A strategy assigns probabilities to read and write quorums.
#[derive(Debug, Clone)]
pub struct Strategy<T: Element> {
    /// Read quorum probabilities.
    pub sigma_r: BTreeMap<Quorum<T>, f64>,
    /// Write quorum probabilities.
    pub sigma_w: BTreeMap<Quorum<T>, f64>,
    /// Per-element read probability: P(x in chosen read quorum).
    x_read_prob: HashMap<T, f64>,
    /// Per-element write probability: P(x in chosen write quorum).
    x_write_prob: HashMap<T, f64>,
    /// Cached node map from the quorum system.
    x_to_node: HashMap<T, Node<T>>,
    /// All nodes in the quorum system.
    all_nodes: HashSet<Node<T>>,
    /// Read expression (for latency computation).
    reads: Expr<T>,
    /// Write expression (for latency computation).
    writes: Expr<T>,
}

impl<T: Element> Strategy<T> {
    /// Build a strategy from a quorum system and quorum
    /// probability maps.
    fn new(
        qs: &QuorumSystem<T>,
        sigma_r: BTreeMap<Quorum<T>, f64>,
        sigma_w: BTreeMap<Quorum<T>, f64>,
    ) -> Self {
        let mut x_read_prob: HashMap<T, f64> = HashMap::new();
        for (rq, &p) in &sigma_r {
            for x in rq {
                *x_read_prob.entry(x.clone()).or_default() += p;
            }
        }

        let mut x_write_prob: HashMap<T, f64> = HashMap::new();
        for (wq, &p) in &sigma_w {
            for x in wq {
                *x_write_prob.entry(x.clone()).or_default() += p;
            }
        }

        Self {
            sigma_r,
            sigma_w,
            x_read_prob,
            x_write_prob,
            x_to_node: qs.x_to_node.clone(),
            all_nodes: qs.nodes(),
            reads: qs.reads.clone(),
            writes: qs.writes.clone(),
        }
    }

    /// Sample a random read quorum according to probabilities.
    pub fn get_read_quorum(&self) -> HashSet<T> {
        sample_quorum(&self.sigma_r)
    }

    /// Sample a random write quorum according to probabilities.
    pub fn get_write_quorum(&self) -> HashSet<T> {
        sample_quorum(&self.sigma_w)
    }

    /// Compute the load for a given distribution.
    pub fn load(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter().map(|(fr, &p)| p * self.load_at(fr.0)).sum())
    }

    /// Compute capacity (inverse load) for a distribution.
    pub fn capacity(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter().map(|(fr, &p)| p / self.load_at(fr.0)).sum())
    }

    /// Compute the expected network load (quorum size).
    pub fn network_load(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let fr: f64 = d.iter().map(|(k, &p)| k.0 * p).sum();
        let reads: f64 = self
            .sigma_r
            .iter()
            .map(|(rq, &p)| p * rq.len() as f64)
            .sum();
        let writes: f64 = self
            .sigma_w
            .iter()
            .map(|(wq, &p)| p * wq.len() as f64)
            .sum();
        Ok(fr * reads + (1.0 - fr) * writes)
    }

    /// Compute the expected latency.
    pub fn latency(
        &self,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<Duration> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        let fr: f64 = d.iter().map(|(k, &p)| k.0 * p).sum();

        let read_lat: f64 = self
            .sigma_r
            .iter()
            .map(|(rq, &p)| {
                let lat = self
                    .quorum_latency(&from_quorum(rq.clone()), true)
                    .as_secs_f64();
                p * lat
            })
            .sum();
        let write_lat: f64 = self
            .sigma_w
            .iter()
            .map(|(wq, &p)| {
                let lat = self
                    .quorum_latency(&from_quorum(wq.clone()), false)
                    .as_secs_f64();
                p * lat
            })
            .sum();

        let total = fr * read_lat + (1.0 - fr) * write_lat;
        Ok(Duration::from_secs_f64(total))
    }

    /// Compute the load on a specific node for a distribution.
    pub fn node_load(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter()
            .map(|(fr, &p)| p * self.node_load_at(node, fr.0))
            .sum())
    }

    /// Compute the utilization of a node for a distribution.
    pub fn node_utilization(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter()
            .map(|(fr, &p)| {
                let nl = self.node_load_at(node, fr.0);
                let l = self.load_at(fr.0);
                p * nl / l
            })
            .sum())
    }

    /// Compute the throughput for a node under a distribution.
    pub fn node_throughput(
        &self,
        node: &Node<T>,
        read_fraction: Option<&Distribution>,
        write_fraction: Option<&Distribution>,
    ) -> Result<f64> {
        let d = distribution::canonicalize_rw(read_fraction, write_fraction)?;
        Ok(d.iter()
            .map(|(fr, &p)| {
                let cap = 1.0 / self.load_at(fr.0);
                let fw = 1.0 - fr.0;
                let rp = self.x_read_prob.get(&node.x).copied().unwrap_or(0.0);
                let wp = self.x_write_prob.get(&node.x).copied().unwrap_or(0.0);
                p * cap * (fr.0 * rp + fw * wp)
            })
            .sum())
    }

    /// Compute the load at a fixed read fraction.
    fn load_at(&self, fr: f64) -> f64 {
        self.all_nodes
            .iter()
            .map(|n| self.node_load_at(n, fr))
            .fold(0.0_f64, f64::max)
    }

    /// Compute the load on a specific node at a fixed fr.
    fn node_load_at(&self, node: &Node<T>, fr: f64) -> f64 {
        let fw = 1.0 - fr;
        let rp = self.x_read_prob.get(&node.x).copied().unwrap_or(0.0);
        let wp = self.x_write_prob.get(&node.x).copied().unwrap_or(0.0);
        fr * rp / node.read_capacity + fw * wp / node.write_capacity
    }

    /// Compute the latency for a quorum.
    fn quorum_latency(&self, quorum: &HashSet<T>, is_read: bool) -> Duration {
        let mut nodes: Vec<&Node<T>> = quorum
            .iter()
            .filter_map(|x| self.x_to_node.get(x))
            .collect();
        nodes.sort_by_key(|n| n.latency);

        let mut seen = HashSet::new();
        for node in nodes {
            seen.insert(node.x.clone());
            let satisfied = if is_read {
                self.reads.is_quorum(&seen)
            } else {
                self.writes.is_quorum(&seen)
            };
            if satisfied {
                return node.latency;
            }
        }
        Duration::ZERO
    }
}

impl<T: Element> std::fmt::Display for Strategy<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reads: Vec<String> = self
            .sigma_r
            .iter()
            .map(|(q, p)| {
                let mut elems: Vec<String> = q.iter().map(ToString::to_string).collect();
                elems.sort();
                format!("{{{}}}: {p:.4}", elems.join(", "))
            })
            .collect();
        let writes: Vec<String> = self
            .sigma_w
            .iter()
            .map(|(q, p)| {
                let mut elems: Vec<String> = q.iter().map(ToString::to_string).collect();
                elems.sort();
                format!("{{{}}}: {p:.4}", elems.join(", "))
            })
            .collect();
        write!(
            f,
            "Strategy(reads=[{}], writes=[{}])",
            reads.join(", "),
            writes.join(", ")
        )
    }
}

/// Remove non-minimal sets: keep only sets that are not
/// supersets of any other set in the collection.
fn minimize<T: Element>(mut sets: Vec<HashSet<T>>) -> Vec<HashSet<T>> {
    sets.sort_by_key(|s| s.len());
    let mut minimal: Vec<HashSet<T>> = Vec::new();
    for s in sets {
        if !minimal.iter().any(|m| s.is_superset(m)) {
            minimal.push(s);
        }
    }
    minimal
}

/// Sample a quorum from a probability distribution.
fn sample_quorum<T: Element>(sigma: &BTreeMap<Quorum<T>, f64>) -> HashSet<T> {
    let entries: Vec<(&Quorum<T>, &f64)> = sigma.iter().collect();
    let mut rng = rand::thread_rng();
    let chosen = entries
        .choose_weighted(&mut rng, |(_q, &w)| w)
        .map(|(q, _)| from_quorum((*q).clone()))
        .unwrap_or_default();
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Node;
    use std::collections::HashSet;

    fn n(x: &str) -> Expr<String> {
        Expr::Node(Node::new(x.to_string()))
    }

    fn node(x: &str) -> Node<String> {
        Node::new(x.to_string())
    }

    fn node_with(x: &str, rc: f64, wc: f64, lat: u64) -> Node<String> {
        Node::new(x.to_string())
            .with_read_write_capacity(rc, wc)
            .with_latency(Duration::from_secs(lat))
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn quorum(items: &[&str]) -> Quorum<String> {
        let mut v: Vec<String> = items.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v
    }

    fn quorum_set(qs: impl Iterator<Item = HashSet<String>>) -> HashSet<Vec<String>> {
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
        let qs = QuorumSystem::new(n("a") + n("b"), n("a").clone());
        assert!(qs.is_err());
    }

    // -- Basic methods --

    #[test]
    fn elements_returns_all() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let elems = qs.elements();
        assert!(elems.contains(&"a".to_string()));
        assert!(elems.contains(&"b".to_string()));
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
        let qs = QuorumSystem::from_reads(n("a").clone());
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r.len(), 1);
        assert!((sigma.sigma_r[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
        assert_eq!(sigma.sigma_w.len(), 1);
        assert!((sigma.sigma_w[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn uniform_strategy_two_nodes() {
        let qs = QuorumSystem::from_reads(n("a") + n("b"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r.len(), 2);
        assert!((sigma.sigma_r[&quorum(&["a"])] - 0.5).abs() < 1e-10);
        assert!((sigma.sigma_r[&quorum(&["b"])] - 0.5).abs() < 1e-10);
        assert_eq!(sigma.sigma_w.len(), 1);
        assert!((sigma.sigma_w[&quorum(&["a", "b"])] - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn uniform_strategy_grid() {
        let qs = QuorumSystem::from_reads(n("a") * n("b") + n("c") * n("d"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r.len(), 2);
        assert!((sigma.sigma_r[&quorum(&["a", "b"])] - 0.5).abs() < 1e-10);
        assert!((sigma.sigma_r[&quorum(&["c", "d"])] - 0.5).abs() < 1e-10);
        assert_eq!(sigma.sigma_w.len(), 4);
        for wq in &[
            quorum(&["a", "c"]),
            quorum(&["a", "d"]),
            quorum(&["b", "c"]),
            quorum(&["b", "d"]),
        ] {
            assert!((sigma.sigma_w[wq] - 0.25).abs() < 1e-10);
        }
    }

    #[test]
    fn uniform_strategy_minimizes() {
        // a + a*b should reduce to just {a}
        let qs = QuorumSystem::from_reads(n("a") + n("a") * n("b"));
        let sigma = qs.uniform_strategy(0).expect("ok");
        assert_eq!(sigma.sigma_r.len(), 1);
        assert!((sigma.sigma_r[&quorum(&["a"])] - 1.0).abs() < f64::EPSILON);
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
        assert!((sigma.sigma_r[&quorum(&["a", "b"])] - 0.25).abs() < 1e-10);
        assert!((sigma.sigma_r[&quorum(&["c", "d"])] - 0.75).abs() < 1e-10);
        for wq in &[
            quorum(&["a", "c"]),
            quorum(&["a", "d"]),
            quorum(&["b", "c"]),
            quorum(&["b", "d"]),
        ] {
            assert!((sigma.sigma_w[wq] - 0.25).abs() < 1e-10);
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
        let _lc = 0.8 / 70.0 * 0.25 + 0.2 / 30.0 * (0.1 + 0.3);
        let _ld = 0.8 / 80.0 * 0.25 + 0.2 / 40.0 * (0.2 + 0.4);

        let load_08 = [la, lb, _lc, _ld].iter().copied().fold(0.0_f64, f64::max);

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

        let reads =
            Expr::Node(a) * Expr::Node(b) + Expr::Node(c) * Expr::Node(d) * Expr::Node(e_node);
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
    fn strategy_latency() {
        let a = node_with("a", 1.0, 1.0, 1);
        let b = node_with("b", 1.0, 1.0, 2);
        let c = node_with("c", 1.0, 1.0, 3);
        let d = node_with("d", 1.0, 1.0, 4);
        let e = node_with("e", 1.0, 1.0, 5);

        let reads = Expr::Node(a) * Expr::Node(b.clone())
            + Expr::Node(c.clone()) * Expr::Node(d.clone()) * Expr::Node(e.clone());
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
}
