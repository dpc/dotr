{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, fenix, naersk, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
    let pkgs = nixpkgs.legacyPackages.${system};
    in {
      packages.dotr = (pkgs.makeRustPlatform {
        inherit (fenix.packages.${system}.minimal) cargo rustc;
      }).buildRustPackage {
        pname = "dotr";
        version = "0.4.0";
        src = ./.;
        cargoSha256 = "sha256-E0TXVz4ziIeuXGKfdJ0ROHcYZLWXWJxs+waMgxc5MVM=";
      };

      defaultPackage = self.packages.${system}.dotr;
      defaultApp = self.packages.${system}.dotr;

      devShell =
        pkgs.mkShell {
          nativeBuildInputs = [ fenix.packages.${system}.minimal.rustc ];
          buildInputs = [];
        };
  });
}
