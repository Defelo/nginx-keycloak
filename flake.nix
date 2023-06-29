{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-23.05";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = {
    self,
    nixpkgs,
    fenix,
    naersk,
    ...
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
    target = "x86_64-unknown-linux-musl";
    toolchain = with fenix.packages.${system};
      combine [
        stable.rustc
        stable.cargo
        targets.${target}.stable.rust-std
      ];
    naersk' = naersk.lib.${system}.override {
      cargo = toolchain;
      rustc = toolchain;
    };
  in {
    packages.${system} = {
      default = self.packages.${system}.nginx-keycloak;
      nginx-keycloak = naersk'.buildPackage {
        src = pkgs.stdenvNoCC.mkDerivation {
          name = "nginx-keycloak-src";
          phases = ["installPhase"];
          installPhase = ''
            mkdir $out
            ln -s ${./Cargo.toml} $out/Cargo.toml
            ln -s ${./Cargo.lock} $out/Cargo.lock
            ln -s ${./src} $out/src
          '';
        };
        CARGO_BUILD_TARGET = target;
      };
    };
  };
}
