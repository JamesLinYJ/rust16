# rust16

rust16 generates real-mode x86 machine code for 8086, 80286 and 80386 targets. Its host backend plugs into upstream rustc, lowers monomorphized MIR to Intel OMF and links DOS MZ executables. The target has 16-bit near pointers. Far calls cross code segments and DOS services use segment:offset buffers.

## Build on macOS or Linux

Install the matching rustc components, then run the build from this checkout.

```sh
rustup toolchain install nightly-2026-11-05 --component rustc-dev --component rust-src
scripts/seed.sh
scripts/build-backend.sh
scripts/cross-seed.sh
RUST16_CPU=286 scripts/cargo16.sh --manifest-path examples/cargo/Cargo.toml --release
```

The backend checks rustc commit `2d20b42b883dcfc014f3c2c56081eeb6091c2179`. Set `RUST16_SYSROOT` to use an existing installation of that nightly. The build applies the 16-bit core overlay to a private copy under `target/dos-sysroot`.

## Build formats

- `scripts/rustc16.sh` produces a standalone OMF object. `scripts/cargo16.sh` builds a Cargo application and its Rust dependencies. Rust library archives retain MIR for specialization at the final link.
- Ordinary DOS programs export `main16` and link the DOS runtime. Arguments are pushed left to right. Word results return in AX and four-byte results use DX:AX. Stack and near data share a segment.
- Set `RUST16_PLATFORM=bios-loader` or `bios-kernel` for a BIOS image. The backend constructs the architecture runtime while linking. That object supplies reset, real-mode interrupt entry and BIOS I/O. The kernel supplies `main16` and `dos16_service`. The DOS project builds the FAT boot disk and relocates the loader and kernels at their load addresses.
- The DOS-hosted `RUST16.EXE` is a separate compiler for the Rust subset used by its own sources. Run `scripts/seed.sh` to build it and `LINK16.EXE` in `dist/`.

The MIR backend supports the integer, enum, aggregate, slice and call operations exercised by the DOS kernel. Its near-data and code segments are bounded. Unsupported MIR instructions still produce compiler diagnostics. `cargo test --locked` checks the OMF toolchain. The DOS repository contains the BIOS, CPU and persistent FAT integration tests.
