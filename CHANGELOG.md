# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `QuorumSystem::strategy_with_failures` optimizes routing around known
  failed nodes while retaining the original quorum rules and tolerance
  for `f` further failures.

## [2.0.0] - 2026-10-06

2.0 fixes correctness bugs and makes invalid states unrepresentable, which
needs some breaking API changes. Most code needs only small edits. See
"Migrating from 1.x" below.

### Fixed
- **CBC backend returned wrong answers under concurrency.** `good_lp`
  disables `coin_cbc`'s global lock and CBC is not thread-safe, so parallel
  solves (including `cargo test`) returned `0` for resilience. The `cbc`
  feature now re-enables the lock.
- **Breaking-change hygiene:** 1.4.0 changed the set type in the public API
  from `std::collections::HashSet` to `hashbrown::HashSet` without saying
  so. 2.0 re-exports the exact version as `quoracle::hashbrown`.
- `search` honors its timeout. It used to build every candidate expression
  before checking the clock (8 nodes: 1.3 GB and a timeout overrun);
  candidates are now generated lazily.
- `search(&[])` returned an error instead of panicking.
- Infeasible strategy limits now return `Error::NoStrategyFound` (they
  returned `Error::LpError`), and `QuorumSystem::new` returns
  `Error::NonOverlappingQuorums` (this variant was never used).
- `make_strategy` merges keys naming the same quorum (`["b","a"]`,
  `["a","a","b"]`), so load is not double-counted. Empty, all-zero, NaN, and
  infinite weights are rejected instead of producing NaN.
- `Strategy::node_load` / `node_utilization` use the quorum system's node
  capacities. Passing `Node::new(id)` silently used capacity 1.0.
- LP solutions are re-normalized, so strategy probabilities sum to exactly 1.
- `Expr::resilience` no longer needs an LP solver (exact branch-and-bound
  over minimal quorums) and cannot fail. It used to return `-1` when the
  solver failed.
- `OrderedFloat` treats `0.0` and `-0.0` as equal, consistent with `Hash`.
- `Distribution` rejects NaN and infinite values and merges repeated
  fractions. Directly-built `Distribution::Weighted` values are validated
  when used.
- `Node` rejects zero, negative, NaN, and infinite capacities.
- `rust-version` was wrong (`1.70`); the crate needs and is tested on 1.88.

### Changed (breaking)
- `Or`, `And`, `Choose`, `Node`, `QuorumSystem`, and `Strategy` fields are
  private; use the accessors (`children()`, `k()`, `x()`,
  `read_capacity()`, `reads()`, `sigma_r()`, …). This prevents invalid
  expressions such as `Choose { k: 5, children: vec![] }`.
- `Node::with_capacity` and `with_read_write_capacity` return `Result`.
- `resilience()` returns `usize` (was `i64`).
- `SearchConfig` takes `limits: StrategyLimits` instead of separate
  `load_limit` / `network_limit` / `latency_limit`, and `resilience: usize`.
- `Error` is `#[non_exhaustive]` and implements `Eq`.
- Removed the `lp` module (`lp::min_hitting_set`, a duplicate
  `lp::Objective`, and the unused `solve_strategy_lp`).
- When both solver features are enabled, CBC is used (this is `good_lp`'s
  rule; the docs said Microlp).

### Added
- `quoracle::hashbrown` re-export and `quoracle::Quorum` type alias.
- `Strategy::quorum_system()`, `From<Or|And|Choose>` for `Expr`.
- Python parity tests ported from the reference implementation's
  `test_quorum_system.py`.
- README and user-guide code blocks are compiled and run as doctests.
- CI on Codeberg (Forgejo Actions) and GitHub Actions: both solver
  backends, MSRV, `nix flake check`, and coverage gates (≥ 85% lines,
  functions, and branches per configuration). Docs deploy to GitHub Pages.
- User guide at <https://gburd.github.io/rs-quoracle/>.

### Dependencies
- Rust 1.88+; `good_lp` 1.15, `microlp` 0.6, `itertools` 0.15, `rand`
  0.10, `hashbrown` 0.17, `criterion` 0.8 (dev).
- Nix flake repaired (`nix build` and `nix flake check` failed) and
  updated to the latest nixpkgs and Rust 1.99.

### Migrating from 1.x

| 1.x | 2.0 |
|---|---|
| `node.x`, `node.read_capacity` | `node.x()`, `node.read_capacity()` |
| `Node::new(x).with_capacity(c)` | `Node::new(x).with_capacity(c)?` |
| `or.children`, `choose.k` | `or.children()`, `choose.k()` |
| `qs.reads`, `strategy.sigma_r` | `qs.reads()`, `strategy.sigma_r()` |
| `let r: i64 = qs.resilience()` | `let r: usize = qs.resilience()` |
| `SearchConfig { load_limit: Some(x), .. }` | `SearchConfig { limits: StrategyLimits { load: Some(x), ..Default::default() }, .. }` |
| `use hashbrown::HashSet` (to call `is_quorum`) | `use quoracle::hashbrown::HashSet` |
| `quoracle::lp::min_hitting_set` | `Expr::resilience` (hitting set − 1) |
| `match err { … }` exhaustively | add a `_ =>` arm |

