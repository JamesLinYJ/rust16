#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked
mkdir -p dist
cp examples/tri.rs dist/TRI.RS
cp examples/crc.rs dist/CRC.RS
cat dos/common.rs dos/compile.rs > dist/CC.RS
cat dos/common.rs dos/link.rs > dist/LD.RS
bin=target/release
"$bin/mkrt16" dist/RT.OBJ
for part in CC LD; do "$bin/rust16" "dist/$part.RS" -o "dist/$part.OBJ"; done
"$bin/link16" dist/CC.OBJ dist/RT.OBJ -o dist/RUST16.EXE
"$bin/link16" dist/LD.OBJ dist/RT.OBJ -o dist/LINK16.EXE
