# Quick Start

This guide will get you up and running with Quoracle in minutes.

## Basic Example

```rust
use quoracle::*;

// Define nodes
let a = Expr::Node(Node::new("a"));
let b = Expr::Node(Node::new("b"));
let c = Expr::Node(Node::new("c"));

// Create majority quorum system
// Reads need any single node, writes need all nodes
let qs = QuorumSystem::from_reads(a + b + c);

// Check resilience
assert_eq!(qs.read_resilience(), 2);  // Can lose 2 nodes for reads
assert_eq!(qs.write_resilience(), 0); // Cannot lose any for writes

// Find optimal strategy for 50% reads
let dist = Distribution::fixed(0.5)?;
let limits = StrategyLimits::default();
let strategy = qs.strategy(
    Objective::Load,
    Some(&dist),
    None,
    &limits,
    0
)?;

// Calculate load
let load = strategy.load(Some(&dist), None)?;
println!("Load: {:.4}", load);
println!("Capacity: {:.2} ops/s", 1.0 / load);
```

## Key Concepts

### Expressions

Build quorum systems using:
- `+` (OR) - any quorum from either expression
- `*` (AND) - combine elements from both
- `choose(k, nodes)` - choose k nodes from list

```rust
// Grid quorum: reads need one row, writes need one from each row
let a = Expr::Node(Node::new("a"));
let b = Expr::Node(Node::new("b"));
let c = Expr::Node(Node::new("c"));
let d = Expr::Node(Node::new("d"));

let grid = QuorumSystem::from_reads(
    (a.clone() * b.clone()) + (c.clone() * d.clone())
);
```

### Strategies

A strategy assigns probabilities to quorums. Optimize for:
- `Objective::Load` - minimize maximum node load
- `Objective::Network` - minimize expected quorum size
- `Objective::Latency` - minimize expected quorum latency

```rust
// Optimize for latency
let strategy = qs.strategy(
    Objective::Latency,
    Some(&dist),
    None,
    &limits,
    0
)?;

let latency = strategy.latency(Some(&dist), None)?;
println!("Expected latency: {:?}", latency);
```

### Constraints

Add optional constraints using `StrategyLimits`:

```rust
use std::time::Duration;

let limits = StrategyLimits {
    load: Some(0.5),                // Max load ≤ 0.5
    network: Some(3.0),             // Max network ≤ 3 nodes
    latency: Some(Duration::from_secs(5)), // Max latency ≤ 5s
};

let strategy = qs.strategy(
    Objective::Network,
    Some(&dist),
    None,
    &limits,
    0
)?;
```

### F-Resilience

Find strategies that tolerate `f` node failures:

```rust
// Strategy that works even if 1 node fails
let strategy = qs.strategy(
    Objective::Load,
    Some(&dist),
    None,
    &limits,
    1  // f = 1
)?;
```

## Next Steps

- See [Examples](./examples.md) for more detailed usage
- Check [Performance](./performance.md) for benchmarks
- Read the [API Documentation](https://docs.rs/quoracle) for complete reference
