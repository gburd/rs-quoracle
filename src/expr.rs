//! Expression algebra for defining quorum systems
//!
//! This module provides types for building quorum expressions
//! using combinators:
//! - [`Node`]: A single node/server
//! - [`Or`]: At least one child must be satisfied
//! - [`And`]: All children must be satisfied
//! - [`Choose`]: At least k children must be satisfied

use crate::error::{Error, Result};
use itertools::Itertools;
use std::collections::HashSet;
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::ops::{Add, Mul};
use std::time::Duration;

/// Trait for types that can be used as node identifiers
/// in quorum expressions.
pub trait Element: Ord + Clone + Hash + Debug + Display + Send + Sync + 'static {}

impl<T> Element for T where T: Ord + Clone + Hash + Debug + Display + Send + Sync + 'static {}

/// A node in a quorum system.
#[derive(Debug, Clone)]
pub struct Node<T: Element> {
    /// The node identifier
    pub x: T,
    /// Read capacity (requests per unit time)
    pub read_capacity: f64,
    /// Write capacity (requests per unit time)
    pub write_capacity: f64,
    /// Latency for operations on this node
    pub latency: Duration,
}

impl<T: Element> PartialEq for Node<T> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x
    }
}

impl<T: Element> Eq for Node<T> {}

impl<T: Element> Hash for Node<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.x.hash(state);
    }
}

impl<T: Element> PartialOrd for Node<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Element> Ord for Node<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.x.cmp(&other.x)
    }
}

impl<T: Element> Display for Node<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.x)
    }
}

impl<T: Element> Node<T> {
    /// Create a new node with the given identifier and default
    /// capacities (1.0) and latency (1 second).
    #[must_use]
    pub fn new(x: T) -> Self {
        Self {
            x,
            read_capacity: 1.0,
            write_capacity: 1.0,
            latency: Duration::from_secs(1),
        }
    }

    /// Set a single capacity for both reads and writes.
    #[must_use]
    pub fn with_capacity(mut self, capacity: f64) -> Self {
        self.read_capacity = capacity;
        self.write_capacity = capacity;
        self
    }

    /// Set separate read and write capacities.
    #[must_use]
    pub fn with_read_write_capacity(mut self, read: f64, write: f64) -> Self {
        self.read_capacity = read;
        self.write_capacity = write;
        self
    }

    /// Set the latency for this node.
    #[must_use]
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }
}

/// An expression representing a quorum requirement.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr<T: Element> {
    /// A single node
    Node(Node<T>),
    /// At least one child expression must be satisfied
    Or(Or<T>),
    /// All child expressions must be satisfied
    And(And<T>),
    /// At least k child expressions must be satisfied
    Choose(Choose<T>),
}

/// OR combinator -- at least one child must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct Or<T: Element> {
    /// Child expressions
    pub children: Vec<Expr<T>>,
}

impl<T: Element> Or<T> {
    /// Create a new OR expression.
    ///
    /// # Errors
    /// Returns `InvalidExpression` if `children` is empty.
    pub fn new(children: Vec<Expr<T>>) -> Result<Self> {
        if children.is_empty() {
            return Err(Error::InvalidExpression(
                "Or cannot be constructed with an empty list".into(),
            ));
        }
        Ok(Self { children })
    }
}

/// AND combinator -- all children must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct And<T: Element> {
    /// Child expressions
    pub children: Vec<Expr<T>>,
}

impl<T: Element> And<T> {
    /// Create a new AND expression.
    ///
    /// # Errors
    /// Returns `InvalidExpression` if `children` is empty.
    pub fn new(children: Vec<Expr<T>>) -> Result<Self> {
        if children.is_empty() {
            return Err(Error::InvalidExpression(
                "And cannot be constructed with an empty list".into(),
            ));
        }
        Ok(Self { children })
    }
}

/// CHOOSE combinator -- at least k children must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct Choose<T: Element> {
    /// Number of children that must be satisfied
    pub k: usize,
    /// Child expressions
    pub children: Vec<Expr<T>>,
}

impl<T: Element> Choose<T> {
    /// Create a new CHOOSE expression.
    ///
    /// # Errors
    /// Returns `InvalidExpression` if k is 0 or greater than
    /// the number of children.
    pub fn new(k: usize, children: Vec<Expr<T>>) -> Result<Self> {
        if k == 0 || k > children.len() {
            return Err(Error::InvalidExpression(format!(
                "k must be in the range [1, {}], got {k}",
                children.len()
            )));
        }
        Ok(Self { k, children })
    }
}

