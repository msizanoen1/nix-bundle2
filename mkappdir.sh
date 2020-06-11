#!/bin/sh
if [ -z "$1" ] || [ -z "$2" ] || [ -z "$3" ]
then
    echo "Usage: mkappdir.sh PACKAGE BINARY OUTDIRLINK"
    exit 1
fi
nix-build -E "import ./default.nix{package=(import<nixpkgs>{}).$1;run=\"$2\";}" -o $3
