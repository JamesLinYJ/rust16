"""Apply the DOS stdarch patch to a private copy of the pinned official sources."""
from pathlib import Path
import shutil
import subprocess
import sys
source, dest=map(Path,sys.argv[1:])
patch=Path(__file__).resolve().parents[1]/"backend/rust-src.patch"
root=dest/"library"
root.mkdir(exist_ok=True)
for child in source.iterdir():
    target=root/child.name
    if child.name in ("core","stdarch","portable-simd"):
        if target.is_symlink(): target.unlink()
        shutil.copytree(child,target,dirs_exist_ok=True)
    elif not target.exists(): target.symlink_to(child.resolve(),target_is_directory=child.is_dir())
subprocess.run(["patch","-p1","-i",str(patch)],cwd=dest,check=True)
