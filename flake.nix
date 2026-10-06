{
  description = "Quoracle - construct and analyze read-write quorum systems";

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
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        lib = pkgs.lib;

        # Latest stable toolchain (pinned by flake.lock via rust-overlay).
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" "llvm-tools-preview" ];
        };
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        cargoToml = lib.importTOML ./Cargo.toml;

        # Only the files cargo needs (the README and guide are doctested).
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./build.rs
            ./src
            ./tests
            ./examples
            ./benches
            ./README.md
            ./docs/src/quick-start.md
            ./docs/src/guide.md
          ];
        };

        mkQuoracle = { solver ? "microlp" }:
          rustPlatform.buildRustPackage {
            pname = "quoracle" + lib.optionalString (solver != "microlp") "-${solver}";
            inherit (cargoToml.package) version;
            inherit src;
            cargoLock.lockFile = ./Cargo.lock;

            buildNoDefaultFeatures = true;
            buildFeatures = [ solver ];

            nativeBuildInputs = lib.optionals (solver == "cbc") [ pkgs.pkg-config ];
            buildInputs = lib.optionals (solver == "cbc") [ pkgs.cbc ];

            # Library crate: the useful output is a tested build, not a binary.
            doCheck = true;

            meta = {
              description = cargoToml.package.description;
              homepage = cargoToml.package.repository;
              license = with lib.licenses; [ mit asl20 ];
              platforms = lib.platforms.unix;
            };
          };

        quoracle = mkQuoracle { };
        quoracle-cbc = mkQuoracle { solver = "cbc"; };
      in
      {
        packages = {
          default = quoracle;
          inherit quoracle quoracle-cbc;
        };

        checks = {
          inherit quoracle quoracle-cbc;

          fmt = pkgs.runCommand "quoracle-fmt" { nativeBuildInputs = [ rustToolchain ]; } ''
            cd ${./.}
            cargo fmt --all -- --check
            touch $out
          '';
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            rustToolchain
            pkgs.pkg-config
            pkgs.rust-analyzer
            pkgs.cargo-llvm-cov
            pkgs.cargo-deny
            pkgs.cargo-edit
            pkgs.cargo-msrv
            pkgs.mdbook
          ];
          buildInputs = [ pkgs.cbc ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        formatter = pkgs.nixpkgs-fmt;
      }
    );
}
