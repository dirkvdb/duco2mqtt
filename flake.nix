{
  description = "duco2mqtt";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      self,
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
        )
        // {
          # Add dockerImage to packages

          dockerImage =
            let
              # Select the appropriate package (static for Linux, default otherwise)
              duco2mqttApp =
                if pkgs.stdenv.isLinux then self.packages.${system}.static else self.packages.${system}.default;

              # Create an FHS-like environment for the application.
              # This ensures the binary is at a predictable path like /app/bin/duco2mqtt
              # along with its runtime dependencies.
              appFhs = pkgs.buildEnv {
                name = "duco2mqtt-fhs";
                paths = [
                  duco2mqttApp
                ];
                postBuild = ''
                  mkdir -p $out/app/bin
                  ln -s ${duco2mqttApp}/bin/duco2mqtt $out/app/bin/duco2mqtt
                '';
              };

              app = pkgs.buildEnv {
                name = "duco2mqtt";
                paths = [
                  appFhs
                ];
              };
            in
            pkgs.dockerTools.buildImage {
              name = "duco2mqtt";
              tag = "latest";
              copyToRoot = app;

              config = {
                Cmd = [ "/app/bin/duco2mqtt" ]; # The command to run when the container starts
              };
            };
        };
      }
    );
}
