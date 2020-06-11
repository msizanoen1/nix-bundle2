#!/bin/bash
PATH="$coreutils/bin:$gnused/bin:$PATH"
if [ -z "$wrapperName" ]; then
    wrapperName="$(basename $run)"
fi
mkdir -p $out/nix/store
cat $src/entrypoint.in \
    | sed "s,@NIX_BINARY@,$run," \
    | cat > $out/$wrapperName
chmod +x $out/$wrapperName
ln -sf $wrapperName $out/AppRun
cp $src/nixon $out/
cp -a $(cat $storePaths) $out/nix/store/
