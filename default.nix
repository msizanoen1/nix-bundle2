{ package, run }:

with import <nixpkgs> {};

let pkgClosureInfo = closureInfo { rootPaths = package; };
in

derivation {
    name = "nix-bundle2";
    builder = "${bash}/bin/bash";
    coreutils = coreutils;
    gnused = gnused;
    src = ./src;
    system = builtins.currentSystem;
    args = [ ./build.sh ];
    run = "${package}${run}";
    storePaths = "${pkgClosureInfo}/store-paths";
}
