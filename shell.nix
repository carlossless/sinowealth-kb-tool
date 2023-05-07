{ pkgs ? import <nixpkgs> { } }:
with pkgs;
mkShell {
  buildInputs = [
    rustc
    cargo
  ] ++
  (lib.optionals (stdenv.hostPlatform.isLinux) [
    udev
    pkg-config
  ]) ++
  (lib.optionals (stdenv.hostPlatform.isDarwin) [
    darwin.apple_sdk.frameworks.IOKit
    darwin.apple_sdk.frameworks.AppKit
    iconv
  ]);

  shellHook = ''
    echo "Entered the dev shell! $(rustc --version) $(cargo --version)"
  '';
}
