#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
dir=target/upstream-seed
mkdir -p "$dir"
for pair in cc:RUST16 ld:LINK16; do
    tool=${pair%:*}
    name=${pair#*:}
    scripts/rustc16.sh -Ctarget-cpu=8086 -Copt-level=1 --cfg "dos_$tool" --crate-name "$tool" backend/seed.rs -o "$dir/$name.OBJ"
    target/release/link16 "$dir/$name.OBJ" dist/RT.OBJ -o "$dir/$name.EXE"
done
