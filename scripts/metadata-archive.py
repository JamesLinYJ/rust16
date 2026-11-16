"""Write portable System V ar archives containing rustc's metadata member."""
from pathlib import Path
import sys

def pack(path):
    data=path.read_bytes()
    fields=[("lib.rmeta/",16),("0",12),("0",6),("0",6),("644",8),(str(len(data)),10)]
    header="".join(text.ljust(width) for text,width in fields)
    with path.with_suffix(".rlib").open("wb") as archive:
        archive.write(b"!<arch>\n")
        archive.write(header.encode("ascii")+b"`\n")
        archive.write(data)
        if len(data)%2:archive.write(b"\n")

for arg in sys.argv[1:]:pack(Path(arg))
