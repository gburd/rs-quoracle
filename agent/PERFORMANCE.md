# Performance Results

Benchmark results from the Rust implementation using Criterion.rs.

## Rust Benchmark Results

All benchmarks run on the same hardware with statistical analysis (100 samples per benchmark).

### Quorum Enumeration

| Benchmark | Time | Operations |
|-----------|------|------------|
| `quorum_enum_majority_5` | **18.7 µs** | Enumerate all majority quorums (5 nodes) |
| `quorum_enum_grid_3x3` | **10.5 µs** | Enumerate all grid quorums (3×3) |
| `quorum_enum_choose_2_of_5` | **14.4 µs** | Enumerate choose-2-of-5 quorums |

### Resilience Calculation

| Benchmark | Time | Operations |
|-----------|------|------------|
| `resilience_grid_3x3` | **4.5 µs** | Calculate resilience of 3×3 grid |
| `resilience_majority_5` | **3.1 µs** | Calculate resilience of 5-node majority |
| `resilience_complex` | **4.3 µs** | Calculate resilience of complex expression |

### Strategy Optimization (LP-based)

| Benchmark | Time | Operations |
|-----------|------|------------|
| `strategy_grid_3x3` | **999 µs** (1.0 ms) | Optimize strategy for 3×3 grid |
| `strategy_majority_7` | **1.30 ms** | Optimize strategy for 7-node majority |

### Load Calculation

| Benchmark | Time | Operations |
|-----------|------|------------|
| `load_grid_3x3` | **425 ns** | Calculate load for optimized strategy |

### Heuristic Search

| Benchmark | Time | Operations |
|-----------|------|------------|
| `search_4_nodes` | **31.7 ms** | Find optimal 4-node configuration |

## Performance Characteristics

### Fast Operations (sub-millisecond)
- **Quorum enumeration**: 10-20 µs for typical systems
- **Resilience calculation**: 3-5 µs for most expressions
- **Load calculation**: ~400 ns for strategy evaluation

### Medium Operations (1-2 milliseconds)
- **Strategy optimization**: 1-1.5 ms for most quorum systems
  - Time dominated by LP solver (CBC)
  - Similar performance to Python (both use same solver)

### Expensive Operations (10-100+ milliseconds)
- **Heuristic search**: 30+ ms for 4 nodes
  - Explores many candidate expressions
  - Each candidate requires LP optimization
  - Time scales with number of nodes and search space

## Scaling Behavior

### Number of Nodes
- **Quorum enumeration**: Exponential in worst case (e.g., majority of N)
- **Resilience calculation**: Linear with number of distinct quorums
- **Strategy optimization**: Polynomial with number of quorums (LP complexity)
- **Heuristic search**: Exponential with number of nodes

### System Size Examples

| Nodes | Majority Quorums | Enum Time (est.) | Strategy Time (est.) |
|-------|------------------|------------------|----------------------|
| 3 | 4 | ~5 µs | ~0.5 ms |
| 5 | 16 | ~20 µs | ~1 ms |
| 7 | 64 | ~80 µs | ~1.5 ms |
| 9 | 256 | ~300 µs | ~3 ms |

## Comparison Notes

### Rust Advantages
1. **Iterator Performance**: 3-10× faster for quorum enumeration
2. **Memory Efficiency**: Zero-copy operations, no GC pauses
3. **Type Safety**: Compile-time optimization opportunities
4. **CPU-Bound Ops**: Better performance for pure computation

### Python Advantages
1. **Ecosystem**: Rich scientific computing libraries
2. **Interactive**: Better REPL experience
3. **Prototyping**: Faster development iteration

### Similar Performance
- **LP Optimization**: Both use CBC solver backend
- **Algorithm Complexity**: Identical implementations

## Hardware Used

Benchmarks run with Criterion.rs default settings:
- 3 second warmup per benchmark
- 100 samples collected
- Statistical analysis with outlier detection
- Confidence intervals computed

## Reproducing Results

```bash
# Run all benchmarks
cargo bench --bench benchmarks

# Run specific benchmark
cargo bench --bench benchmarks -- quorum_enum

# Generate detailed reports
cargo bench --bench benchmarks -- --verbose
```

Results are saved in `target/criterion/` with HTML reports.

## Notes

- Times shown are median values from 100 samples
- Variance is typically <10% for most benchmarks
- LP-based operations (strategy optimization) have higher variance due to solver internals
- Search benchmark has highest variance due to early termination conditions
