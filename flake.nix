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

        # Build inputs for the Rust package
        nativeBuildInputs = with pkgs; [
          rustToolchain
          pkg-config
        ];

        buildInputs = with pkgs; [
          # CBC solver (optional, for cbc feature)
          coin-cbc
        ] ++ lib.optionals stdenv.isDarwin [
          darwin.apple_sdk.frameworks.Security
        ];

        # The main package
        quoracle = pkgs.rustPlatform.buildRustPackage {
          pname = "quoracle";
          version = "1.2.1";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          inherit nativeBuildInputs buildInputs;

          # Run tests during build
          doCheck = true;

          meta = with pkgs.lib; {
            description = "A library for constructing and analyzing read-write quorum systems";
            homepage = "https://github.com/gregburd/quoracle";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
            platforms = platforms.unix;
          };
        };

      in
      {
        packages = {
          default = quoracle;
          inherit quoracle;
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = nativeBuildInputs ++ (with pkgs; [
            # Development tools
            rust-analyzer
            cargo-edit
            cargo-audit
            cargo-deny
            cargo-tarpaulin

            # Documentation
            mdbook
          ]);

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "🔬 Quoracle development environment"
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build        - Build the library"
            echo "  cargo test         - Run tests"
            echo "  cargo clippy       - Run linter"
            echo "  cargo doc --open   - Generate and open docs"
            echo "  cargo bench        - Run benchmarks"
          '';
        };

        # Expose checks for CI
        checks = {
          inherit quoracle;
        };
      }
    );
}
