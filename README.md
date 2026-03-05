# Quoracle

A Rust library for constructing and analyzing read-write quorum systems used in distributed systems research.

## Features

- **Expression algebra** for defining quorum systems using operators (OR, AND, Choose)
- **Linear programming-based optimization** for finding optimal read/write strategies
- **Multi-metric analysis** including load, capacity, network overhead, and latency
- **Resilience calculation** for fault tolerance analysis
- **Heuristic search** for discovering optimal quorum configurations

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
quoracle = "0.1"
```

## Quick Start

```rust
use quoracle::*;

// Define nodes
let a = Node::new('a');
let b = Node::new('b');
let c = Node::new('c');

// Create a simple majority quorum system
let qs = QuorumSystem::new_reads(a + b + c)?;

// Find optimal strategy
let strategy = qs.strategy(
    OptimizeTarget::Load,
    None, // load_limit
    None, // network_limit
    None, // latency_limit
    Some(&Distribution::Fixed(0.5)), // read_fraction
    None, // write_fraction
    1,    // f-resilience
)?;
```

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Benchmarking

```bash
cargo bench
```

## License

MIT OR Apache-2.0
