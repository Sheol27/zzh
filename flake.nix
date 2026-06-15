{
  description = "A flake for Rust project development on NixOS";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs { inherit system; };
    in {
      devShells.default = pkgs.mkShell {
        name = "rust-dev-shell";
        packages = [
          pkgs.rustc
          pkgs.cargo
          pkgs.rust-analyzer
          pkgs.clippy
          pkgs.rustfmt
        ];
        shellHook = ''
          echo "Welcome to your Rust development environment"
        '';
      };
    });
}