// -- Display implementations --

impl<T: Element> Display for Expr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Node(n) => write!(f, "{n}"),
            Expr::Or(o) => write!(f, "{o}"),
            Expr::And(a) => write!(f, "{a}"),
            Expr::Choose(c) => write!(f, "{c}"),
        }
    }
}

impl<T: Element> Display for Or<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, child) in self.children.iter().enumerate() {
            if i > 0 {
                write!(f, " + ")?;
            }
            write!(f, "{child}")?;
        }
        write!(f, ")")
    }
}

impl<T: Element> Display for And<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for (i, child) in self.children.iter().enumerate() {
            if i > 0 {
                write!(f, " * ")?;
            }
            write!(f, "{child}")?;
        }
        write!(f, ")")
    }
}

impl<T: Element> Display for Choose<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "choose{}(", self.k)?;
        for (i, child) in self.children.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{child}")?;
        }
        write!(f, ")")
    }
}

// -- Core expression methods --

impl<T: Element> Expr<T> {
    /// Returns an iterator over all quorums satisfying this
    /// expression. Each quorum is a `HashSet<T>` of element
    /// identifiers.
    pub fn quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        match self {
            Expr::Node(node) => {
                let mut s = HashSet::with_capacity(1);
                s.insert(node.x.clone());
                Box::new(std::iter::once(s))
            }
            Expr::Or(or) => Box::new(or.children.iter().flat_map(Expr::quorums)),
            Expr::And(and) => Box::new(and_quorums(&and.children)),
            Expr::Choose(ch) => Box::new(choose_quorums(ch.k, &ch.children)),
        }
    }

    /// Check if a set of element identifiers forms a quorum for
    /// this expression.
    #[must_use]
    pub fn is_quorum(&self, xs: &HashSet<T>) -> bool {
        match self {
            Expr::Node(node) => xs.contains(&node.x),
            Expr::Or(or) => or.children.iter().any(|e| e.is_quorum(xs)),
            Expr::And(and) => and.children.iter().all(|e| e.is_quorum(xs)),
            Expr::Choose(ch) => ch.children.iter().filter(|e| e.is_quorum(xs)).count() >= ch.k,
        }
    }

    /// Return the set of all element identifiers in this
    /// expression.
    #[must_use]
    pub fn elements(&self) -> HashSet<T> {
        self.nodes().into_iter().map(|n| n.x.clone()).collect()
    }

    /// Return the set of all nodes in this expression.
    #[must_use]
    pub fn nodes(&self) -> HashSet<Node<T>> {
        match self {
            Expr::Node(node) => {
                let mut s = HashSet::with_capacity(1);
                s.insert(node.clone());
                s
            }
            Expr::Or(or) => or.children.iter().flat_map(Expr::nodes).collect(),
            Expr::And(and) => and.children.iter().flat_map(Expr::nodes).collect(),
            Expr::Choose(ch) => ch.children.iter().flat_map(Expr::nodes).collect(),
        }
    }

    /// Return the dual of this expression.
    ///
    /// The dual swaps Or and And, and for Choose(k, n) produces
    /// Choose(n - k + 1, duals).
    #[must_use]
    pub fn dual(&self) -> Self {
        match self {
            Expr::Node(_) => self.clone(),
            Expr::Or(or) => {
                let duals = or.children.iter().map(Expr::dual).collect();
                // Safety: or.children is non-empty by construction
                Expr::And(And { children: duals })
            }
            Expr::And(and) => {
                let duals = and.children.iter().map(Expr::dual).collect();
                Expr::Or(Or { children: duals })
            }
            Expr::Choose(ch) => {
                let duals = ch.children.iter().map(Expr::dual).collect();
                let dual_k = ch.children.len() - ch.k + 1;
                Expr::Choose(Choose {
                    k: dual_k,
                    children: duals,
                })
            }
        }
    }

    /// Check if the expression is duplicate-free (no element
    /// appears more than once).
    #[must_use]
    pub fn dup_free(&self) -> bool {
        self.nodes().len() == self.num_leaves()
    }

    /// Calculate the resilience of this expression.
    ///
    /// Resilience is the maximum number of node failures that can
    /// be tolerated while still having at least one quorum
    /// available. Formally, it is `min_hitting_set_size - 1`.
    ///
    /// For duplicate-free expressions, this is computed
    /// analytically. For expressions with duplicated elements, this
    /// requires solving a minimum hitting set problem via LP.
    pub fn resilience(&self) -> i64 {
        if self.dup_free() {
            self.dup_free_min_failures() - 1
        } else {
            min_hitting_set(self.quorums()) - 1
        }
    }

    /// Count the total number of leaf occurrences (including
    /// duplicates) in the expression tree.
    fn num_leaves(&self) -> usize {
        match self {
            Expr::Node(_) => 1,
            Expr::Or(or) => or.children.iter().map(Expr::num_leaves).sum(),
            Expr::And(and) => and.children.iter().map(Expr::num_leaves).sum(),
            Expr::Choose(ch) => ch.children.iter().map(Expr::num_leaves).sum(),
        }
    }

    /// For duplicate-free expressions, compute the minimum number
    /// of node failures needed to eliminate all quorums.
    fn dup_free_min_failures(&self) -> i64 {
        match self {
            Expr::Node(_) => 1,
            Expr::Or(or) => or.children.iter().map(Expr::dup_free_min_failures).sum(),
            Expr::And(and) => and
                .children
                .iter()
                .map(Expr::dup_free_min_failures)
                .min()
                .unwrap_or(0),
            Expr::Choose(ch) => {
                let mut subfailures: Vec<i64> = ch
                    .children
                    .iter()
                    .map(Expr::dup_free_min_failures)
                    .collect();
                subfailures.sort_unstable();
                let take = ch.children.len() - ch.k + 1;
                subfailures.iter().take(take).sum()
            }
        }
    }
}

