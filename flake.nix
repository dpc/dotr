{
  description = "A very basic flake";

  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    flakebox.url = "github:rustshop/flakebox";
  };

  outputs = { self, flakebox, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = flakebox.inputs.nixpkgs.legacyPackages.${system};

        lib = pkgs.lib;
        fs = lib.fileset;

        projectName = "dotr";

        flakeboxLib = flakebox.lib.${system} {
          config = {
            github.ci.buildOutputs = [ ".#ci.${projectName}" ];
          };
        };

        srcFileset = fs.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
        ];


        multiBuild =
          (flakeboxLib.craneMultiBuild { }) (craneLib':
            let
              craneLib = (craneLib'.overrideArgs {
                pname = projectName;
                src = fs.toSource {
                  root = ./.;
                  fileset = srcFileset;
                };
              });
            in
            {
              "${projectName}" = craneLib.buildPackage { };
            });
      in
      {
        packages = {
          default = multiBuild.dotr;
        };
        legacyPackages = multiBuild;

        devShells = flakeboxLib.mkShells { };
      });
}
