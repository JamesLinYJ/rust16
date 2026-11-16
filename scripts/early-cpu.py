"""Use emu2-cpm86's independent 8088, 8086 and 80286 CPU selections."""
from pathlib import Path
import argparse
import os
import shutil
import subprocess
import tempfile

cli=argparse.ArgumentParser()
cli.add_argument("--emulator",default="emu2")
cli.add_argument("--scratch",required=True,type=Path)
args=cli.parse_args()
root=Path(__file__).resolve().parents[1]
for cpu,target in [("8088","8088"),("8086","8086"),("80286","286")]:
    with tempfile.TemporaryDirectory(dir=args.scratch) as tmp:
        d=Path(tmp)
        env=dict(os.environ, EMU2_DRIVE_C=str(d), EMU2_CWD="C:\\", EMU2_DEFAULT_DRIVE="C")
        for name in ["RUST16.EXE","LINK16.EXE","RT.OBJ"]:shutil.copyfile(root/"dist"/name,d/name)
        shutil.copyfile(root/"examples/tri.rs",d/"TRI.RS")
        def run(file,*argv):
            return subprocess.check_output([args.emulator,"-c",cpu,file,*argv],cwd=d,env=env,timeout=60)
        run("RUST16.EXE","-m"+target,"TRI.RS","-o","TRI.OBJ")
        run("LINK16.EXE","TRI.OBJ","RT.OBJ","-o","TRI.EXE")
        exe=next(p for p in d.iterdir() if p.name.upper()=="TRI.EXE")
        data=run(exe.name)
        if data.replace(b"\r",b"")!=b"78\n":raise RuntimeError((cpu,data))
        print(cpu,"native compile, link and execute: 78")