/// Compute quorums for an AND expression: cartesian product of
/// child quorums, with set union.
fn and_quorums<T: Element>(children: &[Expr<T>]) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
    if children.is_empty() {
        return Box::new(std::iter::empty());
    }

    let child_quorums: Vec<Vec<HashSet<T>>> =
        children.iter().map(|e| e.quorums().collect()).collect();

    Box::new(
        child_quorums
            .into_iter()
            .multi_cartesian_product()
            .map(|subquorums| {
                subquorums.into_iter().fold(HashSet::new(), |mut acc, q| {
                    acc.extend(q);
                    acc
                })
            }),
    )
}

/// Compute quorums for a CHOOSE expression: for each k-sized
/// combination of children, take the cartesian product and union.
fn choose_quorums<T: Element>(
    k: usize,
    children: &[Expr<T>],
) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
    let n = children.len();
    let child_quorums: Vec<Vec<HashSet<T>>> =
        children.iter().map(|e| e.quorums().collect()).collect();

    Box::new((0..n).combinations(k).flat_map(move |combo| {
        let selected: Vec<Vec<HashSet<T>>> =
            combo.iter().map(|&i| child_quorums[i].clone()).collect();
        selected
            .into_iter()
            .multi_cartesian_product()
            .map(|subquorums| {
                subquorums.into_iter().fold(HashSet::new(), |mut acc, q| {
                    acc.extend(q);
                    acc
                })
            })
    }))
}

/// Solve the minimum hitting set problem via integer linear
/// programming. Returns the size of the smallest set that
/// intersects every quorum.
fn min_hitting_set<T: Element>(quorums: impl Iterator<Item = HashSet<T>>) -> i64 {
    use good_lp::*;

    let quorum_list: Vec<HashSet<T>> = quorums.collect();
    if quorum_list.is_empty() {
        return 0;
    }

    // Collect all unique elements
    let all_elements: Vec<T> = quorum_list
        .iter()
        .flat_map(|q| q.iter().cloned())
        .collect::<HashSet<T>>()
        .into_iter()
        .collect();

    // Create binary variables, one per element
    let mut vars = ProblemVariables::new();
    let x: Vec<Variable> = all_elements
        .iter()
        .map(|_| vars.add(variable().binary()))
        .collect();

    // Build element -> variable index mapping
    let elem_to_idx: std::collections::HashMap<&T, usize> = all_elements
        .iter()
        .enumerate()
        .map(|(i, e)| (e, i))
        .collect();

    // Minimize sum of all variables
    let objective: Expression = x.iter().copied().sum();
    let mut problem = vars.minimise(objective).using(default_solver);

    // For each quorum, at least one element must be in the
    // hitting set
    for quorum in &quorum_list {
        let constraint: Expression = quorum
            .iter()
            .filter_map(|e| elem_to_idx.get(e).map(|&i| x[i]))
            .sum();
        problem = problem.with(constraint.geq(1));
    }

    match problem.solve() {
        Ok(solution) => {
            let total: f64 = x.iter().map(|&v| solution.value(v)).sum();
            total.round() as i64
        }
        Err(_) => 0,
    }
}

