# LP Solver Selection

Quoracle uses linear programming to optimize quorum strategies. The library requires a solver that supports both continuous variables (for strategy optimization) and binary/integer variables (for resilience calculation). You can choose between two solver backends at build time.

## Default: Microlp (Recommended)

**Microlp** is the default solver (as of v1.2.0).

### Why Microlp?

- ✅ **Pure Rust** - No C/C++ dependencies, easy deployment, WASM support
- ✅ **Silent by default** - No verbose solver output cluttering your logs
- ✅ **Supports binary variables** - Required for resilience calculation
- ✅ **Lightweight** - Minimal dependencies
- ⚠️ **Slower than CBC** - Acceptable for most use cases, but noticeably slower for large problems

### Installation

```toml
[dependencies]
quoracle = "1.4"  # Uses Microlp by default
```

## Alternative: CBC (Maximum Performance)

If you need absolute maximum performance and don't mind verbose output:

```toml
[dependencies]
# Replace Microlp with CBC solver
quoracle = { version = "1.4", default-features = false, features = ["cbc"] }
```

### CBC Trade-offs

- ✅ **Excellent performance** - Industry-standard solver (COIN-OR)
- ✅ **Battle-tested** - Widely used in research and production
- ✅ **Supports binary variables** - Required for resilience calculation
- ⚠️ **Verbose output** - Prints optimization details to stdout/stderr (cannot be easily suppressed)
- ⚠️ **C library dependency** - Requires CBC library headers at build time
- ⚠️ **EPL 2.0 license** - Copyleft license (less permissive than Apache/MIT)

**Suppressing CBC output**:
```bash
# Redirect stderr when running your program
your_program 2>/dev/null
```

## Why Not Clarabel?

**Clarabel cannot be used with Quoracle** because it only supports continuous variables. Quoracle requires binary (integer) variables for resilience calculation via minimum hitting set. Attempting to use Clarabel will cause test failures with the error "Clarabel doesn't support integer variables".

## Performance Comparison

Based on benchmarks (4-node search with timeout):

| Solver   | Time (ms) | Notes                              |
|----------|-----------|--------------------------------------|
| CBC      | 31-48     | Fastest, but verbose output       |
| Microlp  | 80-120    | Slower but pure Rust and silent   |

**Recommendation**: Use the default (Microlp) for most cases:
- Use **CBC** if you need maximum performance and can tolerate verbose output or redirect stderr
- **Microlp** is the default because it's pure Rust, silent, and supports all required features

## Verification

Both solvers produce identical results - only performance and output verbosity differ:

```bash
# Test with Microlp (default)
cargo test

# Test with CBC
cargo test --no-default-features --features cbc

# Or explicitly test Microlp
cargo test --no-default-features --features microlp
```

**Note**: Enable exactly one solver feature for a given deployment. Building
with no solver feature (`--no-default-features` with neither `microlp` nor
`cbc`) fails fast with a clear error. Building with both enabled (for example
`cargo test --all-features` in CI) is accepted: the build prefers Microlp.

## Technical Details

The solver choice is made at compile time via Cargo features. Quoracle uses the `good_lp` crate, which provides a unified interface to multiple solver backends. The API remains identical regardless of which solver you choose - only the backend implementation changes.

### Why not expose solver selection in the API?

Solver selection is a deployment/performance concern, not an algorithm concern. Making it a compile-time feature keeps the API clean and ensures consistent behavior across your application. If you need different solvers for different use cases, consider:

1. **Using the default for most cases** - Microlp is good enough for 95% of use cases
2. **Profiling first** - Only switch to CBC if benchmarks show you need it
3. **Separate builds** - If you really need multiple solvers, compile separate binaries

## Migration from v1.1.0

Version 1.1.0 used CBC by default. If you want to keep using CBC:

```toml
[dependencies]
# Explicit CBC (fastest performance, verbose output)
quoracle = { version = "1.4", default-features = false, features = ["cbc"] }
```

Otherwise, the default Microlp solver will work as a drop-in replacement with no code changes needed.
