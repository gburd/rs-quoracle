# Quoracle

[![crates.io](https://img.shields.io/crates/v/quoracle.svg)](https://crates.io/crates/quoracle)
[![docs.rs](https://img.shields.io/docsrs/quoracle)](https://docs.rs/quoracle)
[![CI](https://github.com/gburd/rs-quoracle/actions/workflows/ci.yml/badge.svg)](https://github.com/gburd/rs-quoracle/actions/workflows/ci.yml)

Quoracle is a Rust library for **designing and analyzing read-write quorum
systems**: the rules that decide which replicas a read must contact, which
replicas a write must contact, and how often to use each choice.

Give it a quorum system and a workload (the fraction of operations that are
reads) and it tells you:

- **fault tolerance**: how many nodes can fail while reads and writes still
  work;
- **load and capacity**: how busy the busiest node is, and so how much
  throughput the whole system can sustain;
- **network cost** and **latency**: expected nodes contacted, and expected
  time to assemble a quorum;
- the **optimal strategy**: the probability of choosing each quorum that
  minimizes load, network cost, or latency, optionally subject to limits on
  the others and to `f`-failure resilience;
- the **best quorum system** for a set of nodes, found by search.

It is a Rust port of the Python [Quoracle](https://github.com/mwhittaker/quoracle)
library from *Read-Write Quorum Systems Made Practical* (Whittaker, Charapko,
Hellerstein, Howard, Stoica, PaPoC 2021,
[paper](https://mwhittaker.github.io/publications/quoracle.pdf)). The
algorithms and results match the Python version; its test suite is ported
as parity tests.

## When to use Quoracle

Use it when you are **choosing or tuning a replication scheme** and want
numbers instead of rules of thumb:

- You run Paxos, Raft-style, Dynamo-style (`R + W > N`) or flexible-quorum
  replication and want to know whether majorities are actually the best
  choice for *your* read/write mix. Often they aren't. With 9 nodes, a 3×3
  grid sustains 1.67× the throughput of a majority quorum at 50% reads, and
  read-one/write-all sustains 2.8× at 90% reads.
- Your nodes are **heterogeneous**: some have more capacity or lower
  latency, and you want quorums that use them well.
- You need to **verify a design**: that every read quorum intersects every
  write quorum, and how many failures it tolerates.
- You need a **runtime quorum picker**: compute a strategy once, then
  sample `get_read_quorum()` / `get_write_quorum()` per request.

Quoracle is a planning and analysis library. It does **not** do networking,
failure detection, or replication. Plug its output into your own protocol.
It also doesn't model ordered schemes like chain replication.

**Scale:** quorums are enumerated explicitly. That suits replica groups of
up to about 15 nodes. Measured on one core: the optimal strategy for a
majority of 9 nodes takes about 1 ms, 13 nodes about 70 ms, and 15 nodes
(6,435 quorums) about 1 s. Resilience is closed-form and takes microseconds.
`search` is exhaustive and practical to about 6 nodes; give it a timeout for
more.

## Installation

```toml
[dependencies]
quoracle = "2"
```

The default `microlp` feature is a pure-Rust LP solver, with no system
dependencies. You can use COIN-OR CBC instead if you already depend on it.
It needs the CBC C library, and in our measurements it is no faster at these
problem sizes:

```toml
quoracle = { version = "2", default-features = false, features = ["cbc"] }
```

See [SOLVERS.md](SOLVERS.md). The minimum supported Rust version is 1.88.

## Quick start

```rust
use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem, StrategyLimits};

fn main() -> Result<(), quoracle::Error> {
    // Four replicas in a 2x2 grid: a read contacts a full row,
    // a write contacts one node in every row (the dual).
    let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
    let grid = QuorumSystem::from_reads(a * b + c * d);
    println!("tolerates {} failure(s)", grid.resilience()); // 1

    // Best strategy for a 75%-read workload.
    let fr = Distribution::fixed(0.75)?;
    let strategy = grid.strategy(
        Objective::Load, Some(&fr), None, &StrategyLimits::default(), 0,
    )?;
    println!("load {:.2}", strategy.load(Some(&fr), None)?);         // 0.50
    println!("capacity {:.2}x", strategy.capacity(Some(&fr), None)?); // 2.00x

    // At request time:
    let replicas = strategy.get_read_quorum();
    assert!(grid.is_read_quorum(&replicas));
    Ok(())
}
```

## Integrating Quoracle

**1. Model your nodes.** Node identifiers can be any `Ord + Hash + Display`
type (strings, integers, your own IDs). Add per-node capacity (requests per
second) and latency if your nodes differ:

```rust
use quoracle::Node;
use std::time::Duration;
# fn f() -> Result<(), quoracle::Error> {
let fast = Node::new("us-east-1a")
    .with_read_write_capacity(2000.0, 1000.0)?
    .with_latency(Duration::from_millis(2));
# Ok(()) }
```

**2. Describe the quorum system.** `a + b` means "either", `a * b` means
"both", `choose(k, ...)` / `majority(...)` mean "any k". Build from reads
(writes become the dual, the smallest write rule that is safe), from writes,
or from both with `QuorumSystem::new`, which checks that they intersect.

**3. Describe the workload.** `Distribution::fixed(0.9)` is 90% reads. If
the mix changes over time, use `Distribution::weighted(&[(0.9, 3.0), (0.5,
1.0)])` to optimize for a weighted mix of read fractions.

**4. Optimize.** `strategy(objective, read_fraction, write_fraction,
&limits, f)` minimizes `Load`, `Network`, or `Latency`. Use `StrategyLimits`
to bound the other metrics, and `f > 0` to only use quorums that still work
after `f` of their nodes fail (useful to avoid retries).

**5. Use the result.** Either sample quorums at runtime
(`get_read_quorum`), or read the probabilities (`sigma_r()` / `sigma_w()`)
and feed them into your own router. Per-node metrics (`node_load`,
`node_utilization`, `node_throughput`) help with capacity planning.

**Search:** if you don't know which quorum system to use,
`search(&nodes, &SearchConfig { .. })` tries all of them and returns the best
one with its strategy.

**Errors:** every fallible call returns `quoracle::Error`:
`NoStrategyFound` when limits can't be met, `NonOverlappingQuorums` for
unsafe read/write pairs, and `Invalid*` for bad arguments. Nothing in the
public API panics on bad input.

**Threads:** all types are `Send + Sync`. With the `cbc` feature, solves are
serialized through a global lock, because CBC is not thread-safe.

Quorums are `hashbrown::HashSet`s; the matching version is re-exported as
`quoracle::hashbrown`.

## Examples

```bash
cargo run --example simple    # grid system, resilience, load sweep
cargo run --example tutorial  # the full tour, following the Python tutorial
cargo run --example search    # find the best 4-node system
```

## Documentation

- API reference: <https://docs.rs/quoracle>
- User guide: <https://gburd.github.io/rs-quoracle/>
- [CHANGELOG.md](CHANGELOG.md), including the 1.x → 2.0 migration guide

## Development

Everything is pinned by the Nix flake (`nix develop` gives the toolchain,
CBC, `cargo-llvm-cov`, and `mdbook`; `nix flake check` builds and tests both
solver backends), or use any Rust ≥ 1.88:

```bash
cargo test                                         # microlp backend
cargo test --no-default-features --features cbc    # CBC backend
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly llvm-cov --branch                   # coverage
cargo bench
```

CI runs on both [Codeberg](https://codeberg.org/gregburd/rs-quoracle) (the
primary repository) and [GitHub](https://github.com/gburd/rs-quoracle) (a
mirror). Both test the two solver backends and the MSRV, and enforce ≥ 85%
line, function, and branch coverage.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE-2.0) or
[MIT](LICENSE-MIT) at your option. Based on the MIT-licensed Python
Quoracle by Michael Whittaker; see [NOTICE](NOTICE).
