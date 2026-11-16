"""Run matching DOS compiler, linker and fresh output under CPU-specific DOSBox-X cores."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

def main():
    cli=argparse.ArgumentParser()
    cli.add_argument("--emulator",default="dosbox-x")
    cli.add_argument("--scratch",required=True,type=Path)
    args=cli.parse_args()
    root=Path(__file__).resolve().parents[1]
    for target, cpu in [("8086","8086"),("8088","8086_prefetch"),("286","286"),("386","386")]:
        with tempfile.TemporaryDirectory(dir=args.scratch) as temp:
            d=Path(temp)
            for name in ["RUST16.EXE","LINK16.EXE","RT.OBJ"]: shutil.copyfile(root/"dist"/name,d/name)
            shutil.copyfile(root/"examples/tri.rs",d/"TRI.RS")
            config=d/"machine.conf"
            config.write_text(f"""[sdl]
output=surface
[dosbox]
show start banner=false
[cpu]
core=normal
cputype={cpu}
cycles=fixed 20000
[mixer]
nosound=true
[autoexec]
@echo off
mount c "{d}"
c:
rust16 -m{target} tri.rs -o tri.obj
if errorlevel 1 goto done
link16 tri.obj rt.obj -o tri.exe
if errorlevel 1 goto done
tri > actual.txt
:done
exit
""")
            env=os.environ.copy()
            env.update(SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
            subprocess.run([args.emulator,"-conf",str(config),"-nogui","-nomenu"],env=env,cwd=d,check=True,timeout=90,stdout=subprocess.DEVNULL)
            txt=next((p for p in d.iterdir() if p.name.upper()=="ACTUAL.TXT"),None)
            if txt is None or txt.read_bytes()!=b"78\r\n":raise RuntimeError(f"{cpu}: incorrect result")
            print(f"{target} -> {cpu}: 78",flush=True)
if __name__=="__main__":main()
