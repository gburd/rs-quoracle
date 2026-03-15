{
  description = "Quoracle - A library for constructing and analyzing read-write quorum systems";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
          ] ++ lib.optionals stdenv.isDarwin [
            darwin.apple_sdk.frameworks.Security
          ];

          nativeBuildInputs = with pkgs; [
            # Development tools
            rust-analyzer
            cargo-edit
            cargo-audit
            cargo-deny
            cargo-tarpaulin

            # Documentation
            mdbook
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "🔬 Quoracle development environment"
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo ""
            echo "Default solver: microlp (pure Rust, no external deps)"
            echo ""
            echo "Available commands:"
            echo "  cargo build        - Build the library"
            echo "  cargo test         - Run tests"
            echo "  cargo clippy       - Run linter"
            echo "  cargo doc --open   - Generate and open docs"
            echo "  cargo bench        - Run benchmarks"
            echo ""
            echo "Note: CBC solver not included in Nix environment."
            echo "Tests use microlp by default (no external dependencies)."
          '';
        };
      }
    );
}
