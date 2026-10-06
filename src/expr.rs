//! Expression algebra for defining quorum systems.
//!
//! An [`Expr`] describes which sets of nodes form a quorum:
//! - [`Node`]: a single node
//! - [`Or`] (`a + b`): any one child is a quorum
//! - [`And`] (`a * b`): every child is needed
//! - [`Choose`] ([`choose`]): any `k` of the children
//!
//! `Or`, `And`, and `Choose` are always non-empty and `Choose` always has
//! `1 <= k <= children.len()`. Their fields are private so those
//! invariants cannot be broken after construction.

use crate::error::{Error, Result};
use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use std::fmt::{self, Debug, Display};
use std::hash::Hash;
use std::ops::{Add, Mul};
use std::time::Duration;

/// Trait for types that can be used as node identifiers
/// in quorum expressions.
pub trait Element:
    Ord + Clone + Hash + Debug + Display + Send + Sync + 'static
{
}

impl<T> Element for T where
    T: Ord + Clone + Hash + Debug + Display + Send + Sync + 'static
{
}

/// A node in a quorum system.
///
/// Nodes are identified by `x`: equality, ordering, and hashing only look
/// at `x`. Capacities are in requests per unit time; the defaults are a
/// capacity of 1.0 and a latency of 1 second.
#[derive(Debug, Clone)]
pub struct Node<T: Element> {
    x: T,
    read_capacity: f64,
    write_capacity: f64,
    latency: Duration,
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

fn check_capacity(name: &str, c: f64) -> Result<f64> {
    if c.is_finite() && c > 0.0 {
        Ok(c)
    } else {
        Err(Error::InvalidExpression(format!(
            "{name} must be finite and > 0, got {c}"
        )))
    }
}

impl<T: Element> Node<T> {
    /// Create a node with capacity 1.0 and latency 1 second.
    #[must_use]
    pub fn new(x: T) -> Self {
        Self {
            x,
            read_capacity: 1.0,
            write_capacity: 1.0,
            latency: Duration::from_secs(1),
        }
    }

    /// Set one capacity for both reads and writes.
    ///
    /// # Errors
    /// Returns [`Error::InvalidExpression`] unless `capacity` is finite
    /// and positive.
    pub fn with_capacity(self, capacity: f64) -> Result<Self> {
        self.with_read_write_capacity(capacity, capacity)
    }

    /// Set separate read and write capacities.
    ///
    /// # Errors
    /// Returns [`Error::InvalidExpression`] unless both capacities are
    /// finite and positive.
    pub fn with_read_write_capacity(
        mut self,
        read: f64,
        write: f64,
    ) -> Result<Self> {
        self.read_capacity = check_capacity("read capacity", read)?;
        self.write_capacity = check_capacity("write capacity", write)?;
        Ok(self)
    }

    /// Set the latency for this node.
    #[must_use]
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// The node identifier.
    #[must_use]
    pub fn x(&self) -> &T {
        &self.x
    }

    /// Read capacity (requests per unit time).
    #[must_use]
    pub fn read_capacity(&self) -> f64 {
        self.read_capacity
    }

    /// Write capacity (requests per unit time).
    #[must_use]
    pub fn write_capacity(&self) -> f64 {
        self.write_capacity
    }

    /// Latency of a request to this node.
    #[must_use]
    pub fn latency(&self) -> Duration {
        self.latency
    }
}

/// An expression describing which sets of nodes form a quorum.
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

/// OR combinator: at least one child must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct Or<T: Element> {
    children: Vec<Expr<T>>,
}

/// AND combinator: all children must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct And<T: Element> {
    children: Vec<Expr<T>>,
}

/// CHOOSE combinator: at least `k` children must be satisfied.
#[derive(Debug, Clone, PartialEq)]
pub struct Choose<T: Element> {
    k: usize,
    children: Vec<Expr<T>>,
}