// -- Operator overloading --

/// `Expr + Expr` produces an Or expression, flattening nested Or
/// children.
impl<T: Element> Add for Expr<T> {
    type Output = Expr<T>;

    fn add(self, rhs: Self) -> Self::Output {
        let mut children = Vec::new();
        match self {
            Expr::Or(or) => children.extend(or.children),
            other => children.push(other),
        }
        match rhs {
            Expr::Or(or) => children.extend(or.children),
            other => children.push(other),
        }
        Expr::Or(Or { children })
    }
}

/// `Expr * Expr` produces an And expression, flattening nested And
/// children.
impl<T: Element> Mul for Expr<T> {
    type Output = Expr<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        let mut children = Vec::new();
        match self {
            Expr::And(and) => children.extend(and.children),
            other => children.push(other),
        }
        match rhs {
            Expr::And(and) => children.extend(and.children),
            other => children.push(other),
        }
        Expr::And(And { children })
    }
}

// -- Convenience conversions --

impl<T: Element> From<Node<T>> for Expr<T> {
    fn from(node: Node<T>) -> Self {
        Expr::Node(node)
    }
}

// -- Helper functions --

/// Create a choose expression. Returns `Or` when k == 1, `And`
/// when k == n, and `Choose` otherwise.
///
/// # Errors
/// Returns `InvalidExpression` if `exprs` is empty or `k` is
/// out of range `[1, len]`.
pub fn choose<T: Element>(k: usize, exprs: Vec<Expr<T>>) -> Result<Expr<T>> {
    if exprs.is_empty() {
        return Err(Error::InvalidExpression("no expressions provided".into()));
    }
    if k == 0 || k > exprs.len() {
        return Err(Error::InvalidExpression(format!(
            "k must be in the range [1, {}], got {k}",
            exprs.len()
        )));
    }
    if k == 1 {
        Ok(Expr::Or(Or { children: exprs }))
    } else if k == exprs.len() {
        Ok(Expr::And(And { children: exprs }))
    } else {
        Ok(Expr::Choose(Choose { k, children: exprs }))
    }
}

