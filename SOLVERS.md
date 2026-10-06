# LP solver backends

`QuorumSystem::strategy` solves a linear program. Quoracle uses
[`good_lp`](https://crates.io/crates/good_lp) and supports two backends,
selected with Cargo features. Resilience is computed exactly without any
solver.

| Feature | Backend | Needs | Notes |
|---|---|---|---|
| `microlp` (default) | [microlp](https://crates.io/crates/microlp), pure Rust | nothing | Works everywhere, including WASM. |
| `cbc` | [COIN-OR CBC](https://github.com/coin-or/Cbc) | system `libCbcSolver` (Debian: `coinor-libcbc-dev`, Nix: `cbc`) | Solves are serialized through a global lock because CBC is not thread-safe. |

```toml
# default
quoracle = "2"

# CBC
quoracle = { version = "2", default-features = false, features = ["cbc"] }
```

If both features are enabled (for example `--all-features`), CBC is used.
Building with neither fails with a compile error.

## Which one?

Use the default. Measured on an EC2 c6id.4xlarge, Rust 1.99, one thread:

| Problem | microlp | CBC |
|---|---|---|
| majority of 5, load-optimal | 0.14 ms | 0.80 ms |
| majority of 9 | 1.2 ms | 1.6 ms |
| majority of 13 | 71 ms | 72 ms |
| majority of 15 (6,435 quorums) | 0.89 s | 1.10 s |
| 5×5 grid | 56 ms | 51 ms |

Both backends produce the same optimal values; the whole test suite runs
against each in CI. Pick CBC only if you already ship it.

## Why not Clarabel or HiGHS?

Strategy optimization only needs continuous variables, so any LP solver
would work, but only these two are tested. Contributions welcome.