## [1.4.0] - 2026-05-30

### Changed
- Switched internal `HashMap`/`HashSet` usage from `std` to `hashbrown` for
  faster lookups in quorum enumeration and strategy construction.
- Build script now silently accepts `--all-features` (both solvers enabled,
  e.g. in CI) and only errors when no solver feature is enabled.

### Fixed
- Fixed a `SolverModel` dyn-incompatibility in the LP helpers by making them
  generic over the solver model type.
- Corrected the `repository` URL to the canonical Codeberg location and added
  `homepage`/`documentation` metadata for crates.io.
- Updated the README Quick Start to the current `StrategyLimits` /
  `Distribution::fixed` API.

### Added
- `rustfmt.toml` pinning the project's 80-column style so `cargo fmt --check`
  is reproducible in CI.
- Forgejo Actions CI workflow for Codeberg (`.forgejo/workflows/ci.yml`)
  alongside the existing GitHub Actions workflow.
- Re-exported `SearchConfig` and `SearchResult` from the crate root.

### Removed
- Deleted the stale, unused `solver.rs` module (it documented a Clarabel
  default that does not exist; the active solver abstraction lives in `lp.rs`).

## [1.3.0] - 2026-03-15

### Changed
- **Breaking:** `QuorumSystem::strategy` now takes a single `StrategyLimits`
  struct instead of three positional `Option` limit arguments, and `f` is the
  final argument.
- `Distribution::fixed` and `Distribution::weighted` now validate their inputs
  and return `Result`.

### Added
- mdBook documentation under `docs/`.
- Nix flake (`flake.nix`) for reproducible development environments.
- `deny.toml` for `cargo-deny` license/advisory checks.
- Standalone `LICENSE-MIT` and `LICENSE-APACHE-2.0` files.

## [1.2.1] - 2026-03-05

### Added
- Feature flags for solver selection (`microlp`, `cbc`)
- Enables easier testing across different solvers: `cargo test --features cbc`
- Build-time check preventing both solver features from being enabled simultaneously

### Fixed
- Prevent solver conflicts when multiple features are enabled (e.g., `--all-features`)
- Tests now fail cleanly at build time rather than hanging or producing wrong results

### Changed
- Refactored Cargo.toml to use feature flags for solver selection
- Added build.rs to enforce mutually exclusive solver features
- No API changes, fully backward compatible

## [1.2.0] - 2026-03-05

### Changed
- **Default LP solver changed from CBC to Microlp** (Breaking: users requiring CBC must explicitly specify it)
  - Microlp is pure Rust (no C dependencies)
  - Silent by default (no verbose solver output)
  - Supports both continuous and binary variables (required for resilience calculation)
  - Slower than CBC but acceptable for most use cases
  - Drop-in replacement with identical results

### Added
- SOLVERS.md documenting LP solver selection and performance comparison
- Inline documentation for solver alternatives in Cargo.toml
- Documentation explaining why Clarabel cannot be used (no binary variable support)

### Migration from 1.1.0

To keep using CBC (for maximum performance):
```toml
quoracle = { version = "1.2", default-features = false }
good_lp = { version = "1.8", features = ["coin_cbc"] }
```

Otherwise, Microlp works as a drop-in replacement with no code changes needed.

## [1.1.0] - 2026-03-05

### Added
- Comprehensive benchmark suite using Criterion.rs with 11 benchmarks
- BENCHMARKS.md documenting benchmarking methodology
- PERFORMANCE.md with measured performance results
- COMPARISON.md with detailed Rust vs Python performance analysis
- Performance measurements showing:
  - Quorum enumeration: 10-20 µs
  - Resilience calculation: 3-5 µs
  - Strategy optimization: 1-1.5 ms
  - Load calculation: 425 ns
  - Heuristic search: 31.7 ms

### Changed
- Updated README.md with comprehensive documentation
- Added performance comparison guidance for implementation selection

## [1.0.0] - 2026-03-05

### Added
- Complete Rust port of Quoracle library
- Expression algebra with operator overloading (Or, And, Choose)
- Linear programming optimization using CBC solver (good_lp)
- Multi-metric analysis (load, capacity, network, latency)
- Resilience calculation via minimum hitting set
- Heuristic search for optimal quorum configurations
- Distribution types for workload modeling (Fixed and Weighted)
- 155 comprehensive tests (118 unit + 36 integration + 1 doc)
- Three working examples (simple, tutorial, search)
- GitHub Actions CI/CD workflow with format, clippy, and test checks
- Complete rustdoc API documentation
- Strict clippy lints and zero unsafe code

### Implementation Details
- 4,103 lines of production code across 8 modules
- Type-safe generics with Element trait
- BTreeMap-based quorum storage for hashability
- Support for heterogeneous nodes (capacity, latency)
- F-resilient quorum enumeration

[2.0.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v2.0.0
[1.4.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.4.0
[1.3.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.3.0
[1.2.1]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.2.1
[1.2.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.2.0
[1.1.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.1.0
[1.0.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.0.0
