{
  description = "Magic Opener";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = {
    self,
    nixpkgs,
    ...
  }: let
    supportedSystems = ["aarch64-darwin" "x86_64-linux" "aarch64-linux" "x86_64-darwin"];
    forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

    nixpkgsFor = system:
      import nixpkgs {
        inherit system;
      };
  in {
    packages = forAllSystems (
      system: let
        pkgs = nixpkgsFor system;
      in {
        default = pkgs.callPackage ./default.nix {};
      }
    );

    overlays.default = final: prev: {
      magic-opener = self.packages.${prev.system}.default;
    };
  };
}
