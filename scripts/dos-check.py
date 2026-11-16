"""Run DOS tools inside an independent, bootable DOS floppy; all guest writes use scratch."""
import argparse
import hashlib
from challenge import challenge
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

def run(*args, **kw):
    return subprocess.run(args, check=True, **kw)

def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest()

def main():
    cli = argparse.ArgumentParser()
    cli.add_argument("--boot", type=Path, required=True)
    cli.add_argument("--scratch", type=Path, required=True)
    cli.add_argument("--timeout", type=int, default=300)
    args = cli.parse_args()
    root = Path(__file__).resolve().parents[1]
    seed = root / "dist"
    with tempfile.TemporaryDirectory(dir=args.scratch) as tmp:
        tmp = Path(tmp)
        disk = tmp / "boot.img"
        shutil.copyfile(args.boot, disk)
        def copy(src, dest):
            run("mcopy", "-o", "-i", str(disk), str(src), str(dest))
        for name in ["CC.RS","LD.RS","RT.OBJ","RUST16.EXE","LINK16.EXE"]:
            copy(seed/name, "::"+name)
        copy(root/"examples/tri.rs", "::TRI.RS")
        expected_crc=challenge(tmp)
        for name in ["PROBE.RS","PROBE.DAT"]:copy(tmp/name,"::"+name)
        commands = []
        def command(line):
            commands.extend([line, "if errorlevel 1 goto fail"])
        for cpu in ["8086","8088","286","386"]:
            command(f"rust16 -m{cpu} tri.rs -o t{cpu}.obj")
            command(f"link16 t{cpu}.obj rt.obj -o t{cpu}.exe")
            command(f"t{cpu} > t{cpu}.txt")
        for gen, cc, ld in [(1,"rust16","link16"),(2,"cc1","ld1")]:
            for part in ["cc","ld"]:
                command(f"{cc} {part}.rs -o {part}{gen}.obj")
                command(f"{ld} {part}{gen}.obj rt.obj -o {part}{gen}.exe")
        command("cc2 probe.rs -o final.obj")
        command("ld2 final.obj rt.obj -o final.exe")
        command("final probe.dat > final.txt")
        batch = tmp/"AUTOEXEC.BAT"
        batch.write_bytes(("\r\n".join(["@echo off","a:"]+commands+["echo OK>done.txt","goto end",":fail","echo FAILED>done.txt",":end",""])).encode())
        copy(batch,"::AUTOEXEC.BAT")
        qemu = subprocess.Popen(["qemu-system-i386","-accel","tcg","-cpu","486","-m","16","-boot","a","-drive",f"if=floppy,format=raw,file={disk},cache=writethrough","-display","none","-monitor","none","-serial","none","-no-reboot"],stdout=subprocess.DEVNULL)
        try:
            end=time.monotonic()+args.timeout
            while time.monotonic()<end and qemu.poll() is None:
                done = subprocess.run(["mtype","-i",str(disk),"::DONE.TXT"],capture_output=True)
                if done.returncode==0:
                    if done.stdout.strip()!=b"OK": raise RuntimeError("DOS batch failed")
                    break
                time.sleep(0.4)
            else: raise RuntimeError("DOS batch did not finish")
        finally:
            qemu.terminate()
            qemu.wait()
            run("mdir","-i",str(disk),"::")
        for part, original in [("CC","RUST16.EXE"),("LD","LINK16.EXE")]:
            for generation in [1,2]:
                for suffix,expected in [("EXE",original),("OBJ",part+".OBJ")]:
                    name=f"{part}{generation}.{suffix}"
                    copy("::"+name,tmp/name)
                    if digest(tmp/name)!=digest(seed/expected): raise RuntimeError(name+" differs")
                    print(name,digest(tmp/name))
        for label in ["T8086","T8088","T286","T386","FINAL"]:
            copy("::"+label+".TXT",tmp/label)
            expected=expected_crc if label=="FINAL" else b"78"
            if (tmp/label).read_bytes()!=expected+b"\r\n": raise RuntimeError(label+" output differs")
            print(label,expected.decode())
if __name__=="__main__": main()
