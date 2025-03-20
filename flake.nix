{
  description = "Magic Opener";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    ...
  }: let
    supportedSystems = ["aarch64-darwin" "x86_64-linux" "aarch64-linux" "x86_64-darwin"];
    forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

    nixpkgsFor = system:
      import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };
  in {
    packages = forAllSystems (
      system: let
        pkgs = nixpkgsFor system;
        toolchain = pkgs.rust-bin.stable.latest.minimal;
        rustPlatform = pkgs.makeRustPlatform {
          rustc = toolchain;
          cargo = toolchain;
        };
      in {
        default = pkgs.callPackage ./default.nix {
          inherit rustPlatform;
        };
      }
    );

    overlays.default = final: prev: {
      magic-opener = self.packages.${prev.system}.default;
    };
  };
}
