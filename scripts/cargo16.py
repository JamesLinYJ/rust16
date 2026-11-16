"""Build a DOS executable through Cargo with target-specific rustc arguments."""
import hashlib
import json
import os
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
out = root / "target/dos-sysroot"
toolchain = Path((out / "sysroot.path").read_text().strip())
plugin = Path((out / "backend.path").read_text().strip())
platform = os.environ.get("RUST16_PLATFORM", "dos")
if platform not in ("dos","bios-loader","bios-kernel"):
    sys.exit("RUST16_PLATFORM must be dos, bios-loader or bios-kernel")
runtime = Path(os.environ.get("RUST16_RUNTIME", root / "dist/RT.OBJ")).resolve()
linker = root / "target/release/link16"
cpu = os.environ.get("RUST16_CPU", "8086")

# Cargo normally fingerprints flags, but a backend rebuilt at the same path
# would otherwise leave the previous target executable in its cache.
fingerprint = hashlib.sha256()
fingerprint.update(platform.encode())
artifacts = [plugin, linker, out/"libcore.rlib", out/"libcompiler_builtins.rlib"]
if platform == "dos":
    artifacts.append(runtime)
for artifact in artifacts:
    fingerprint.update(artifact.read_bytes())
flags = ["-Zalways-encode-mir", f"-Ldependency={out}", "-Zunstable-options", f"-Zcodegen-backend={plugin}", f"-Ctarget-cpu={cpu}", f'--cfg=rust16_cpu="{cpu}"',
         "-Coverflow-checks=off", f'--cfg=rust16_build="{fingerprint.hexdigest()}"',
         f"--extern=core={out}/libcore.rlib", f"--extern=compiler_builtins={out}/libcompiler_builtins.rlib"]
env = dict(os.environ, RUSTC=str(toolchain/"bin/rustc"), RUST16_LINKER=str(linker), RUST16_RUNTIME=str(runtime))
cargo = str(toolchain/"bin/cargo")
# Cargo looks for .cargo/config.toml in its working directory. The DOS
# backend supplies its own sysroot, so do not inherit unrelated build-std
# settings from the UEFI kernel. Resolve caller-relative paths first.
args = sys.argv[1:]
has_manifest = False
for index, arg in enumerate(args):
    for option in ("--manifest-path", "--target-dir"):
        if arg == option and index + 1 < len(args):
            args[index + 1] = str(Path(args[index + 1]).resolve())
        elif arg.startswith(option + "="):
            args[index] = option + "=" + str(Path(arg.split("=",1)[1]).resolve())
        else:
            continue
        has_manifest |= option == "--manifest-path"
if not has_manifest:
    manifest = next((parent/"Cargo.toml" for parent in (Path.cwd(), *Path.cwd().parents)
                     if (parent/"Cargo.toml").is_file()), None)
    if manifest is None:
        sys.exit("no Cargo.toml in this directory or its parents")
    args.extend(["--manifest-path",str(manifest)])
env["RUST16_PLATFORM"] = platform
os.chdir(root)
os.execve(cargo,[cargo,"build","-Zjson-target-spec","--config","target.dos16.rustflags="+json.dumps(flags),
                "--target",str(root/"backend/dos16.json"),*args],env)
