{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, utils, naersk, rust-overlay }:
    utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];

        pkgs = import nixpkgs {
          inherit system overlays;
        };

        toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        naersk' = pkgs.callPackage naersk {
          cargo = toolchain;
          rustc = toolchain;
          clippy = toolchain;
        };

        buildInputs = with pkgs; [
          pkg-config
          libusb1
          binutils
        ] ++
        (lib.optionals (stdenv.hostPlatform.isLinux) [
          udev
        ]) ++
        (lib.optionals (stdenv.hostPlatform.isDarwin) [
          apple-sdk
          iconv
        ]);

        package = naersk'.buildPackage {
          pname = "sinowealth-kb-tool";
          version = "latest";

          src = ./.;

          doCheck = false; # integration tests from running since they require an attached specific device

          inherit buildInputs;

          meta = with pkgs.lib; {
            description = "A utility for reading and writing flash contents on Sinowealth 8051-based devices";
            homepage = "https://github.com/carlossless/sinowealth-kb-tool";
            license = licenses.mit;
            mainProgram = "sinowealth-kb-tool";
            maintainers = with maintainers; [ carlossless ];
          };
        };

        lib = nixpkgs.lib;

        darwin = [ "x86_64-darwin" "aarch64-darwin" ];
        linux = [ "x86_64-linux" "aarch64-linux" ];
        allSystems = darwin ++ linux;
        forEachSystem = systems: f: lib.genAttrs systems (system: f system);

        naerskBuildPackage = target: args:
          naersk'.buildPackage (
            args
              // { CARGO_BUILD_TARGET = target; }
          );
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        packages = {
          # For `nix build` `nix run`, & `nix profile install`:
          default = package;

          x86_64-unknown-linux-gnu = naerskBuildPackage "x86_64-unknown-linux-gnu" {
            src = ./.;

            doCheck = false;

            depsBuildBuild = with pkgs; [
              pkgsCross.gnu64.stdenv.cc
            ];

            nativeBuildInputs = with pkgs; [
              pkg-config
            ];

            buildInputs = with pkgs; [
              pkgsCross.gnu64.libusb1
            ];
          };

          i386-unknown-linux-gnu = naerskBuildPackage "i386-unknown-linux-gnu" {
            src = ./.;

            doCheck = false;

            depsBuildBuild = with pkgs; [
              pkgsCross.gnu32.stdenv.cc
            ];

            nativeBuildInputs = with pkgs; [
              pkg-config
            ];

            buildInputs = with pkgs; [
              pkgsCross.gnu32.libusb1
            ];
          };

          aarch64-unknown-linux-gnu = naerskBuildPackage "aarch64-unknown-linux-gnu" {
            src = ./.;

            doCheck = false;

            depsBuildBuild = with pkgs; [
              pkgsCross.aarch64-multiplatform.stdenv.cc
            ];

            nativeBuildInputs = with pkgs; [
              pkg-config
            ];

            buildInputs = with pkgs; [
              pkgsCross.aarch64-multiplatform.libusb1
            ];
          };
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = with pkgs; [ rustup toolchain ];
        };
      }
    );
}
