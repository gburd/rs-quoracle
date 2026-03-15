# Examples

Practical examples demonstrating common Quoracle usage patterns.

## Simple Majority Quorum

```rust
use quoracle::*;

let a = Expr::Node(Node::new("a"));
let b = Expr::Node(Node::new("b"));
let c = Expr::Node(Node::new("c"));

// Any single node for reads, all nodes for writes
let qs = QuorumSystem::from_reads(a + b + c);

assert_eq!(qs.read_resilience(), 2);  // Can lose 2 nodes
assert_eq!(qs.write_resilience(), 0); // Cannot lose any
```

## Grid Quorum

```rust
use quoracle::*;

// 3×2 grid: reads need one row, writes need one from each row
let a = Expr::Node(Node::new("a"));
let b = Expr::Node(Node::new("b"));
let c = Expr::Node(Node::new("c"));
let d = Expr::Node(Node::new("d"));
let e = Expr::Node(Node::new("e"));
let f = Expr::Node(Node::new("f"));

let grid = QuorumSystem::from_reads(
    (a.clone() * b.clone() * c.clone()) +
    (d.clone() * e.clone() * f.clone())
);

// Balanced resilience
assert_eq!(grid.resilience(), 2);
```

## Load Optimization

```rust
use quoracle::*;

let qs = /* ... create quorum system ... */;
let dist = Distribution::fixed(0.75)?; // 75% reads
let limits = StrategyLimits::default();

// Minimize load
let strategy = qs.strategy(
    Objective::Load,
    Some(&dist),
    None,
    &limits,
    0
)?;

println!("Optimal load: {:.4}", strategy.load(Some(&dist), None)?);
println!("Capacity: {:.2} ops/s", strategy.capacity(Some(&dist), None)?);
```

## Heterogeneous Nodes

```rust
use quoracle::*;
use std::time::Duration;

// Nodes with different capacities and latencies
let fast = Node::with_capacities("fast", 1000.0, 1000.0)
    .with_latency(Duration::from_millis(10));

let slow = Node::with_capacities("slow", 100.0, 100.0)
    .with_latency(Duration::from_millis(50));

let qs = QuorumSystem::from_reads(
    Expr::Node(fast) + Expr::Node(slow)
);

let dist = Distribution::fixed(0.5)?;
let limits = StrategyLimits::default();

// Optimize for latency
let strategy = qs.strategy(
    Objective::Latency,
    Some(&dist),
    None,
    &limits,
    0
)?;

println!("Expected latency: {:?}", strategy.latency(Some(&dist), None)?);
```

## Asymmetric Read/Write Capacities

```rust
use quoracle::*;

// Nodes optimized for reads (10x read capacity vs write)
let a = Node::with_read_write_capacity("a", 10000.0, 1000.0);
let b = Node::with_read_write_capacity("b", 5000.0, 500.0);
let c = Node::with_read_write_capacity("c", 10000.0, 1000.0);

let qs = QuorumSystem::from_reads(
    Expr::Node(a) * Expr::Node(b) * Expr::Node(c)
);

// Check capacity at different read fractions
let limits = StrategyLimits::default();
for frac in [0.0, 0.5, 1.0] {
    let dist = Distribution::fixed(frac)?;
    let strategy = qs.strategy(Objective::Load, Some(&dist), None, &limits, 0)?;
    let capacity = strategy.capacity(Some(&dist), None)?;
    println!("fr={:.1}: capacity={:.2} ops/s", frac, capacity);
}
```

## Constrained Optimization

```rust
use quoracle::*;
use std::time::Duration;

let qs = /* ... create quorum system ... */;
let dist = Distribution::fixed(0.5)?;

// Minimize latency with load constraint
let limits = StrategyLimits {
    load: Some(1.0 / 1500.0),
    ..Default::default()
};

let strategy = qs.strategy(
    Objective::Latency,
    Some(&dist),
    None,
    &limits,
    0
)?;

println!("Latency: {:?}", strategy.latency(Some(&dist), None)?);
println!("Capacity: {:.2}", strategy.capacity(Some(&dist), None)?);
```

## F-Resilient Strategies

```rust
use quoracle::*;

let qs = /* ... create quorum system ... */;
let dist = Distribution::fixed(0.5)?;
let limits = StrategyLimits::default();

// Find strategy that tolerates 1 node failure
let strategy = qs.strategy(
    Objective::Load,
    Some(&dist),
    None,
    &limits,
    1  // f-resilience
)?;

// Strategy still works if any 1 node fails
let load = strategy.load(Some(&dist), None)?;
println!("Load with f=1: {:.4}", load);
```

## Workload Distributions

```rust
use quoracle::*;

let qs = /* ... create quorum system ... */;

// Weighted distribution: 50% at 10% reads, 50% at 75% reads
let dist = Distribution::weighted(&[
    (0.1, 0.5),
    (0.75, 0.5),
])?;

let limits = StrategyLimits::default();
let strategy = qs.strategy(Objective::Load, Some(&dist), None, &limits, 0)?;

let load = strategy.load(Some(&dist), None)?;
println!("Load across distribution: {:.4}", load);
```

## Heuristic Search

```rust
use quoracle::*;
use quoracle::search::{search, SearchConfig};
use std::time::Duration;

// Search for optimal 5-node configuration
let result = search(
    5,                              // nodes
    Objective::Load,                // minimize load
    Some(&Distribution::fixed(0.5)?), // 50% reads
    None,
    SearchConfig {
        f: 1,                       // tolerate 1 failure
        timeout: Some(Duration::from_secs(30)),
        search_mode: SearchMode::Fast,
    }
)?;

if let Some((qs, strategy)) = result {
    println!("Found optimal system!");
    println!("Load: {:.4}", strategy.load(Some(&dist), None)?);
}
```

## Network Load Optimization

```rust
use quoracle::*;

let qs = /* ... create quorum system ... */;
let dist = Distribution::fixed(0.5)?;
let limits = StrategyLimits::default();

// Minimize expected quorum size
let strategy = qs.strategy(
    Objective::Network,
    Some(&dist),
    None,
    &limits,
    0
)?;

let network_load = strategy.network_load(Some(&dist), None)?;
println!("Expected quorum size: {:.2} nodes", network_load);
```

## Running the Examples

Run the included examples:

```bash
# Simple example with grid quorum
cargo run --example simple

# Comprehensive tutorial
cargo run --example tutorial

# Heuristic search example
cargo run --example search
```
