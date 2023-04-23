{ pkgs ? import <nixpkgs> { } }:
with pkgs;
mkShell {
  buildInputs = lib.flatten [
    [rustup]
    (lib.optionals (stdenv.hostPlatform.isDarwin) [
      darwin.apple_sdk.frameworks.IOKit
      darwin.apple_sdk.frameworks.AppKit
      iconv
    ])
  ];

  shellHook = ''
    echo "Entered the dev shell! $(rustup --version) $(cargo --version)"
  '';
}