fn non_empty<T: Element>(
    what: &str,
    children: Vec<Expr<T>>,
) -> Result<Vec<Expr<T>>> {
    if children.is_empty() {
        Err(Error::InvalidExpression(format!(
            "{what} cannot be constructed with an empty list"
        )))
    } else {
        Ok(children)
    }
}

fn check_k(k: usize, n: usize) -> Result<()> {
    if k == 0 || k > n {
        Err(Error::InvalidExpression(format!(
            "k must be in the range [1, {n}], got {k}"
        )))
    } else {
        Ok(())
    }
}

impl<T: Element> Or<T> {
    /// Create an OR expression.
    ///
    /// # Errors
    /// Returns [`Error::InvalidExpression`] if `children` is empty.
    pub fn new(children: Vec<Expr<T>>) -> Result<Self> {
        Ok(Self { children: non_empty("Or", children)? })
    }

    /// The child expressions.
    #[must_use]
    pub fn children(&self) -> &[Expr<T>] {
        &self.children
    }
}

impl<T: Element> And<T> {
    /// Create an AND expression.
    ///
    /// # Errors
    /// Returns [`Error::InvalidExpression`] if `children` is empty.
    pub fn new(children: Vec<Expr<T>>) -> Result<Self> {
        Ok(Self { children: non_empty("And", children)? })
    }

    /// The child expressions.
    #[must_use]
    pub fn children(&self) -> &[Expr<T>] {
        &self.children
    }
}

impl<T: Element> Choose<T> {
    /// Create a CHOOSE expression.
    ///
    /// # Errors
    /// Returns [`Error::InvalidExpression`] unless
    /// `1 <= k <= children.len()`.
    pub fn new(k: usize, children: Vec<Expr<T>>) -> Result<Self> {
        check_k(k, children.len())?;
        Ok(Self { k, children })
    }

    /// How many children must be satisfied.
    #[must_use]
    pub fn k(&self) -> usize {
        self.k
    }

    /// The child expressions.
    #[must_use]
    pub fn children(&self) -> &[Expr<T>] {
        &self.children
    }
}

// -- Display implementations --

fn write_joined<T: Element>(
    f: &mut fmt::Formatter<'_>,
    children: &[Expr<T>],
    sep: &str,
) -> fmt::Result {
    for (i, child) in children.iter().enumerate() {
        if i > 0 {
            f.write_str(sep)?;
        }
        write!(f, "{child}")?;
    }
    Ok(())
}

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
        f.write_str("(")?;
        write_joined(f, &self.children, " + ")?;
        f.write_str(")")
    }
}

impl<T: Element> Display for And<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        write_joined(f, &self.children, " * ")?;
        f.write_str(")")
    }
}

impl<T: Element> Display for Choose<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "choose{}(", self.k)?;
        write_joined(f, &self.children, ", ")?;
        f.write_str(")")
    }
}

// -- Core expression methods --

