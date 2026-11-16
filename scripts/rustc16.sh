#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
out="$root/target/dos-sysroot"
sys=$(cat "$out/sysroot.path")
backend=$(cat "$out/backend.path")
exec "$sys/bin/rustc" --edition=2024 -Zunstable-options -Zcodegen-backend="$backend" --target "$root/backend/dos16.json" --crate-type=lib --emit=obj -Cpanic=abort -Coverflow-checks=off -L dependency="$out" --extern compiler_builtins="$out/libcompiler_builtins.rmeta" --extern core="$out/libcore.rmeta" "$@"
