"""Compile, link and run from a fresh RAM-backed C: using an existing DOS-RS boot image."""
import argparse
import hashlib
from challenge import challenge
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--scratch",required=True,type=Path)
    ap.add_argument("--boot",required=True,type=Path)
    ap.add_argument("--arch",required=True,choices=["i686","x86_64"])
    ap.add_argument("--proof",required=True,type=Path)
    ap.add_argument("--mir",type=Path)
    ap.add_argument("--cargo-exe",type=Path)
    ap.add_argument("--seed-dir",type=Path)
    ap.add_argument("--self-host",action="store_true")
    args=ap.parse_args()
    root=Path(__file__).resolve().parents[1]
    args.proof.mkdir(exist_ok=True,parents=True)
    with tempfile.TemporaryDirectory(dir=args.scratch) as d:
        tmp=Path(d)
        disk=tmp/"c.img"
        with disk.open("wb") as f: f.truncate(8<<20)
        subprocess.run(["mformat","-i",disk,"::"],check=True)
        files={k:root/"dist"/k for k in ["RUST16.EXE","LINK16.EXE","RT.OBJ"]}
        if args.seed_dir:files.update({k:args.seed_dir/k for k in ["RUST16.EXE","LINK16.EXE"]})
        files["TRI.RS"]=root/"examples/tri.rs"
        if args.self_host:
            expected_crc=challenge(tmp)
            files.update({k:root/"dist"/k for k in ["CC.RS","LD.RS"]})
            files.update({k:tmp/k for k in ["PROBE.RS","PROBE.DAT"]})
        if args.cargo_exe: files["CARGO.EXE"]=args.cargo_exe
        if args.mir: files["MIR.EXE"]=args.mir
        for name,p in files.items(): subprocess.run(["mcopy","-i",disk,p,"::"+name],check=True)
        ios=["-drive",f"if=none,id=bootdisk,format=raw,readonly=on,file={args.boot}","-device","virtio-blk-pci,drive=bootdisk,bootindex=1"]
        binary="qemu-system-i386" if args.arch=="i686" else "qemu-system-x86_64"
        if args.arch=="x86_64":
            qdir=Path(shutil.which(binary)).resolve().parents[1]/"share/qemu"
            variables=tmp/"vars.fd"
            shutil.copyfile(qdir/"edk2-i386-vars.fd",variables)
            ios+=["-drive",f"if=pflash,unit=0,format=raw,readonly=on,file={qdir}/edk2-x86_64-code.fd","-drive",f"if=pflash,unit=1,format=raw,file={variables}"]
        log=args.proof/(args.arch+".log")
        log.write_bytes(b"")
        cmd=[binary,"-m","768","-smp","2",*ios,"-drive",f"if=ide,index=0,format=raw,file={disk}","-nic","none","-display","none","-monitor","stdio","-serial","file:"+str(log),"-no-reboot"]
        vm=subprocess.Popen(cmd,stdin=subprocess.PIPE,stdout=subprocess.DEVNULL)
        def wait(start, marker, timeout):
            end=time.monotonic()+timeout
            while time.monotonic()<end and vm.poll() is None:
                data=log.read_bytes()[start:]
                if marker in data:return data
                time.sleep(0.1)
            raise RuntimeError(f"{args.arch}: timed out waiting for {marker!r}: {log.read_bytes()[-600:]!r}")
        def enter(line,timeout=90):
            start=log.stat().st_size
            for ch in line+"\n":
                key={" ":"spc",".":"dot","-":"minus",":":"shift-semicolon","\n":"ret"}.get(ch,ch)
                vm.stdin.write(f"sendkey {key} 25\n".encode());vm.stdin.flush();time.sleep(0.055)
            return wait(start,b"> ",timeout).replace(b"\r",b"")
        try:
            wait(0,b"> ",60)
            # Startup can have queued batch commands; settle before sending input.
            time.sleep(1)
            enter("c:")
            for cpu in ("286","386"):
                enter(f"rust16 -m{cpu} tri.rs -o t{cpu}.obj")
                enter(f"link16 t{cpu}.obj rt.obj -o t{cpu}.exe")
                output=enter(f"t{cpu}.exe")
                if b"\n78\n" not in output:raise RuntimeError(output)
            if args.mir:
                if b"\n78\n" not in enter("mir.exe"):raise RuntimeError("rustc MIR output differs")
            if args.cargo_exe:
                if b"\n78\n" not in enter("cargo.exe"):raise RuntimeError("Cargo output differs")
            if args.self_host:
                for gen,cc,ld in [(1,"rust16","link16"),(2,"cc1","ld1")]:
                    for part in ("cc","ld"):
                        enter(f"{cc} {part}.rs -o {part}{gen}.obj",1200)
                        enter(f"{ld} {part}{gen}.obj rt.obj -o {part}{gen}.exe",480)
                enter("cc2 probe.rs -o fresh.obj")
                enter("ld2 fresh.obj rt.obj -o fresh.exe")
                if b"\n"+expected_crc+b"\n" not in enter("fresh.exe probe.dat"):raise RuntimeError("second-generation CRC differs")
                print(args.arch,"CRC16",expected_crc.decode(),flush=True)
        finally:
            if vm.poll() is None:
                vm.stdin.write(b"quit\n");vm.stdin.flush();vm.wait(timeout=10)
        if args.self_host:
            for part,exe in [("CC","RUST16.EXE"),("LD","LINK16.EXE")]:
                for gen in (1,2):
                    for suffix,expected in [("OBJ",part+".OBJ"),("EXE",exe)]:
                        name=f"{part}{gen}.{suffix}"
                        data=subprocess.check_output(["mtype","-i",disk,"::"+name])
                        if data!=(root/"dist"/expected).read_bytes():raise RuntimeError(name+" differs from seed")
                        print(args.arch,name,hashlib.sha256(data).hexdigest(),flush=True)
        for cpu in ("286","386"):
            out=args.proof/(args.arch+"-"+cpu+".exe")
            subprocess.run(["mcopy","-o","-i",disk,"::T"+cpu+".EXE",out],check=True)
            print(args.arch,cpu,out.stat().st_size,"bytes; guest output 78")
if __name__=="__main__":main()
