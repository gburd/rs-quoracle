# Verification Guide

This guide helps verify that all compiler warnings and errors have been fixed.

## Quick Verification

Run these commands to verify all issues are resolved:

```bash
# 1. Clean build with default features (microlp)
cargo clean
cargo build --verbose

# 2. Build with CBC feature
cargo build --no-default-features --features cbc --verbose

# 3. Run all tests
cargo test --verbose
cargo test --no-default-features --features cbc --verbose

# 4. Check for warnings with strict lints
cargo clippy --all-targets --all-features -- -D warnings

# 5. Build documentation
cargo doc --no-deps --document-private-items

# 6. Format check
cargo fmt --all -- --check

# 7. Build benchmarks
cargo bench --no-run
```

## Expected Results

### ✅ All Builds Should Succeed

**Default (microlp):**
```
$ cargo build
   Compiling quoracle v1.2.1
    Finished dev [unoptimized + debuginfo] target(s) in 5.23s
```

**CBC feature:**
```
$ cargo build --no-default-features --features cbc
   Compiling quoracle v1.2.1
    Finished dev [unoptimized + debuginfo] target(s) in 5.45s
```

### ✅ No Warnings

**Build script:**
- No "missing documentation for the crate" warning from `build.rs`

**Library code:**
- No trait type errors
- No missing method warnings
- No unused imports

### ✅ All Tests Pass

```
$ cargo test
running 155 tests
...
test result: ok. 155 passed; 0 failed; 0 ignored
```

### ✅ Clippy Clean

```
$ cargo clippy --all-targets --all-features -- -D warnings
    Finished dev [unoptimized + debuginfo] target(s) in 0.23s
```

No clippy warnings should appear.

### ✅ Documentation Builds

```
$ cargo doc --no-deps
 Documenting quoracle v1.2.1
    Finished dev [unoptimized + debuginfo] target(s) in 3.12s
```

## Detailed Verification

### Check Each Fix

#### 1. Build Script Documentation

**Verify:**
```bash
head -n 5 build.rs
```

**Expected output:**
```rust
//! Build script for quoracle.
//!
//! Enforces mutually exclusive solver features (microlp or cbc).

fn main() {
```

✅ Should see documentation comment (`//!`) at the top.

---

#### 2. Trait Type Fixes

**Verify in `src/quorum_system.rs`:**

**Line ~469:** `add_probability_sum_constraints`
```bash
sed -n '469,475p' src/quorum_system.rs
```

**Expected:**
```rust
fn add_probability_sum_constraints<S: good_lp::SolverModel>(
    mut problem: S,
    r_vars: &[good_lp::Variable],
    w_vars: &[good_lp::Variable],
) -> S {
```

✅ Should have generic parameter `<S: good_lp::SolverModel>`
✅ Parameter type: `S` (not `good_lp::SolverModel`)
✅ Return type: `S` (not `good_lp::SolverModel`)

---

**Line ~486:** `add_node_load_constraints`
```bash
sed -n '486,492p' src/quorum_system.rs
```

**Expected:**
```rust
fn add_node_load_constraints<S: good_lp::SolverModel>(
    &self,
    mut problem: S,
    load_info: &[(OrderedFloat, f64, good_lp::Variable)],
    x_to_r_vars: &HashMap<T, Vec<good_lp::Variable>>,
    x_to_w_vars: &HashMap<T, Vec<good_lp::Variable>>,
) -> S {
```

✅ Should have generic parameter `<S: good_lp::SolverModel>`

---

**Line ~521:** `extract_strategy_from_solution`
```bash
sed -n '521,528p' src/quorum_system.rs
```

**Expected:**
```rust
fn extract_strategy_from_solution(
    &self,
    solution: &impl good_lp::Solution,
    read_quorums: &[HashSet<T>],
    write_quorums: &[HashSet<T>],
    r_vars: &[good_lp::Variable],
    w_vars: &[good_lp::Variable],
) -> Result<Strategy<T>> {
```

✅ Should have `solution: &impl good_lp::Solution`

---

#### 3. SolverModel Import

**Verify in `src/quorum_system.rs`:**

**Line ~567:** Inside `lp_optimal_strategy` function
```bash
grep -A 2 "fn lp_optimal_strategy" src/quorum_system.rs | head -10
```

**Expected:**
```rust
use good_lp::{default_solver, Expression, ProblemVariables, SolverModel, Variable};
```

✅ Should include `SolverModel` in the use statement

---

## Common Issues

### Issue: "no method named 'with' found"

**Cause:** `SolverModel` trait not imported

**Fix:** Verify line 567 includes `SolverModel` in imports

---

### Issue: "expected a type, found a trait"

**Cause:** Using bare trait name instead of generic or `impl Trait`

**Fix:** Check that helper functions use `<S: SolverModel>` generic parameter

---

### Issue: Tests hang or fail

**Cause:** Might have both solver features enabled

**Fix:** Use `--no-default-features` with explicit feature:
```bash
cargo test --no-default-features --features microlp
cargo test --no-default-features --features cbc
```

---

## Files Modified

Summary of changes:

1. **`build.rs`**
   - Added: Documentation comment (`//!`)
   - Lines: 1-3

2. **`src/quorum_system.rs`**
   - Modified: `add_probability_sum_constraints` (line ~469)
   - Modified: `add_node_load_constraints` (line ~486)
   - Modified: `extract_strategy_from_solution` (line ~521)
   - Modified: `lp_optimal_strategy` imports (line ~567)

## Success Criteria

All of these should pass:

- ✅ `cargo build` succeeds with 0 warnings
- ✅ `cargo build --no-default-features --features cbc` succeeds
- ✅ `cargo test` passes all 155+ tests
- ✅ `cargo clippy -- -D warnings` shows no warnings
- ✅ `cargo doc` builds without warnings
- ✅ All examples compile and run
- ✅ Benchmarks compile

## Troubleshooting

### If build fails with "trait not found":

1. Check that `good_lp` is imported in `Cargo.toml`
2. Verify feature flags are correct
3. Run `cargo clean` and rebuild

### If tests hang:

1. You may have both features enabled
2. Use `cargo build --all-features` to see the intentional panic message
3. Use explicit single-feature builds

### If missing documentation warning persists:

1. Verify `build.rs` has `//!` at the top (not `//`)
2. Run `cargo clean` to clear old build artifacts
3. Rebuild with `cargo build`

## Additional Checks

### Check for Unused Allows

```bash
# Should show only justified allows (cast_precision_loss, test code, etc.)
grep -n "#\[allow" src/*.rs
```

### Check Examples

```bash
cargo run --example simple
cargo run --example tutorial
cargo run --example search
```

All should run without warnings or errors.

## Continuous Integration

The CI should pass all checks:

```yaml
- cargo fmt --all -- --check
- cargo clippy --all-targets --all-features -- -D warnings
- cargo deny check  # Supply chain security
- cargo build --verbose
- cargo test --verbose
```

## Contact

If you encounter any remaining issues:

1. Check `COMPILER_FIXES.md` for technical details
2. Review error messages carefully
3. Ensure you're using the correct feature flags
4. Try `cargo clean` and rebuild

---

**Last Updated:** 2026-03-15
**Rust Version:** 1.70+
**Quoracle Version:** 1.2.1
