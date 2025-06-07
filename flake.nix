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
      in
      {
        formatter = pkgs.nixpkgs-fmt;

        packages = {
          # For `nix build` `nix run`, & `nix profile install`:
          default = naersk'.buildPackage {
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
        };

        devShells.default = pkgs.mkShell {
          inherit buildInputs;
          nativeBuildInputs = with pkgs; [ rustup toolchain ];
        };
      }
    );
}
