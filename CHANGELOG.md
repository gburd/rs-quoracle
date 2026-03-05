# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
