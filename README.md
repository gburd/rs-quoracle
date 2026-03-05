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
quoracle = "1.2"
```

The default uses the **Clarabel** solver (pure Rust, silent output). For alternatives, see [SOLVERS.md](SOLVERS.md).

## Quick Start

```rust
use quoracle::*;

// Define node expressions
let a = Expr::Node(Node::new("a"));
let b = Expr::Node(Node::new("b"));
let c = Expr::Node(Node::new("c"));

// Create a simple majority quorum system
let qs = QuorumSystem::from_reads(a + b + c);

// Find optimal strategy
let dist = Distribution::Fixed(0.5);
let strategy = qs.strategy(
    Objective::Load,      // Minimize load
    Some(&dist),          // Read fraction
    None,                 // Write fraction (inferred)
    None,                 // No load limit
    None,                 // No network limit
    None,                 // No latency limit
    0,                    // f-resilience
)?;

// Calculate load
let load = strategy.load(Some(&dist), None)?;
println!("Load: {:.4}", load);
```

## Development

### Building

```bash
# Build the library
cargo build --lib

# Check code without building
cargo check

# Run clippy linter
cargo clippy --all-targets --all-features

# Generate documentation
cargo doc --no-deps --open
```

### Testing

```bash
# Run all tests (155 total)
cargo test

# Results:
# - 118 unit tests: ✅ all pass
# - 36 integration tests: ✅ all pass
# - 1 doc test: ✅ pass
```

### Examples

#### Simple Example - Basic quorum system usage

```bash
cargo run --example simple
```

Output shows grid quorum system with resilience calculation and load optimization.

#### Tutorial Example - Comprehensive walkthrough

```bash
cargo run --example tutorial
```

Covers all major features including heterogeneous nodes and latency optimization.

#### Search Example - Heuristic search for optimal systems

```bash
cargo run --example search
```

Demonstrates automated search for optimal 4-node configurations.

### Benchmarking

```bash
# Run benchmarks
cargo bench --bench benchmarks

# Compare with Python implementation
cd _
pip install -r requirements.txt
python benchmarks.py
```

See [BENCHMARKS.md](BENCHMARKS.md) for benchmarking methodology and [COMPARISON.md](COMPARISON.md) for detailed Rust vs Python performance analysis.

## Performance

The Rust implementation provides significant performance improvements over the Python version:

- **3-10× faster** for quorum enumeration and iteration
- **2-10× faster** for load calculations
- **3-10× faster** for heuristic search
- **~1.5× faster** for LP optimization (both use CBC solver)

See [PERFORMANCE.md](PERFORMANCE.md) for measured results and [COMPARISON.md](COMPARISON.md) for detailed Rust vs Python analysis.

## Documentation

Full API documentation is available:

```bash
cargo doc --no-deps --open
```

Or view online at [docs.rs/quoracle](https://docs.rs/quoracle) (when published).

## Project Structure

```
quoracle/
├── src/
│   ├── lib.rs              # Public API exports
│   ├── expr.rs             # Expression algebra (Node, Or, And, Choose)
│   ├── quorum_system.rs    # QuorumSystem and Strategy
│   ├── distribution.rs     # Workload distribution types
│   ├── geometry.rs         # Point/Segment for piecewise functions
│   ├── search.rs           # Heuristic search
│   ├── lp.rs              # LP solver abstraction
│   └── error.rs           # Error types
├── tests/                  # Integration tests
├── benches/               # Performance benchmarks
├── examples/              # Usage examples
└── _/                     # Python reference implementation
```

## Implementation Notes

- **4,103 lines** of production code
- **155 tests** (all passing)
- **Zero clippy warnings** with strict lints
- Uses **CBC solver** via good_lp for LP optimization
- Type-safe generics with `Element` trait
- BTreeMap-based quorum storage for hashability

## Comparison with Python Version

| Aspect | Rust | Python |
|--------|------|--------|
| Performance | 2-10× faster | Baseline |
| Type Safety | Compile-time | Runtime |
| Memory Usage | Lower | Higher (GC) |
| Ecosystem | Cargo, crates.io | pip, PyPI |
| Use Case | Production systems | Prototyping, research |

Both implementations:
- Use identical algorithms
- Use CBC solver backend
- Are production-ready
- Have comprehensive tests

## Future Work

While Quoracle focuses on read-write quorum systems, related research areas exist:

### Chain Replication (Out of Scope)

Chain replication (e.g., Machi, Hibari) provides linearizable distributed storage through ordered replica chains. While powerful, chain replication is fundamentally incompatible with quorum-based approaches:

- **Chains**: Ordered replicas (head/tail roles), sequential updates, single-node reads
- **Quorums**: Unordered replicas, parallel operations, set-based reads/writes

Modeling chains as degenerate quorums would lose essential chain semantics (ordering, linearizability guarantees). Each approach is optimal for different use cases and should be analyzed with purpose-built tools.

### Potential Extensions

Potential future work within the quorum system domain:
- Custom optimization metrics beyond load/capacity/latency
- Network topology awareness (per-link costs)
- Dynamic reconfiguration support
- Extended heterogeneous node modeling

## Contributing

Contributions welcome! Please ensure:
- All tests pass: `cargo test`
- No clippy warnings: `cargo clippy --all-targets --all-features`
- Code is formatted: `cargo fmt`

## License

MIT OR Apache-2.0
