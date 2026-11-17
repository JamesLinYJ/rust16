"""Link a three-bank MZ in a booted DOS, compare all bytes, then run its third-bank Rust code."""
import argparse
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import time


def record(kind, payload):
    data = bytes([kind]) + struct.pack("<H", len(payload) + 1) + payload
    return data + bytes([-sum(data) % 256])


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--boot", type=Path, required=True)
    cli.add_argument("--scratch", type=Path, required=True)
    args = cli.parse_args()
    root = Path(__file__).resolve().parents[1]
    with tempfile.TemporaryDirectory(dir=args.scratch) as tmp:
        work = Path(tmp)
        boot = work / "a.img"
        disk = work / "b.img"
        shutil.copyfile(args.boot, boot)
        subprocess.run(["mformat", "-C", "-f", "1440", "-i", disk, "::"], check=True)

        def copy(volume, path, data):
            subprocess.run(["mcopy", "-o", "-i", volume, "-", "::" + path], input=data, check=True)

        def read(name):
            return subprocess.run(["mcopy", "-i", disk, "::" + name, "-"], capture_output=True)

        # A legal 60 KiB OMF data segment, including bytes on every bank boundary.
        pad = record(0x98, b"\x68" + struct.pack("<H", 61440) + b"\x01\x01\x01")
        for offset in range(0, 61440, 1024):
            pad += record(0xa0, b"\x01" + struct.pack("<H", offset) + bytes([offset // 1024]) * 1024)
        pad += record(0x8a, b"\x00")
        (work / "PAD.OBJ").write_bytes(pad)
        (work / "RT.OBJ").write_bytes((root / "dist/RT.OBJ").read_bytes())
        subprocess.run([root / "target/release/rust16", root / "examples/tri.rs", "-o", work / "TRI.OBJ"], check=True)
        objects = ["RT.OBJ", "PAD.OBJ", "PAD.OBJ", "TRI.OBJ"]
        subprocess.run([root / "target/release/link16", *objects, "-o", "HOST.EXE"], cwd=work, check=True)
        copy(disk, "LINK16.EXE", (root / "dist/LINK16.EXE").read_bytes())
        for name in dict.fromkeys(objects):
            copy(disk, name, (work / name).read_bytes())
        batch = ["@echo off", "B:", "LINK16 " + " ".join(objects) + " -o GUEST.EXE",
                 "if errorlevel 1 goto fail", "GUEST >RESULT.TXT", "if errorlevel 1 goto fail",
                 "echo OK>DONE", "goto end", ":fail", "echo ERROR>DONE", ":end"]
        copy(boot, "CONFIG.SYS", b"FILES=16\r\n")
        copy(boot, "AUTOEXEC.BAT", ("\r\n".join(batch) + "\r\n").encode())
        with subprocess.Popen(["qemu-system-i386", "-machine", "pc,accel=tcg", "-cpu", "486", "-m", "16",
                "-display", "none", "-serial", "none", "-monitor", "stdio", "-no-reboot", "-boot", "a",
                "-drive", f"if=floppy,index=0,format=raw,file={boot}",
                "-drive", f"if=floppy,index=1,format=raw,file={disk}"],
                stdin=subprocess.PIPE, stdout=subprocess.DEVNULL) as vm:
            try:
                for _ in range(300):
                    done = read("DONE")
                    if done.returncode == 0:
                        if done.stdout.strip() != b"OK":
                            raise RuntimeError("DOS linker or third-bank program returned failure")
                        break
                    time.sleep(0.2)
                else:
                    raise RuntimeError("DOS did not finish the third-bank test")
            finally:
                if vm.poll() is None:
                    vm.stdin.write(b"quit\n")
                    vm.stdin.flush()
                    vm.wait(10)
        image = read("GUEST.EXE").stdout
        if image != (work / "HOST.EXE").read_bytes() or read("RESULT.TXT").stdout != b"78\r\n":
            raise AssertionError("DOS output differed from host link or the Rust computation failed")
        print(f"PASS DOS and host link identical {len(image)} bytes; third-bank Rust program printed 78")


if __name__ == "__main__":
    main()
