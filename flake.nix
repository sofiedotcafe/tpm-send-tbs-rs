{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    pre-commit-hooks-nix = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];

      imports = [
        inputs.pre-commit-hooks-nix.flakeModule
      ];

      perSystem =
        {
          system,
          pkgs,
          config,
          ...
        }:
        let
          target = "x86_64-pc-windows-gnu";

          toolchain =
            with inputs.fenix.packages.${system};
            combine [
              default.cargo
              default.rustc
              default.clippy

              targets.${target}.latest.rust-std
            ];

          mingw = pkgs.pkgsCross.mingwW64;

          commonInputs =
            with mingw.buildPackages;
            with mingw.windows;
            {
              nativeBuildInputs = [
                binutils
                gcc
              ];

              buildInputs = [
                toolchain
                pthreads
              ];

              CARGO_BUILD_TARGET = target;

              CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = "${gcc}/bin/${gcc.targetPrefix}gcc";
            };

          naersk' = inputs.naersk.lib.${system}.override {
            cargo = toolchain;
            rustc = toolchain;
          };

        in
        {
          packages.default = naersk'.buildPackage (
            {
              src = ./.;
              strictDeps = true;
            }
            // commonInputs
          );

          devShells.default = pkgs.mkShell (
            {
              inputsFrom = [ config.pre-commit.devShell ];
            }
            // commonInputs
          );

          pre-commit = {
            check.enable = true;

            settings.hooks = {
              rustfmt.enable = true;
              clippy = {
                enable = true;
                package = toolchain;
                pass_filenames = false;
                entry = "cargo";
                args = [
                  "clippy"
                  "--no-deps"
                  "--offline"
                  "--"
                  "-Dwarnings"
                ];
                files = "\\.rs$";
                types = [ "file" ];
              };

              deadnix.enable = true;
              statix.enable = true;

              nixfmt-rfc-style.enable = true;

              yamlfmt.enable = true;
              markdownlint.enable = true;
            };
          };
        };
    };
}
