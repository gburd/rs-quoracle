# API Documentation

Complete API reference is available on [docs.rs/quoracle](https://docs.rs/quoracle).

## Core Types

### QuorumSystem

The main type for creating and analyzing quorum systems.

```rust
pub struct QuorumSystem<T: Element>
```

**Key Methods:**
- `from_reads(expr: Expr<T>) -> Self` - Create from read expression
- `from_writes(expr: Expr<T>) -> Self` - Create from write expression
- `new(reads: Expr<T>, writes: Expr<T>) -> Result<Self>` - Create from both
- `strategy(...)` - Compute optimal strategy
- `resilience() -> i64` - Overall fault tolerance
- `read_quorums() -> impl Iterator` - Enumerate read quorums
- `write_quorums() -> impl Iterator` - Enumerate write quorums

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/struct.QuorumSystem.html)

### Expr

Expression algebra for defining quorum systems.

```rust
pub enum Expr<T: Element> {
    Node(Node<T>),
    And(Box<And<T>>),
    Or(Box<Or<T>>),
    Choose(Box<Choose<T>>),
}
```

**Operators:**
- `+` (Add) - OR combinator
- `*` (Mul) - AND combinator
- `choose(k, nodes)` - Choose k from n

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/enum.Expr.html)

### Strategy

A read/write strategy with quorum selection probabilities.

```rust
pub struct Strategy<T: Element>
```

**Key Methods:**
- `load(read_fraction, write_fraction) -> Result<f64>` - Calculate load
- `capacity(read_fraction, write_fraction) -> Result<f64>` - Calculate capacity
- `network_load(...) -> Result<f64>` - Expected quorum size
- `latency(...) -> Result<Duration>` - Expected latency
- `node_load(node, ...) -> Result<f64>` - Per-node load

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/struct.Strategy.html)

### StrategyLimits

Optional constraints for strategy optimization.

```rust
pub struct StrategyLimits {
    pub load: Option<f64>,
    pub network: Option<f64>,
    pub latency: Option<Duration>,
}
```

**Usage:**
```rust
let limits = StrategyLimits {
    load: Some(0.5),
    network: Some(3.0),
    latency: Some(Duration::from_secs(5)),
};
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/struct.StrategyLimits.html)

### Node

Represents a node with capacity and latency properties.

```rust
pub struct Node<T: Element>
```

**Constructors:**
- `new(x: T) -> Self` - Default capacity (1.0) and latency (0s)
- `with_capacity(x: T, capacity: f64) -> Self` - Same read/write capacity
- `with_read_write_capacity(x: T, read_cap: f64, write_cap: f64) -> Self`
- `with_latency(self, latency: Duration) -> Self` - Chainable

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/struct.Node.html)

### Distribution

Workload distribution over read fractions.

```rust
pub enum Distribution {
    Fixed(f64),
    Weighted(Vec<(f64, f64)>),
}
```

**Constructors:**
- `fixed(fraction: f64) -> Result<Self>` - Single read fraction
- `weighted(fractions: &[(f64, f64)]) -> Result<Self>` - Multiple fractions with weights

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/enum.Distribution.html)

### Objective

Optimization objective for strategy computation.

```rust
pub enum Objective {
    Load,     // Minimize maximum node load
    Network,  // Minimize expected quorum size
    Latency,  // Minimize expected latency
}
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/enum.Objective.html)

## Search Module

Heuristic search for optimal quorum systems.

```rust
pub mod search
```

**Key Function:**
```rust
pub fn search<T: Element>(
    num_nodes: usize,
    objective: Objective,
    read_fraction: Option<&Distribution>,
    write_fraction: Option<&Distribution>,
    config: SearchConfig,
) -> Result<Option<(QuorumSystem<T>, Strategy<T>)>>
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/search/)

## Error Handling

All fallible operations return `Result<T, Error>`.

```rust
pub enum Error {
    InvalidExpression(String),
    InvalidQuorumSystem(String),
    InvalidDistribution(String),
    NoStrategyFound,
    LpError(String),
}
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/enum.Error.html)

## Traits

### Element

Trait for types that can be used as node identifiers.

```rust
pub trait Element:
    Clone + Ord + std::hash::Hash + std::fmt::Debug
{
}
```

Automatically implemented for types satisfying the bounds.

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/trait.Element.html)

## Helper Functions

### majority

Create a majority quorum expression.

```rust
pub fn majority<T: Element>(
    nodes: Vec<Expr<T>>
) -> Result<Expr<T>>
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/fn.majority.html)

### choose

Create a choose-k expression.

```rust
pub fn choose<T: Element>(
    k: usize,
    nodes: Vec<Expr<T>>
) -> Result<Expr<T>>
```

[Full documentation →](https://docs.rs/quoracle/latest/quoracle/fn.choose.html)

## Feature Flags

### Default Features

- `microlp` - Pure Rust LP solver (default)

### Optional Features

- `cbc` - Use CBC solver for better performance (requires system CBC library)

**Note:** Only enable one solver feature at a time.

```toml
# Use CBC solver
[dependencies]
quoracle = { version = "1.2", default-features = false, features = ["cbc"] }
```

## Examples in Documentation

All API items include usage examples. View them on docs.rs:

```bash
cargo doc --open
```

Or visit: [https://docs.rs/quoracle](https://docs.rs/quoracle)
