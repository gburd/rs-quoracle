# Multi-Solver Implementation Status

## Goal
Allow users to choose between three LP solver backends: CBC, Clarabel (default), and Microlp.

## Progress: 40% Complete

### ✅ Completed

1. **Cargo.toml** - Feature flags configured
   - Added `cbc`, `clarabel`, `microlp` features
   - Default is `clarabel`
   - `all-solvers` convenience feature
   - Separate package imports for each solver

2. **solver.rs** - New module created
   - `Solver` enum with feature-gated variants
   - Comprehensive documentation comparing solvers
   - Helper methods: `is_pure_rust()`, `is_verbose()`, `available()`, etc.
   - Default implementation (prefers Clarabel > CBC > Microlp)
   - Basic tests

3. **lp.rs** - Partial update
   - Created `solve_lp!` macro for solver dispatch
   - Updated `min_hitting_set()` to accept `Solver` parameter
   - Updated all tests to pass `Solver::default()`
   - Imports reorganized for feature-gated solvers

4. **expr.rs** - Partial update
   - Updated `min_hitting_set()` helper to accept `Solver`
   - Added `resilience_with_solver()` method
   - Existing `resilience()` delegates to default solver

### 🚧 In Progress

1. **quorum_system.rs** - NOT STARTED
   - Need to add `*_with_solver()` variants for all strategy methods
   - Methods that need updating:
     - `uniform_strategy()` → `uniform_strategy_with_solver()`
     - `strategy()` → `strategy_with_solver()`
     - Internal `lp_optimal_strategy()` needs solver parameter
   - Update all LP solving code to use solver dispatch macro

2. **search.rs** - NOT STARTED
   - Update `SearchConfig` to include `solver: Solver` field
   - Update `search()` function to use config.solver
   - Update tests and examples

3. **Examples** - NOT STARTED
   - Update `simple.rs`, `tutorial.rs`, `search.rs` examples
   - Add examples demonstrating solver selection
   - Show performance/output differences

4. **Tests** - PARTIALLY COMPLETE
   - lp.rs tests updated
   - Need integration tests for all three solvers
   - Need to test each solver independently
   - Add `#[cfg(all_solvers)]` tests that compare results

### ❌ Not Started

1. **Comprehensive Testing**
   - Test matrix: all tests × all solvers
   - Verify identical results across solvers
   - Performance comparison benchmarks
   - Test with single solver features enabled

2. **Documentation Updates**
   - Update README.md with solver selection guide
   - Update CHANGELOG.md
   - Add SOLVERS.md with detailed comparison
   - Update rustdoc examples
   - Update lib.rs doc comments

3. **Benchmarks**
   - Update benchmarks to accept solver parameter
   - Add comparative benchmarks (CBC vs Clarabel vs Microlp)
   - Document performance differences

4. **CI/CD**
   - Test all three solver feature combinations
   - Test `all-solvers` feature
   - Test default (clarabel only)

## Next Steps (Priority Order)

1. **Update quorum_system.rs** (CRITICAL)
   - This is the main API surface
   - Add `*_with_solver()` methods
   - Update internal LP solving code

2. **Fix compilation errors**
   - The current code won't compile due to incomplete refactoring
   - Need to update all callsites of changed methods

3. **Update search.rs**
   - Add solver to SearchConfig
   - Update search function

4. **Update examples**
   - Make examples work with new API
   - Add solver selection examples

5. **Add comprehensive tests**
   - Test all solvers produce same results
   - Test feature flag combinations

6. **Update documentation**
   - README, CHANGELOG, inline docs

7. **Performance benchmarks**
   - Compare solver performance
   - Document recommendations

## Design Decisions

### Why this approach?

**Option 1 (Chosen)**: Add `*_with_solver()` methods, keep existing methods with default
- ✅ Backward compatible
- ✅ Explicit when needed
- ✅ Default solver "just works"
- ⚠️ More methods to maintain

**Option 2 (Rejected)**: Make solver a struct field
- ❌ Breaking change
- ❌ Awkward ergonomics (need to set everywhere)
- ✅ More explicit

**Option 3 (Rejected)**: Global solver configuration
- ❌ Not thread-safe without locks
- ❌ Surprising action-at-a-distance
- ❌ Hard to test

### Solver Dispatch Strategy

We use a macro because:
- good_lp uses different types for each solver (coin_cbc, clarabel, microlp)
- Can't use trait objects (solvers aren't Sized/object-safe)
- Macro provides compile-time dispatch based on runtime Solver enum
- Each feature-gated solver is a separate type

## Testing Strategy

```bash
# Test with default (clarabel)
cargo test

# Test each solver individually
cargo test --no-default-features --features cbc
cargo test --no-default-features --features clarabel
cargo test --no-default-features --features microlp

# Test all solvers
cargo test --features all-solvers

# Benchmarks
cargo bench --features all-solvers
```

## Known Issues

1. **Cargo cache permissions** - Environmental, not code issue
2. **Incomplete refactoring** - Code won't compile until quorum_system.rs is updated
3. **Test coverage** - Need cross-solver validation tests

## Success Criteria

- [ ] All tests pass with each solver individually
- [ ] All tests pass with all-solvers feature
- [ ] Examples demonstrate solver selection
- [ ] Documentation explains trade-offs
- [ ] Benchmarks quantify performance differences
- [ ] CI tests all feature combinations
- [ ] Backward compatibility maintained (existing code still works)
