{
  description = "Very simple dotfile manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
    flakebox = {
      url = "github:rustshop/flakebox";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      flakebox,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        projectName = "dotr";

        flakeboxLib = flakebox.lib.mkLib pkgs {
          config = {
            github.ci.buildOutputs = [ ".#ci.${projectName}" ];
            just.importPaths = [ "justfile.custom.just" ];
            just.rules.watch.enable = false;
            # Nixpkgs 26.05 still packages Wild 0.8, which cannot consume
            # Flakebox's compressed-debug-section linker flag.
            linker.wild.enable = false;
            linker.mold.enable = true;
            toolchain.components = [
              "rustc"
              "cargo"
              "clippy"
              "rust-analyzer"
              "rust-src"
              "rustfmt"
            ];
          };
        };

        buildPaths = [
          "Cargo.toml"
          "Cargo.lock"
          "src"
          "tests"
        ];

        buildSrc = flakeboxLib.filterSubPaths {
          root = builtins.path {
            name = projectName;
            path = ./.;
          };
          paths = buildPaths;
        };

        multiBuild = (flakeboxLib.craneMultiBuild { }) (
          craneLib':
          let
            craneLib = craneLib'.overrideArgs {
              pname = projectName;
              src = buildSrc;
              nativeBuildInputs = [ ];
              env.RUSTDOCFLAGS = "-D warnings";
            };
          in
          rec {
            workspaceDeps = craneLib.buildWorkspaceDepsOnly { };

            workspace = craneLib.buildWorkspace {
              cargoArtifacts = workspaceDeps;
            };

            tests = craneLib.cargoNextest {
              cargoArtifacts = workspace;
              doInstallCargoArtifacts = false;
              cargoNextestExtraArgs = "--workspace";
            };

            clippy = craneLib.cargoClippy {
              cargoArtifacts = workspaceDeps;
              doInstallCargoArtifacts = false;
            };

            cargoFmt = craneLib.cargoFmt { };

            ${projectName} = craneLib.buildPackage {
              cargoArtifacts = workspaceDeps;
            };
          }
        );

        treefmt =
          pkgs.runCommand "treefmt-check"
            {
              nativeBuildInputs = [
                pkgs.treefmt
                pkgs.nixfmt-rfc-style
                pkgs.rustfmt
                pkgs.taplo
              ];
              src = self;
            }
            ''
              cp -r $src work && chmod -R u+w work
              cd work
              treefmt --ci
              touch $out
            '';
      in
      {
        packages = {
          treefmt = treefmt;
          default = multiBuild.${projectName};
        };
        legacyPackages = multiBuild;

        devShells = flakeboxLib.mkShells {
          packages = [ pkgs.taplo ];
        };
      }
    );
}
