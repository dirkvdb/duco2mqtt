{
  description = "duco2mqtt";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustChannel = pkgs.rust-bin.stable.latest;
        rustToolchain = rustChannel.default.override {
          extensions = [
            "rust-src"
          ];
        };
        rustAnalyzer = rustChannel.rust-analyzer;
      in
      {
        devShells = {
          default =
            with pkgs;
            mkShell {
              buildInputs = [
                cargo-nextest
                nil
                nixfmt-rfc-style
                just
                rustAnalyzer
                rustToolchain
              ];
            };
        };

        packages = {
          # regular, host-native build (dynamic)
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "duco2mqtt";
            version = "1.0.0";

            src = ./.;

            # assuming you have a Cargo.lock
            cargoLock.lockFile = ./Cargo.lock;
          };
        }
        // (
          # musl-static package, only on Linux
          if pkgs.stdenv.isLinux then
            {
              static = pkgs.pkgsStatic.rustPlatform.buildRustPackage {
                pname = "duco2mqtt";
                version = "1.0.0";

                src = ./.;

                cargoLock.lockFile = ./Cargo.lock;
              };
            }
          else
            { }
        );
      }
    );
}
