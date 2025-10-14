{
  description = "Automatic workspace naming for niri window manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustToolchain = pkgs.rust-bin.beta.latest.default;
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "niri-autoname-workspaces";
          version = "0.1.0";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          buildInputs = [ pkgs.zenity ];
        };

        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rustToolchain
            zenity
            rustfmt
            taplo
            nixpkgs-fmt
            nodePackages.prettier
            treefmt
          ];
        };
      }
    );
}
