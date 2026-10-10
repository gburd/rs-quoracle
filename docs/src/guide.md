# Guide

This follows the tutorial of the original Python Quoracle.

## Quorum systems

Given nodes `X`, a read-write quorum system is a pair `(R, W)` of sets of
subsets of `X` (read quorums and write quorums) such that every read quorum
intersects every write quorum. Quoracle describes quorums with
expressions: `a + b` is "a or b", `a * b` is "a and b", and
`choose(k, [...])` is "any k of".

```rust
use quoracle::{choose, majority, Expr, Node, QuorumSystem};

# fn main() -> Result<(), quoracle::Error> {
let [a, b, c, d, e, f] =
    ["a", "b", "c", "d", "e", "f"].map(|x| Expr::Node(Node::new(x)));

// Reads need a full row; writes (the dual) need one node from each row.
let grid = QuorumSystem::from_reads(a.clone() * b.clone() * c.clone()
    + d.clone() * e.clone() * f.clone());
assert_eq!(grid.read_quorums().count(), 2);
assert_eq!(grid.write_quorums().count(), 9);

// Any 2 of 3 for both reads and writes.
let maj = QuorumSystem::from_reads(majority(vec![a.clone(), b.clone(), c.clone()])?);
assert_eq!(maj.resilience(), 1);

// Explicit reads and writes are checked for safety.
assert!(QuorumSystem::new(a.clone() + b.clone(), a.clone()).is_err());
let _ = choose(2, vec![d, e, f])?;
# Ok(())
# }
```

`from_reads` sets the writes to the *dual* of the reads, the smallest
write rule that intersects every read quorum. `from_writes` does the
reverse.

## Resilience

The resilience is the number of nodes that can fail while some quorum is
still alive. `QuorumSystem::resilience` is the minimum of the read and
write resilience.

## Strategies, load, and capacity

A strategy assigns a probability to each read quorum and each write
quorum. A node's load is the fraction of operations it serves, divided by
its capacity. The system's load is the load of its busiest node, and its
capacity is `1 / load`: how much throughput it sustains relative to a
single unit-capacity node.

```rust
use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem, StrategyLimits};

# fn main() -> Result<(), quoracle::Error> {
let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
let grid = QuorumSystem::from_reads(a * b + c * d);
let none = StrategyLimits::default();

// Optimal for 100% reads: alternate rows, each node serves half of reads.
let fr = Distribution::fixed(1.0)?;
let s = grid.strategy(Objective::Load, Some(&fr), None, &none, 0)?;
assert!((s.load(Some(&fr), None)? - 0.5).abs() < 1e-6);

// Optimize for a mix: 25% reads 1/3 of the time, 75% reads 2/3.
let mix = Distribution::weighted(&[(0.25, 1.0), (0.75, 2.0)])?;
let s = grid.strategy(Objective::Load, Some(&mix), None, &none, 0)?;
println!("{s}");
# Ok(())
# }
```

## Heterogeneous nodes

Capacities are requests per unit time and can differ between reads and
writes.

```rust
use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem, StrategyLimits};

# fn main() -> Result<(), quoracle::Error> {
let big = |x| Node::new(x).with_read_write_capacity(2000.0, 1000.0);
let small = |x| Node::new(x).with_read_write_capacity(1000.0, 500.0);
let [a, c] = ["a", "c"].map(|x| big(x).map(Expr::Node));
let [b, d] = ["b", "d"].map(|x| small(x).map(Expr::Node));
let grid = QuorumSystem::from_reads(a? * b? + c? * d?);

let fr = Distribution::fixed(0.9)?;
let s = grid.strategy(Objective::Load, Some(&fr), None, &StrategyLimits::default(), 0)?;
println!("capacity: {:.0} ops/s", s.capacity(Some(&fr), None)?);
# Ok(())
# }
```

## Network load, latency, and limits

`Objective::Network` minimizes the expected number of nodes contacted;
`Objective::Latency` minimizes expected latency (each node's latency is set
with `Node::with_latency`; a quorum's latency is when its fastest nodes form
a quorum). `StrategyLimits` bounds the metrics you are not optimizing:

```rust
use quoracle::{Distribution, Error, Expr, Node, Objective, QuorumSystem, StrategyLimits};

# fn main() -> Result<(), quoracle::Error> {
let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
let grid = QuorumSystem::from_reads(a * b + c * d);
let fr = Distribution::fixed(0.5)?;

let limits = StrategyLimits { load: Some(0.75), ..Default::default() };
let s = grid.strategy(Objective::Network, Some(&fr), None, &limits, 0)?;
assert!(s.load(Some(&fr), None)? <= 0.75 + 1e-9);

// Impossible limits are reported, not ignored.
let tight = StrategyLimits { load: Some(0.1), ..Default::default() };
assert_eq!(
    grid.strategy(Objective::Network, Some(&fr), None, &tight, 0).unwrap_err(),
    Error::NoStrategyFound
);
# Ok(())
# }
```

## f-resilient strategies

With `f > 0`, strategies only use quorums that remain quorums after any
`f` of their nodes fail, so an operation can tolerate `f` slow or failed
nodes without picking a new quorum. This costs capacity.

## Known failed nodes

When failures are already known, `strategy_with_failures` optimizes using
only surviving nodes. It keeps the original read and write rules, node
capacities, and latencies. `f` still specifies how many further failures
each selected quorum must tolerate.

```rust
use quoracle::hashbrown::HashSet;
use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem, StrategyLimits};

# fn main() -> Result<(), quoracle::Error> {
let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
let grid = QuorumSystem::from_reads(a * b + c * d);
let fr = Distribution::fixed(0.75)?;
let failed = HashSet::from(["a"]);
let s = grid.strategy_with_failures(
    Objective::Load, Some(&fr), None, &StrategyLimits::default(), 0, &failed,
)?;

// Reads use {c, d}; writes alternate between {b, c} and {b, d}.
assert!((s.load(Some(&fr), None)? - 0.875).abs() < 1e-6);
assert_eq!(s.node_load(&Node::new("a"), Some(&fr), None)?, 0.0);
assert!(s.get_read_quorum().is_disjoint(&failed));
assert!(s.get_write_quorum().is_disjoint(&failed));
# Ok(())
# }
```

Recompute the strategy when the known failed set changes. An empty set is
equivalent to `strategy`. Unknown node identifiers return
`Error::InvalidQuorumSystem`; losing all read or write quorums, or being
unable to tolerate `f` further failures, returns `Error::NoStrategyFound`.

## Search

`search` tries every quorum system over the given nodes and returns the
best one. It is exhaustive, so use a timeout above about 6 nodes.

```rust
use quoracle::{search, Distribution, Node, SearchConfig};
use std::time::Duration;

# fn main() -> Result<(), quoracle::Error> {
let nodes: Vec<Node<u32>> = (0..4).map(Node::new).collect();
let config = SearchConfig {
    resilience: 1,
    read_fraction: Some(Distribution::fixed(0.5)?),
    timeout: Duration::from_secs(5),
    ..Default::default()
};
let best = search(&nodes, &config)?;
println!("{}", best.quorum_system);
# Ok(())
# }
```
