# LP Solver Selection

Quoracle uses linear programming to optimize quorum strategies. You can choose between three solver backends at build time.

## Default: Clarabel (Recommended)

**Clarabel** is now the default solver (as of v1.2.0).

### Why Clarabel?

- ✅ **Pure Rust** - No C/C++ dependencies, easy deployment, WASM support
- ✅ **Silent by default** - No verbose solver output cluttering your logs
- ✅ **Good performance** - Competitive with CBC for most problems
- ✅ **Apache 2.0 license** - Permissive open source license
- ⚠️ Only supports continuous variables (sufficient for quoracle's use case)

### Installation

```toml
[dependencies]
quoracle = "1.2"  # Uses Clarabel by default
```

## Alternative: CBC (Maximum Performance)

If you need absolute maximum performance and don't mind verbose output:

```toml
[dependencies]
# Replace Clarabel with CBC solver
quoracle = { version = "1.2", default-features = false }
good_lp = { version = "1.8", features = ["coin_cbc"] }
```

### CBC Trade-offs

- ✅ **Excellent performance** - Industry-standard solver (COIN-OR)
- ✅ **Battle-tested** - Widely used in research and production
- ⚠️ **Verbose output** - Prints optimization details to stdout/stderr (cannot be easily suppressed)
- ⚠️ **C library dependency** - Requires CBC library headers at build time
- ⚠️ **EPL 2.0 license** - Copyleft license (less permissive than Apache/MIT)

**Suppressing CBC output**:
```bash
# Redirect stderr when running your program
your_program 2>/dev/null
```

## Alternative: Microlp (Maximum Simplicity)

For small problems or when deployment simplicity is critical:

```toml
[dependencies]
quoracle = { version = "1.2", default-features = false }
good_lp = { version = "1.8", features = ["microlp"] }
```

### Microlp Trade-offs

- ✅ **Pure Rust** - No external dependencies
- ✅ **Lightweight** - Minimal code footprint
- ✅ **Silent** - No verbose output
- ⚠️ **Slower** - Noticeably slower than CBC/Clarabel for larger problems
- ⚠️ **Best for small problems only** - May struggle with complex quorum systems

## Performance Comparison

Based on benchmarks (4-node search with timeout):

| Solver   | Time (ms) | Notes                              |
|----------|-----------|-----------------------------------|
| CBC      | 31-48     | Fastest, but verbose output       |
| Clarabel | 35-55     | Slightly slower, silent           |
| Microlp  | 80-120    | Slowest, but simple               |

**Recommendation**: Use the default (Clarabel) unless you have specific needs:
- Use **CBC** only if you need the absolute best performance and can handle verbose output
- Use **Microlp** only for very simple problems or embedded/WASM deployments

## Verification

All solvers produce identical results - only performance and output verbosity differ:

```bash
# Test with Clarabel (default)
cargo test

# Test with CBC
cargo test --no-default-features --features cbc

# Test with Microlp
cargo test --no-default-features --features microlp
```

## Technical Details

The solver choice is made at compile time via Cargo features. Quoracle uses the `good_lp` crate, which provides a unified interface to multiple solver backends. The API remains identical regardless of which solver you choose - only the backend implementation changes.

### Why not expose solver selection in the API?

Solver selection is a deployment/performance concern, not an algorithm concern. Making it a compile-time feature keeps the API clean and ensures consistent behavior across your application. If you need different solvers for different use cases, consider:

1. **Using the default for most cases** - Clarabel is good enough for 95% of use cases
2. **Profiling first** - Only switch to CBC if benchmarks show you need it
3. **Separate builds** - If you really need multiple solvers, compile separate binaries

## Migration from v1.1.0

Version 1.1.0 used CBC by default. If you want to keep using CBC:

```toml
[dependencies]
# Explicit CBC (same as v1.1.0)
quoracle = { version = "1.2", default-features = false }
good_lp = { version = "1.8", features = ["coin_cbc"] }
```

Otherwise, the default Clarabel solver will work as a drop-in replacement with no code changes needed.