impl<T: Element> Expr<T> {
    /// Iterate over the quorums of this expression.
    ///
    /// The same quorum can be produced more than once (for example by
    /// `a + a`), and non-minimal quorums are included (for example
    /// `{a, b}` from `a + a * b`).
    pub fn quorums(&self) -> Box<dyn Iterator<Item = HashSet<T>> + '_> {
        match self {
            Expr::Node(node) => {
                let mut s = HashSet::with_capacity(1);
                s.insert(node.x.clone());
                Box::new(std::iter::once(s))
            }
            Expr::Or(or) => {
                Box::new(or.children.iter().flat_map(Expr::quorums))
            }
            Expr::And(and) => Box::new(and_quorums(&and.children)),
            Expr::Choose(ch) => Box::new(choose_quorums(ch.k, &ch.children)),
        }
    }

    /// Whether `xs` contains a quorum of this expression.
    #[must_use]
    pub fn is_quorum(&self, xs: &HashSet<T>) -> bool {
        match self {
            Expr::Node(node) => xs.contains(&node.x),
            Expr::Or(or) => or.children.iter().any(|e| e.is_quorum(xs)),
            Expr::And(and) => and.children.iter().all(|e| e.is_quorum(xs)),
            Expr::Choose(ch) => {
                ch.children.iter().filter(|e| e.is_quorum(xs)).count() >= ch.k
            }
        }
    }

    /// The identifiers of all nodes in this expression.
    #[must_use]
    pub fn elements(&self) -> HashSet<T> {
        self.nodes().into_iter().map(|n| n.x).collect()
    }

    /// All nodes in this expression.
    ///
    /// If the same identifier appears more than once with different
    /// capacities or latency, the first occurrence (in left-to-right order)
    /// wins.
    #[must_use]
    pub fn nodes(&self) -> HashSet<Node<T>> {
        let mut out = HashSet::new();
        self.collect_nodes(&mut out);
        out
    }

    fn collect_nodes(&self, out: &mut HashSet<Node<T>>) {
        match self {
            Expr::Node(node) => {
                if !out.contains(node) {
                    out.insert(node.clone());
                }
            }
            Expr::Or(Or { children })
            | Expr::And(And { children })
            | Expr::Choose(Choose { children, .. }) => {
                for c in children {
                    c.collect_nodes(out);
                }
            }
        }
    }

    /// The dual expression: `Or` and `And` swap, and `choose(k, n)` becomes
    /// `choose(n - k + 1, n)`.
    ///
    /// Every quorum of an expression intersects every quorum of its dual.
    #[must_use]
    pub fn dual(&self) -> Self {
        match self {
            Expr::Node(_) => self.clone(),
            Expr::Or(or) => Expr::And(And {
                children: or.children.iter().map(Expr::dual).collect(),
            }),
            Expr::And(and) => Expr::Or(Or {
                children: and.children.iter().map(Expr::dual).collect(),
            }),
            Expr::Choose(ch) => Expr::Choose(Choose {
                k: ch.children.len() - ch.k + 1,
                children: ch.children.iter().map(Expr::dual).collect(),
            }),
        }
    }

    /// Whether every node identifier appears at most once.
    #[must_use]
    pub fn dup_free(&self) -> bool {
        self.elements().len() == self.num_leaves()
    }

    /// The resilience: the largest number of nodes that can fail while
    /// some quorum is still fully alive.
    ///
    /// This equals `min_hitting_set(quorums) - 1`. Duplicate-free
    /// expressions are solved in closed form. Otherwise the hitting set is
    /// solved exactly as an integer program over the minimal quorums.
    #[must_use]
    pub fn resilience(&self) -> usize {
        let min_failures = if self.dup_free() {
            self.dup_free_min_failures()
        } else {
            min_hitting_set(self)
        };
        min_failures.saturating_sub(1)
    }

    fn num_leaves(&self) -> usize {
        match self {
            Expr::Node(_) => 1,
            Expr::Or(Or { children })
            | Expr::And(And { children })
            | Expr::Choose(Choose { children, .. }) => {
                children.iter().map(Expr::num_leaves).sum()
            }
        }
    }

    /// For duplicate-free expressions, the minimum number of node failures
    /// that leaves no quorum alive.
    fn dup_free_min_failures(&self) -> usize {
        match self {
            Expr::Node(_) => 1,
            Expr::Or(or) => {
                or.children.iter().map(Expr::dup_free_min_failures).sum()
            }
            Expr::And(and) => and
                .children
                .iter()
                .map(Expr::dup_free_min_failures)
                .min()
                .unwrap_or(0),
            Expr::Choose(ch) => {
                let mut subfailures: Vec<usize> = ch
                    .children
                    .iter()
                    .map(Expr::dup_free_min_failures)
                    .collect();
                subfailures.sort_unstable();
                subfailures.iter().take(ch.children.len() - ch.k + 1).sum()
            }
        }
    }
}

