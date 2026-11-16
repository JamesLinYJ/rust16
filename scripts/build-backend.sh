#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
root=$(pwd)
sys=${RUST16_SYSROOT:-$(rustc +nightly-2026-11-05 --print sysroot)}
commit=$("$sys/bin/rustc" -vV | sed -n 's/^commit-hash: //p')
test "$commit" = 2d20b42b883dcfc014f3c2c56081eeb6091c2179 || { echo "rustc-dev and sources must match nightly-2026-11-05" >&2; exit 1; }
RUSTC="$sys/bin/rustc" "$sys/bin/cargo" rustc --manifest-path backend/Cargo.toml --release --locked -- -L "native=$sys/lib"
case $(uname -s) in Darwin) ext=dylib;; Linux) ext=so;; *) echo "unsupported host" >&2; exit 1;; esac
backend="$root/backend/target/release/librustc_codegen_rust16.$ext"
out="$root/target/dos-sysroot"
mkdir -p "$out"
python3 scripts/core-overlay.py "$sys/lib/rustlib/src/rust/library" "$out"
"$sys/bin/rustc" --edition=2024 -Zcodegen-backend="$backend" -Zunstable-options --target backend/dos16.json --crate-name core --crate-type rlib --emit=metadata -Zalways-encode-mir --cap-lints allow "$out/library/core/src/lib.rs" -o "$out/libcore.rmeta"
"$sys/bin/rustc" --edition=2024 -Zcodegen-backend="$backend" -Zunstable-options --target backend/dos16.json --crate-name compiler_builtins --crate-type rlib --emit=metadata --extern core="$out/libcore.rmeta" --cfg 'feature="compiler-builtins"' --cap-lints allow "$out/library/compiler-builtins/compiler-builtins/src/lib.rs" -o "$out/libcompiler_builtins.rmeta"
# The backend provides its own arithmetic and DOS runtime. rustc reads the
# target core MIR and lang items from metadata-only Rust archives.
python3 "$root/scripts/metadata-archive.py" "$out/libcore.rmeta" "$out/libcompiler_builtins.rmeta"
printf '%s\n' "$backend" > "$out/backend.path"
printf '%s\n' "$sys" > "$out/sysroot.path"
