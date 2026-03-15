# Quoracle

A Rust library for constructing and analyzing read-write quorum systems used in distributed systems research.

## What are Quorum Systems?

Quorum systems are fundamental to distributed storage and consensus protocols. They specify which subsets of replicas (quorums) must agree on read/write operations to ensure consistency.

**Example:** In a 5-node system with majority quorums, any 3 nodes form a quorum. Reads from any 3 nodes and writes to any 3 nodes guarantee that read and write quorums intersect, ensuring consistency.

## Features

- **Expression algebra** for defining quorum systems (OR, AND, Choose)
- **LP-based optimization** for optimal read/write strategies
- **Multi-metric analysis**: load, capacity, network overhead, latency
- **Resilience calculation** for fault tolerance analysis
- **Heuristic search** for discovering optimal configurations

## When to Use Quoracle

- **Designing distributed databases** - optimize replica strategies
- **Research** - analyze quorum system properties and trade-offs
- **System analysis** - understand fault tolerance and performance
- **Protocol design** - find optimal quorum configurations

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
quoracle = "1.2"
```

## Performance

Rust implementation is **2-10× faster** than Python reference:

- Quorum enumeration: 3-10× faster
- Load calculations: 2-10× faster
- Heuristic search: 3-10× faster
- LP optimization: ~1.5× faster

## Links

- [GitHub Repository](https://github.com/gregburd/quoracle)
- [Crates.io](https://crates.io/crates/quoracle)
- [API Documentation](https://docs.rs/quoracle)

## License

Licensed under MIT OR Apache-2.0
