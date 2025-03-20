{
  lib,
  rustPlatform,
  ...
}: let
  manifest = (lib.importTOML ./Cargo.toml).package;
in
  rustPlatform.buildRustPackage {
    pname = manifest.name;
    inherit (manifest) version;

    src = ./.;

    cargoLock = {
      lockFile = ./Cargo.lock;
    };

    meta = with lib; {
      inherit (manifest) description;
      inherit (manifest) homepage;
      license = licenses.mit;
    };
  }
