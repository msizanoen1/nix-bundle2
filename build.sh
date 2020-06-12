#!/bin/bash
PATH="$coreutils/bin:$gnused/bin:$PATH"
mkdir -p $out/usr/lib
echo -n "$run" > $out/nixon_command.txt
cp $src/nixon $out/AppRun
cp -a $(cat $storePaths) $out/usr/lib/