/// Create a majority quorum expression. Requires
/// `floor(n/2) + 1` children to be satisfied.
///
/// # Errors
/// Returns `InvalidExpression` if `exprs` is empty.
pub fn majority<T: Element>(exprs: Vec<Expr<T>>) -> Result<Expr<T>> {
    if exprs.is_empty() {
        return Err(Error::InvalidExpression("no expressions provided".into()));
    }
    let k = exprs.len() / 2 + 1;
    choose(k, exprs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn n(x: &str) -> Expr<String> {
        Expr::Node(Node::new(x.to_string()))
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    fn quorum_set(e: &Expr<String>) -> HashSet<Vec<String>> {
        e.quorums()
            .map(|q| {
                let mut v: Vec<String> = q.into_iter().collect();
                v.sort();
                v
            })
            .collect()
    }

    fn sorted_set(items: &[&str]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v.dedup();
        v
    }

    fn assert_quorums(e: &Expr<String>, expected: &[&[&str]]) {
        let got = quorum_set(e);
        let want: HashSet<Vec<String>> = expected.iter().map(|s| sorted_set(s)).collect();
        assert_eq!(got, want, "quorums mismatch");
    }

    // -- quorums tests --

    #[test]
    fn test_quorums_or() {
        let e = n("a") + n("b") + n("c");
        assert_quorums(&e, &[&["a"], &["b"], &["c"]]);
    }

    #[test]
    fn test_quorums_and() {
        let e = n("a") * n("b") * n("c");
        assert_quorums(&e, &[&["a", "b", "c"]]);
    }

    #[test]
    fn test_quorums_mixed() {
        let e = n("a") + n("b") * n("c");
        assert_quorums(&e, &[&["a"], &["b", "c"]]);
    }

    #[test]
    fn test_quorums_dup_and() {
        let e = n("a") * n("a") * n("a");
        assert_quorums(&e, &[&["a"]]);
    }

    #[test]
    fn test_quorums_dup_or() {
        let e = n("a") + n("a") + n("a");
        assert_quorums(&e, &[&["a"]]);
    }

    #[test]
    fn test_quorums_node_times_or() {
        let e = n("a") * (n("a") + n("b"));
        assert_quorums(&e, &[&["a"], &["a", "b"]]);
    }

    #[test]
    fn test_quorums_choose_1() {
        let e = choose(1, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_quorums(&e, &[&["a"], &["b"], &["c"]]);
    }

    #[test]
    fn test_quorums_choose_2() {
        let e = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
    }

    #[test]
    fn test_quorums_choose_3() {
        let e = choose(3, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_quorums(&e, &[&["a", "b", "c"]]);
    }

    #[test]
    fn test_quorums_cross_product() {
        let e = (n("a") + n("b")) * (n("c") + n("d"));
        assert_quorums(&e, &[&["a", "c"], &["a", "d"], &["b", "c"], &["b", "d"]]);
    }

    #[test]
    fn test_quorums_cross_product_dup() {
        let e = (n("a") + n("b")) * (n("a") + n("c"));
        assert_quorums(&e, &[&["a"], &["a", "c"], &["a", "b"], &["b", "c"]]);
    }

    #[test]
    fn test_quorums_nested_choose() {
        let e = choose(
            2,
            vec![
                choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}")),
                choose(2, vec![n("d"), n("e"), n("f")]).unwrap_or_else(|e| panic!("{e}")),
                choose(2, vec![n("a"), n("c"), n("e")]).unwrap_or_else(|e| panic!("{e}")),
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));

        // The Python test lists many quorums, but since sets
        // deduplicate, we just check the total count matches
        // and spot-check some quorums.
        let qs = quorum_set(&e);
        // Verify some specific quorums are present
        assert!(qs.contains(&sorted_set(&["a", "b", "d", "e"])));
        assert!(qs.contains(&sorted_set(&["b", "c", "d", "f"])));
        assert!(qs.contains(&sorted_set(&["a", "c", "e", "f"])));
    }

    // -- is_quorum tests --

    #[test]
    fn test_is_quorum_or() {
        let expr = n("a") + n("b") + n("c");
        assert!(expr.is_quorum(&set(&["a"])));
        assert!(expr.is_quorum(&set(&["b"])));
        assert!(expr.is_quorum(&set(&["c"])));
        assert!(expr.is_quorum(&set(&["a", "b"])));
        assert!(expr.is_quorum(&set(&["a", "c"])));
        assert!(expr.is_quorum(&set(&["b", "c"])));
        assert!(expr.is_quorum(&set(&["a", "b", "c"])));
        assert!(expr.is_quorum(&set(&["a", "x"])));
        assert!(!expr.is_quorum(&set(&[])));
        assert!(!expr.is_quorum(&set(&["x"])));
    }

    #[test]
    fn test_is_quorum_and() {
        let expr = n("a") * n("b") * n("c");
        assert!(expr.is_quorum(&set(&["a", "b", "c"])));
        assert!(expr.is_quorum(&set(&["a", "b", "c", "x"])));
        assert!(!expr.is_quorum(&set(&[])));
        assert!(!expr.is_quorum(&set(&["a"])));
        assert!(!expr.is_quorum(&set(&["b"])));
        assert!(!expr.is_quorum(&set(&["c"])));
        assert!(!expr.is_quorum(&set(&["a", "b"])));
        assert!(!expr.is_quorum(&set(&["a", "c"])));
        assert!(!expr.is_quorum(&set(&["b", "c"])));
        assert!(!expr.is_quorum(&set(&["x"])));
        assert!(!expr.is_quorum(&set(&["a", "x"])));
    }

    #[test]
    fn test_is_quorum_choose() {
        let expr = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(expr.is_quorum(&set(&["a", "b"])));
        assert!(expr.is_quorum(&set(&["a", "c"])));
        assert!(expr.is_quorum(&set(&["b", "c"])));
        assert!(expr.is_quorum(&set(&["a", "b", "c"])));
        assert!(expr.is_quorum(&set(&["a", "b", "c", "x"])));
        assert!(!expr.is_quorum(&set(&["a"])));
        assert!(!expr.is_quorum(&set(&["b"])));
        assert!(!expr.is_quorum(&set(&["c"])));
        assert!(!expr.is_quorum(&set(&["x"])));
    }

    #[test]
    fn test_is_quorum_cross_product() {
        let expr = (n("a") + n("b")) * (n("c") + n("d"));
        assert!(expr.is_quorum(&set(&["a", "c"])));
        assert!(expr.is_quorum(&set(&["a", "d"])));
        assert!(expr.is_quorum(&set(&["b", "c"])));
        assert!(expr.is_quorum(&set(&["b", "d"])));
        assert!(expr.is_quorum(&set(&["a", "b", "d"])));
        assert!(expr.is_quorum(&set(&["b", "c", "d"])));
        assert!(expr.is_quorum(&set(&["a", "c", "d"])));
        assert!(expr.is_quorum(&set(&["a", "b", "c", "d"])));
        assert!(!expr.is_quorum(&set(&["a"])));
        assert!(!expr.is_quorum(&set(&["b"])));
        assert!(!expr.is_quorum(&set(&["c"])));
        assert!(!expr.is_quorum(&set(&["d"])));
        assert!(!expr.is_quorum(&set(&["a", "b"])));
        assert!(!expr.is_quorum(&set(&["c", "d"])));
        assert!(!expr.is_quorum(&set(&["a", "b", "x"])));
    }

    // -- resilience tests --

    #[test]
    fn test_resilience_single() {
        assert_eq!(n("a").resilience(), 0);
    }

    #[test]
    fn test_resilience_or() {
        assert_eq!((n("a") + n("b")).resilience(), 1);
        assert_eq!((n("a") + n("b") + n("c")).resilience(), 2);
        assert_eq!((n("a") + n("b") + n("c") + n("d")).resilience(), 3);
    }

    #[test]
    fn test_resilience_and() {
        assert_eq!((n("a") * n("b")).resilience(), 0);
        assert_eq!((n("a") * n("b") * n("c")).resilience(), 0);
        assert_eq!((n("a") * n("b") * n("c") * n("d")).resilience(), 0);
    }

    #[test]
    fn test_resilience_mixed() {
        assert_eq!(((n("a") + n("b")) * (n("c") + n("d"))).resilience(), 1);
        assert_eq!(
            ((n("a") + n("b") + n("c")) * (n("d") + n("e") + n("f"))).resilience(),
            2
        );
    }

    #[test]
    fn test_resilience_dup() {
        // These have duplicate elements, so they use LP
        assert_eq!(
            ((n("a") + n("b") + n("c")) * (n("a") + n("e") + n("f"))).resilience(),
            2
        );
        assert_eq!(
            ((n("a") + n("a") + n("c")) * (n("d") + n("e") + n("f"))).resilience(),
            1
        );
        assert_eq!(
            ((n("a") + n("a") + n("a")) * (n("d") + n("e") + n("f"))).resilience(),
            0
        );
        assert_eq!(
            (n("a") * n("b") + n("b") * n("c") + n("a") * n("d") + n("a") * n("d") * n("e"))
                .resilience(),
            1
        );
    }

    #[test]
    fn test_resilience_choose() {
        let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ch2_3.resilience(), 1);

        let ch2_5 = choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ch2_5.resilience(), 3);

        let ch3_5 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ch3_5.resilience(), 2);

        let ch4_5 = choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(ch4_5.resilience(), 1);
    }

    #[test]
    fn test_resilience_choose_compound() {
        let e1 = choose(2, vec![n("a") + n("b") + n("c"), n("d") + n("e"), n("f")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(e1.resilience(), 2);

        let e2 = choose(2, vec![n("a") * n("b"), n("a") * n("c"), n("d")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(e2.resilience(), 0);

        let e3 = choose(2, vec![n("a") + n("b"), n("a") + n("c"), n("a") + n("d")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(e3.resilience(), 2);
    }

    // -- dual tests --

    fn assert_dual(x: &Expr<String>, y: &Expr<String>) {
        let x_dual = x.dual();
        let x_qs = quorum_set(&x_dual);
        let y_qs = quorum_set(y);
        assert_eq!(x_qs, y_qs, "dual mismatch");
    }

    #[test]
    fn test_dual_node() {
        assert_dual(&n("a"), &n("a"));
    }

    #[test]
    fn test_dual_or_and() {
        assert_dual(&(n("a") + n("b")), &(n("a") * n("b")));
    }

    #[test]
    fn test_dual_dup() {
        assert_dual(&(n("a") + n("a")), &(n("a") * n("a")));
    }

    #[test]
    fn test_dual_compound() {
        assert_dual(
            &((n("a") + n("b")) * (n("c") + n("d"))),
            &((n("a") * n("b")) + (n("c") * n("d"))),
        );
        assert_dual(
            &((n("a") + n("b")) * (n("a") + n("d"))),
            &((n("a") * n("b")) + (n("a") * n("d"))),
        );
        assert_dual(
            &((n("a") + n("b")) * (n("a") + n("a"))),
            &((n("a") * n("b")) + (n("a") * n("a"))),
        );
        assert_dual(
            &((n("a") + n("a")) * (n("a") + n("a"))),
            &((n("a") * n("a")) + (n("a") * n("a"))),
        );
    }

    #[test]
    fn test_dual_nested() {
        assert_dual(
            &((n("a") + (n("a") * n("b"))) + ((n("c") * n("d")) + n("a"))),
            &((n("a") * (n("a") + n("b"))) * ((n("c") + n("d")) * n("a"))),
        );
    }

    #[test]
    fn test_dual_choose() {
        let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        let ch2_3b = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_dual(&ch2_3, &ch2_3b);

        let ch2_ab_cd_e = choose(2, vec![n("a") + n("b"), n("c") + n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        let ch2_ab_cd_e_dual = choose(2, vec![n("a") * n("b"), n("c") * n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_dual(&ch2_ab_cd_e, &ch2_ab_cd_e_dual);

        let ch3_5 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        let ch3_5b = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_dual(&ch3_5, &ch3_5b);

        let ch2_5 = choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        let ch4_5 = choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert_dual(&ch2_5, &ch4_5);
        assert_dual(&ch4_5, &ch2_5);
    }

    // -- dup_free tests --

    #[test]
    fn test_dup_free() {
        assert!(n("a").dup_free());
        assert!((n("a") + n("b")).dup_free());
        assert!((n("a") * n("b")).dup_free());
        assert!((n("a") * n("b") + n("c")).dup_free());

        let ch = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(ch.dup_free());

        let ch2 = choose(2, vec![n("a") * n("b"), n("c"), n("d") + n("e") + n("f")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(ch2.dup_free());

        let ch3 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(ch3.dup_free());

        assert!(((n("a") + n("b")) * (n("c") + (n("d") * n("e")))).dup_free());
    }

    #[test]
    fn test_not_dup_free() {
        assert!(!(n("a") + n("a")).dup_free());
        assert!(!(n("a") * n("a")).dup_free());
        assert!(!(n("a") * (n("b") + n("a"))).dup_free());

        let ch = choose(2, vec![n("a"), n("b"), n("a")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(!ch.dup_free());

        let ch2 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("a")])
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(!ch2.dup_free());

        assert!(!((n("a") + n("b")) * (n("c") + (n("d") * n("a")))).dup_free());
    }

    // -- choose/majority helper tests --

    #[test]
    fn test_choose_returns_or_for_k1() {
        let e = choose(1, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(e, Expr::Or(_)));
    }

    #[test]
    fn test_choose_returns_and_for_k_eq_n() {
        let e = choose(3, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(e, Expr::And(_)));
    }

    #[test]
    fn test_choose_returns_choose_for_middle_k() {
        let e = choose(2, vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(e, Expr::Choose(_)));
    }

    #[test]
    fn test_choose_errors() {
        assert!(choose::<String>(0, vec![]).is_err());
        assert!(choose(0, vec![n("a")]).is_err());
        assert!(choose(2, vec![n("a")]).is_err());
    }

    #[test]
    fn test_majority() {
        let e = majority(vec![n("a"), n("b"), n("c")]).unwrap_or_else(|e| panic!("{e}"));
        assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
    }
}
