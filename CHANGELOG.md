# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[1.1.0]: https://github.com/gregburd/quoracle/releases/tag/v1.1.0
[1.0.0]: https://github.com/gregburd/quoracle/releases/tag/v1.0.0