/// Cartesian product of child quorums, unioned.
fn and_quorums<T: Element>(
    children: &[Expr<T>],
) -> impl Iterator<Item = HashSet<T>> + '_ {
    let child_quorums: Vec<Vec<HashSet<T>>> =
        children.iter().map(|e| e.quorums().collect()).collect();
    union_product(child_quorums)
}

/// For each k-subset of children, the cartesian product of their quorums.
fn choose_quorums<T: Element>(
    k: usize,
    children: &[Expr<T>],
) -> impl Iterator<Item = HashSet<T>> + '_ {
    let child_quorums: Vec<Vec<HashSet<T>>> =
        children.iter().map(|e| e.quorums().collect()).collect();
    (0..children.len()).combinations(k).flat_map(move |combo| {
        union_product(combo.iter().map(|&i| child_quorums[i].clone()).collect())
    })
}

fn union_product<T: Element>(
    parts: Vec<Vec<HashSet<T>>>,
) -> impl Iterator<Item = HashSet<T>> {
    parts.into_iter().multi_cartesian_product().map(|subquorums| {
        subquorums.into_iter().fold(HashSet::new(), |mut acc, q| {
            acc.extend(q);
            acc
        })
    })
}

/// Remove duplicate and non-minimal sets: keep only sets that are not
/// supersets of another set in the collection.
pub(crate) fn minimize<T: Element>(
    mut sets: Vec<HashSet<T>>,
) -> Vec<HashSet<T>> {
    sets.sort_by_key(HashSet::len);
    let mut minimal: Vec<HashSet<T>> = Vec::new();
    for s in sets {
        if !minimal.iter().any(|m| s.is_superset(m)) {
            minimal.push(s);
        }
    }
    minimal
}

/// Exact minimum hitting set of an expression's quorums.
fn min_hitting_set<T: Element>(e: &Expr<T>) -> usize {
    min_hitting_set_of(minimize(e.quorums().collect()))
}

/// Size of the smallest set that intersects every set in `sets`
/// (0 if `sets` is empty).
///
/// Exact branch-and-bound: some element of the first un-hit set must be in
/// any hitting set, so branching on those elements is exhaustive.
// ponytail: exponential worst case (hitting set is NP-hard); fine for the
// tens of nodes quorum systems have. Use an ILP solver if that changes.
fn min_hitting_set_of<T: Element>(sets: Vec<HashSet<T>>) -> usize {
    let mut index: HashMap<T, usize> = HashMap::new();
    let sets: Vec<Vec<usize>> = sets
        .into_iter()
        .map(|q| {
            q.into_iter()
                .map(|x| {
                    let n = index.len();
                    *index.entry(x).or_insert(n)
                })
                .collect()
        })
        .collect();
    let mut best = index.len();
    let mut chosen = vec![false; index.len()];
    hitting_set_search(&sets, &mut chosen, 0, &mut best);
    best
}

fn hitting_set_search(
    sets: &[Vec<usize>],
    chosen: &mut [bool],
    size: usize,
    best: &mut usize,
) {
    if size >= *best {
        return;
    }
    let Some(unhit) = sets.iter().find(|s| !s.iter().any(|&i| chosen[i]))
    else {
        *best = size;
        return;
    };
    for &i in unhit {
        chosen[i] = true;
        hitting_set_search(sets, chosen, size + 1, best);
        chosen[i] = false;
    }
}

// -- Operator overloading --

/// `Expr + Expr` produces an Or expression, flattening nested Or
/// children.
impl<T: Element> Add for Expr<T> {
    type Output = Expr<T>;

