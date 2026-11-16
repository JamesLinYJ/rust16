"""Generate an unseen DOS input; compute its expected CRC independently on the host."""
import binascii
from pathlib import Path
import secrets

def challenge(directory):
    payload=secrets.token_bytes(257)
    directory=Path(directory)
    (directory/"PROBE.DAT").write_bytes(payload)
    root=Path(__file__).resolve().parents[1]
    initial=secrets.randbelow(65536)
    template=(root/"examples/crc.rs").read_bytes()
    source=template.replace(b"let mut crc: u16 = 65535;",f"let mut crc: u16 = {initial};".encode(),1)
    if b"let mut crc: u16 = 65535;" not in template:raise ValueError("CRC template initializer missing")
    (directory/"PROBE.RS").write_bytes(source)
    return str(binascii.crc_hqx(payload,initial)).encode("ascii")
