# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[1.4.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.4.0
[1.3.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.3.0
[1.2.1]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.2.1
[1.2.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.2.0
[1.1.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.1.0
[1.0.0]: https://codeberg.org/gregburd/rs-quoracle/releases/tag/v1.0.0