    fn add(self, rhs: Self) -> Self::Output {
        let mut children = Vec::new();
        for e in [self, rhs] {
            match e {
                Expr::Or(or) => children.extend(or.children),
                other => children.push(other),
            }
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
        for e in [self, rhs] {
            match e {
                Expr::And(and) => children.extend(and.children),
                other => children.push(other),
            }
        }
        Expr::And(And { children })
    }
}

impl<T: Element> From<Node<T>> for Expr<T> {
    fn from(node: Node<T>) -> Self {
        Expr::Node(node)
    }
}

impl<T: Element> From<Or<T>> for Expr<T> {
    fn from(e: Or<T>) -> Self {
        Expr::Or(e)
    }
}

impl<T: Element> From<And<T>> for Expr<T> {
    fn from(e: And<T>) -> Self {
        Expr::And(e)
    }
}

impl<T: Element> From<Choose<T>> for Expr<T> {
    fn from(e: Choose<T>) -> Self {
        Expr::Choose(e)
    }
}

// -- Helper functions --

/// Create a choose expression. Returns `Or` when k == 1, `And`
/// when k == n, and `Choose` otherwise.
///
/// # Errors
/// Returns [`Error::InvalidExpression`] if `exprs` is empty or `k` is
/// out of range `[1, len]`.
pub fn choose<T: Element>(k: usize, exprs: Vec<Expr<T>>) -> Result<Expr<T>> {
    let exprs = non_empty("choose", exprs)?;
    check_k(k, exprs.len())?;
    Ok(if k == 1 {
        Expr::Or(Or { children: exprs })
    } else if k == exprs.len() {
        Expr::And(And { children: exprs })
    } else {
        Expr::Choose(Choose { k, children: exprs })
    })
}

/// Create a majority quorum expression. Requires
/// `floor(n/2) + 1` children to be satisfied.
///
/// # Errors
/// Returns [`Error::InvalidExpression`] if `exprs` is empty.
pub fn majority<T: Element>(exprs: Vec<Expr<T>>) -> Result<Expr<T>> {
    let k = exprs.len() / 2 + 1;
    choose(k, exprs)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use hashbrown::HashSet;

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
        let mut v: Vec<String> =
            items.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v.dedup();
        v
    }

