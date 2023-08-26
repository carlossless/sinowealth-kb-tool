{ pkgs ? import <nixpkgs> { } }:
with pkgs;
mkShell {
  buildInputs = [
    rustc
    cargo
    pkg-config
    libusb1
  ] ++
  (lib.optionals (stdenv.hostPlatform.isLinux) [
    udev
  ]) ++
  (lib.optionals (stdenv.hostPlatform.isDarwin) [
    darwin.apple_sdk.frameworks.IOKit
    darwin.apple_sdk.frameworks.AppKit
    iconv
  ]);

  RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

  shellHook = ''
    echo "Entered the dev shell! $(rustc --version) $(cargo --version)"
  '';
}
