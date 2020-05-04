with import <nixpkgs> {};

rustPlatform.buildRustPackage {
    pname = "nix-user-chroot";
    version = "1.0.3";

    src = ./nix-user-chroot;

    cargoSha256 = "0f835n96p8rxrr96zqwai8wx553yrn65pgy22wrrm6kgvlhhq67h";
    target = "x86_64-unknown-linux-musl";
}