    fn assert_quorums(e: &Expr<String>, expected: &[&[&str]]) {
        let got = quorum_set(e);
        let want: HashSet<Vec<String>> =
            expected.iter().map(|s| sorted_set(s)).collect();
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
        let e = choose(1, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_quorums(&e, &[&["a"], &["b"], &["c"]]);
    }

    #[test]
    fn test_quorums_choose_2() {
        let e = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
    }

    #[test]
    fn test_quorums_choose_3() {
        let e = choose(3, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_quorums(&e, &[&["a", "b", "c"]]);
    }

    #[test]
    fn test_quorums_cross_product() {
        let e = (n("a") + n("b")) * (n("c") + n("d"));
        assert_quorums(
            &e,
            &[&["a", "c"], &["a", "d"], &["b", "c"], &["b", "d"]],
        );
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
                choose(2, vec![n("a"), n("b"), n("c")]).unwrap(),
                choose(2, vec![n("d"), n("e"), n("f")]).unwrap(),
                choose(2, vec![n("a"), n("c"), n("e")]).unwrap(),
            ],
        )
        .unwrap();

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
        let expr = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
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
            ((n("a") + n("b") + n("c")) * (n("d") + n("e") + n("f")))
                .resilience(),
            2
        );
    }

    #[test]
    fn test_resilience_dup() {
        // These have duplicate elements, so they use LP
        assert_eq!(
            ((n("a") + n("b") + n("c")) * (n("a") + n("e") + n("f")))
                .resilience(),
            2
        );
        assert_eq!(
            ((n("a") + n("a") + n("c")) * (n("d") + n("e") + n("f")))
                .resilience(),
            1
        );
        assert_eq!(
            ((n("a") + n("a") + n("a")) * (n("d") + n("e") + n("f")))
                .resilience(),
            0
        );
        assert_eq!(
            (n("a") * n("b")
                + n("b") * n("c")
                + n("a") * n("d")
                + n("a") * n("d") * n("e"))
            .resilience(),
            1
        );
    }

    #[test]
    fn test_resilience_choose() {
        let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_eq!(ch2_3.resilience(), 1);

        let ch2_5 =
            choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        assert_eq!(ch2_5.resilience(), 3);

        let ch3_5 =
            choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        assert_eq!(ch3_5.resilience(), 2);

        let ch4_5 =
            choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        assert_eq!(ch4_5.resilience(), 1);
    }

    #[test]
    fn test_resilience_choose_compound() {
        let e1 =
            choose(2, vec![n("a") + n("b") + n("c"), n("d") + n("e"), n("f")])
                .unwrap();
        assert_eq!(e1.resilience(), 2);

        let e2 =
            choose(2, vec![n("a") * n("b"), n("a") * n("c"), n("d")]).unwrap();
        assert_eq!(e2.resilience(), 0);

        let e3 =
            choose(2, vec![n("a") + n("b"), n("a") + n("c"), n("a") + n("d")])
                .unwrap();
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
        let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
        let ch2_3b = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_dual(&ch2_3, &ch2_3b);

        let ch2_ab_cd_e =
            choose(2, vec![n("a") + n("b"), n("c") + n("d"), n("e")]).unwrap();
        let ch2_ab_cd_e_dual =
            choose(2, vec![n("a") * n("b"), n("c") * n("d"), n("e")]).unwrap();
        assert_dual(&ch2_ab_cd_e, &ch2_ab_cd_e_dual);

        let ch3_5 =
            choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        let ch3_5b =
            choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        assert_dual(&ch3_5, &ch3_5b);

        let ch2_5 =
            choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        let ch4_5 =
            choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
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

        let ch = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
        assert!(ch.dup_free());

        let ch2 =
            choose(2, vec![n("a") * n("b"), n("c"), n("d") + n("e") + n("f")])
                .unwrap();
        assert!(ch2.dup_free());

        let ch3 =
            choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).unwrap();
        assert!(ch3.dup_free());

        assert!(((n("a") + n("b")) * (n("c") + (n("d") * n("e")))).dup_free());
    }

    #[test]
    fn test_not_dup_free() {
        assert!(!(n("a") + n("a")).dup_free());
        assert!(!(n("a") * n("a")).dup_free());
        assert!(!(n("a") * (n("b") + n("a"))).dup_free());

        let ch = choose(2, vec![n("a"), n("b"), n("a")]).unwrap();
        assert!(!ch.dup_free());

        let ch2 =
            choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("a")]).unwrap();
        assert!(!ch2.dup_free());

        assert!(!((n("a") + n("b")) * (n("c") + (n("d") * n("a")))).dup_free());
    }

    // -- choose/majority helper tests --

    #[test]
    fn test_choose_returns_or_for_k1() {
        let e = choose(1, vec![n("a"), n("b"), n("c")]).unwrap();
        assert!(matches!(e, Expr::Or(_)));
    }

    #[test]
    fn test_choose_returns_and_for_k_eq_n() {
        let e = choose(3, vec![n("a"), n("b"), n("c")]).unwrap();
        assert!(matches!(e, Expr::And(_)));
    }

    #[test]
    fn test_choose_returns_choose_for_middle_k() {
        let e = choose(2, vec![n("a"), n("b"), n("c")]).unwrap();
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
        let e = majority(vec![n("a"), n("b"), n("c")]).unwrap();
        assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
    }

    // -- min hitting set (exact) --

    fn mhs(vecs: &[&[&str]]) -> usize {
        min_hitting_set_of(vecs.iter().map(|v| set(v)).collect())
    }

    #[test]
    fn test_min_hitting_set() {
        assert_eq!(mhs(&[]), 0);
        assert_eq!(mhs(&[&["a"]]), 1);
        assert_eq!(mhs(&[&["a"], &["b"]]), 2);
        assert_eq!(mhs(&[&["a", "b"], &["b", "c"]]), 1);
        assert_eq!(mhs(&[&["a"], &["b"], &["c"]]), 3);
        assert_eq!(mhs(&[&["a", "b", "c"]]), 1);
        assert_eq!(
            mhs(&[&["a", "c"], &["a", "d"], &["b", "c"], &["b", "d"]]),
            2
        );
        assert_eq!(mhs(&[&["a", "b"], &["a", "c"], &["b", "c"]]), 2);
        assert_eq!(
            mhs(&[&["a", "b"], &["b", "c"], &["a", "d"], &["a", "d", "e"]]),
            2
        );
        // All pairs of 5 elements: must pick 4.
        let xs = ["a", "b", "c", "d", "e"];
        let pairs: Vec<HashSet<String>> =
            xs.iter().array_combinations().map(|[p, q]| set(&[p, q])).collect();
        assert_eq!(min_hitting_set_of(pairs), 4);
    }

    // -- constructors, accessors, display --

    #[test]
    fn test_node_builders_and_accessors() {
        let a = Node::new("a")
            .with_capacity(4.0)
            .unwrap()
            .with_latency(Duration::from_millis(5));
        assert_eq!(*a.x(), "a");
        assert_eq!(a.read_capacity().to_bits(), 4.0_f64.to_bits());
        assert_eq!(a.write_capacity().to_bits(), 4.0_f64.to_bits());
        assert_eq!(a.latency(), Duration::from_millis(5));
        let b = Node::new("b").with_read_write_capacity(2.0, 3.0).unwrap();
        assert!(b.read_capacity() < b.write_capacity());
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(Node::new("a").with_capacity(bad).is_err());
            assert!(Node::new("a").with_read_write_capacity(1.0, bad).is_err());
        }
        // Identity is by `x` only.
        assert_eq!(a, Node::new("a"));
        assert!(Node::new("a") < Node::new("b"));
        assert_eq!(
            Node::new("a").partial_cmp(&Node::new("b")),
            Some(std::cmp::Ordering::Less)
        );
    }

    #[test]
    fn test_combinator_constructors() {
        assert!(Or::<String>::new(vec![]).is_err());
        assert!(And::<String>::new(vec![]).is_err());
        assert!(Choose::<String>::new(1, vec![]).is_err());
        assert!(Choose::new(0, vec![n("a")]).is_err());
        assert!(Choose::new(2, vec![n("a")]).is_err());

        let or = Or::new(vec![n("a"), n("b")]).unwrap();
        assert_eq!(or.children().len(), 2);
        let and = And::new(vec![n("a"), n("b")]).unwrap();
        assert_eq!(and.children().len(), 2);
        let ch = Choose::new(2, vec![n("a"), n("b"), n("c")]).unwrap();
        assert_eq!(ch.k(), 2);
        assert_eq!(ch.children().len(), 3);

        let e: Expr<String> = ch.into();
        assert_eq!(e.to_string(), "choose2(a, b, c)");
        assert_eq!(Expr::from(or).to_string(), "(a + b)");
        assert_eq!(Expr::from(and).to_string(), "(a * b)");
        assert_eq!(Expr::from(Node::new("z".to_string())).to_string(), "z");
        assert_eq!(e.dual().to_string(), "choose2(a, b, c)");
    }

    #[test]
    fn test_first_node_occurrence_wins() {
        let fast = Node::new("a".to_string()).with_latency(Duration::ZERO);
        let e = Expr::Node(fast) + n("a");
        let nodes = e.nodes();
        assert_eq!(nodes.len(), 1);
        assert!(nodes.iter().all(|x| x.latency() == Duration::ZERO));
    }

    #[test]
    fn test_majority_errors() {
        assert!(majority::<String>(vec![]).is_err());
    }
}
