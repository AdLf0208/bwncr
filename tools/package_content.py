#!/usr/bin/env python3
"""Package `content/` as a deterministic `.hmod` (a content-mod zip).

The archive holds `horizon-mod.toml` and every asset/data file, with fixed
timestamps and modes, so the same content gives the same bytes and SHA-256.
The `.hmod` itself is excluded from the archive.
"""
import hashlib
import json
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
CONTENT = ROOT / 'content'
OUT = CONTENT / 'bwncr.hmod'


def package():
    files = sorted(
        path for path in CONTENT.rglob('*') if path.is_file() and path != OUT
    )
    with zipfile.ZipFile(OUT, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in files:
            info = zipfile.ZipInfo(path.relative_to(CONTENT).as_posix(), (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, path.read_bytes())
    print(json.dumps(dict(
        file=str(OUT),
        size=OUT.stat().st_size,
        sha256=hashlib.sha256(OUT.read_bytes()).hexdigest(),
    ), indent=2))


if __name__ == '__main__':
    package()
