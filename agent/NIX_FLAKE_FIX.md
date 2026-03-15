# Nix Flake Fix Summary

## Problem

The original `flake.nix` had two issues:

1. **Undefined variable `coin-cbc`**: The package name was incorrect for nixpkgs
2. **Missing Cargo.lock**: The flake tried to build a package but `Cargo.lock` is gitignored

## Solution

Simplified the flake to **only provide a development environment** instead of building a package.

### Why This Makes Sense

1. **Users get the library from crates.io** - They don't need Nix to build/install it
2. **Developers get the tools they need** - Nix provides Rust, cargo tools, and dev dependencies
3. **No external C dependencies needed** - The default `microlp` solver is pure Rust
4. **Simpler and more reliable** - No need to track Cargo.lock in git

## Changes Made

### Before (Broken)

```nix
buildInputs = with pkgs; [
  coin-cbc  # ❌ Undefined variable
]

quoracle = pkgs.rustPlatform.buildRustPackage {
  # ...
  cargoLock = {
    lockFile = ./Cargo.lock;  # ❌ File is gitignored
  };
}
```

### After (Working)

```nix
{
  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      {
        # Only provide dev shell, not a package
        devShells.default = pkgs.mkShell {
          buildInputs = [ rustToolchain ];
          nativeBuildInputs = [
            rust-analyzer
            cargo-edit
            cargo-audit
            cargo-deny
            cargo-tarpaulin
            mdbook
          ];
        };
      }
    );
}
```

## What the Flake Provides

✅ **Complete Rust development environment:**
- Latest stable Rust toolchain
- rust-analyzer for IDE integration
- cargo-edit, cargo-audit, cargo-deny
- cargo-tarpaulin for coverage
- mdbook for documentation

✅ **No external dependencies needed:**
- Default `microlp` solver is pure Rust
- No CBC or other C libraries required

✅ **Cross-platform:**
- Works on Linux and macOS
- Automatic platform-specific dependencies

## Usage

### Enter Development Environment

```bash
# One-time: enable direnv (optional)
direnv allow

# Or manually enter
nix develop
```

### Build and Test

Once in the Nix environment:

```bash
# Build
cargo build

# Test
cargo test

# Lint
cargo clippy

# Documentation
cargo doc --open
mdbook serve

# Benchmarks
cargo bench
```

### Exit Environment

```bash
exit  # or Ctrl+D
```

## What About CBC?

The CBC solver is **not included** in the Nix environment because:

1. **Default is microlp** - Pure Rust, no external deps
2. **CBC is optional** - Most users don't need it
3. **Simpler setup** - No system library dependencies

If you need CBC, install it separately:
- Linux: `apt install coinor-libcbc-dev`
- macOS: `brew install cbc`
- Then use: `cargo test --no-default-features --features cbc`

## Benefits of This Approach

1. **✅ Works out of the box** - No configuration needed
2. **✅ Reproducible** - Same environment for all developers
3. **✅ Fast** - Nix caches everything
4. **✅ Clean** - No pollution of system packages
5. **✅ Cross-platform** - Works on Linux, macOS, and NixOS

## Verification

Test that the flake works:

```bash
# Check flake validity
nix flake check

# Enter dev environment
nix develop

# Should see:
# 🔬 Quoracle development environment
# Rust: rustc 1.94.0 ...
# Cargo: cargo 1.94.0 ...

# Verify tools are available
which rustc
which cargo
which rust-analyzer
which cargo-deny
which mdbook

# Build and test
cargo build
cargo test
```

## For NixOS Users

If you're on NixOS, the flake automatically provides:

- Reproducible dev environment
- No need for system packages
- Easy to share with collaborators

Just use:
```bash
nix develop
```

## Troubleshooting

### If `nix develop` fails

1. Make sure you have Nix installed with flakes enabled
2. Check that you're in the repository directory
3. Try: `nix flake update` to refresh inputs

### If direnv isn't working

1. Install direnv: `nix profile install nixpkgs#direnv`
2. Add to your shell rc file:
   ```bash
   eval "$(direnv hook bash)"  # or zsh
   ```
3. Run: `direnv allow`

### If build fails with "linker not found"

Make sure you're inside `nix develop`:
```bash
nix develop
cargo build  # Now it should work
```

## Future Enhancements

Possible additions:

- Add CBC as optional development dependency
- Add pre-commit hooks configuration
- Add Docker integration
- Add CI environment matching

## Related Files

- `flake.nix` - Nix flake definition
- `.envrc` - direnv configuration (auto-loads Nix env)
- `flake.lock` - Lock file with exact dependency versions

## Summary

The Nix flake is now **simpler and more focused**:

- ❌ ~~Building packages~~ (use crates.io)
- ✅ **Development environment** (Rust + tools)
- ✅ **Reproducible** (same env for everyone)
- ✅ **Simple** (no external deps needed)

Just run `nix develop` and start coding! 🚀
