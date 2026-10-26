{
  description = "Tools for reading entropy from a PRG320 hardware random number generator via USB serial";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        nativeBuildInputs = with pkgs; [ pkg-config ];
        buildInputs = with pkgs;
          lib.optionals stdenv.hostPlatform.isLinux [ udev ]
          ++ lib.optionals stdenv.hostPlatform.isDarwin [
            darwin.apple_sdk.frameworks.IOKit
            darwin.apple_sdk.frameworks.CoreFoundation
          ];

        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          inherit nativeBuildInputs buildInputs;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        entrops = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

      in {
        packages = {
          default = entrops;
          inherit entrops;
        };

        checks = {
          build = entrops;

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- -D warnings";
          });

          fmt = craneLib.cargoFmt {
            src = commonArgs.src;
          };

          tests = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps = {
          default = {
            type = "app";
            program = "${entrops}/bin/entrops";
            meta.description = "Read entropy from a PRG320 and print it as JSON";
          };
          serve = {
            type = "app";
            program = "${entrops}/bin/serve";
            meta.description = "Stream entropy data from all USB serial devices via SSE";
          };
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [ rust-analyzer ];
        };
      });
}
