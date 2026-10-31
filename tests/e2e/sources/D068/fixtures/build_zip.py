#!/usr/bin/env python3
"""D068 case-local ZIP builder: stored/deflated payloads and duplicate names.

Writes .cache/e2e/D068/entries.zip with four physical entries, in this order:

    ordinal 0  stored.bin    method 0 (stored)    payload [0,255,65,0,128,10]
    ordinal 1  deflated.bin  method 8 (deflate)   payload [0,255,65,0,128,10]
    ordinal 2  dup.bin       method 0 (stored)    payload [1]
    ordinal 3  dup.bin       method 0 (stored)    payload [2]

The two dup.bin records have the same name and different payloads, so a
selection by name alone is ambiguous. All CRC/size/local/central structures are
legal and every record is independently decodable.

The builder then re-reads its own output with the Python standard-library
zipfile module, checks each payload byte for byte and reports the entry
ordinals, so the input facts are verified by an independent reader before the
recipe records them.

Standard library only, no network, no subprocess.
Usage: python3 build_zip.py [output-path]
"""

from __future__ import annotations

import hashlib
import struct
import sys
import zipfile
import zlib
from pathlib import Path

ENTRY_PAYLOAD = bytes([0, 255, 65, 0, 128, 10])

ENTRIES = [
    {"ordinal": 0, "name": "stored.bin", "method": 0, "payload": ENTRY_PAYLOAD},
    {"ordinal": 1, "name": "deflated.bin", "method": 8, "payload": ENTRY_PAYLOAD},
    {"ordinal": 2, "name": "dup.bin", "method": 0, "payload": bytes([1])},
    {"ordinal": 3, "name": "dup.bin", "method": 0, "payload": bytes([2])},
]

DOS_TIME = 0
DOS_DATE = (1 << 5) | 1  # 1980-01-01, the earliest legal MS-DOS date


def deflate_raw(data: bytes) -> bytes:
    compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
    return compressor.compress(data) + compressor.flush()


def build(path: Path) -> list[dict]:
    local_parts: list[bytes] = []
    central_parts: list[bytes] = []
    offset = 0
    records = []
    for entry in ENTRIES:
        payload = entry["payload"]
        crc = zlib.crc32(payload) & 0xffffffff
        stored = payload if entry["method"] == 0 else deflate_raw(payload)
        name_bytes = entry["name"].encode("ascii")
        local = struct.pack(
            "<IHHHHHIIIHH",
            0x04034b50,
            20,
            0,
            entry["method"],
            DOS_TIME,
            DOS_DATE,
            crc,
            len(stored),
            len(payload),
            len(name_bytes),
            0,
        ) + name_bytes
        local_parts.append(local + stored)
        central_parts.append(
            struct.pack(
                "<IHHHHHHIIIHHHHHII",
                0x02014b50,
                20,
                20,
                0,
                entry["method"],
                DOS_TIME,
                DOS_DATE,
                crc,
                len(stored),
                len(payload),
                len(name_bytes),
                0,
                0,
                0,
                0,
                0,
                offset,
            )
            + name_bytes
        )
        records.append(
            {
                "ordinal": entry["ordinal"],
                "name": entry["name"],
                "method": entry["method"],
                "crc": crc,
                "compressedSize": len(stored),
                "uncompressedSize": len(payload),
                "localHeaderOffset": offset,
            }
        )
        offset += len(local) + len(stored)

    central = b"".join(central_parts)
    eocd = struct.pack(
        "<IHHHHIIH",
        0x06054b50,
        0,
        0,
        len(ENTRIES),
        len(ENTRIES),
        len(central),
        offset,
        0,
    )
    blob = b"".join(local_parts) + central + eocd
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(blob)
    for record in records:
        record["sha256"] = hashlib.sha256(blob).hexdigest()
    return records


def verify(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert names == ["stored.bin", "deflated.bin", "dup.bin", "dup.bin"], names
        infos = archive.infolist()
        for info, entry in zip(infos, ENTRIES):
            assert info.compress_type == entry["method"], info.compress_type
            with archive.open(info) as handle:
                data = handle.read()
            assert data == entry["payload"], (info.filename, data)
            print(
                f"verified ordinal={entry['ordinal']} name={info.filename}"
                f" method={info.compress_type} crc=0x{info.CRC:08x}"
                f" payload={list(data)}"
            )


def main() -> int:
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D068/entries.zip")
    records = build(out_path)
    verify(out_path)
    size = out_path.stat().st_size
    digest = hashlib.sha256(out_path.read_bytes()).hexdigest()
    print(f"output={out_path} file_size={size} sha256={digest}")
    for record in records:
        print(
            f"entry ordinal={record['ordinal']} name={record['name']}"
            f" method={record['method']} crc=0x{record['crc']:08x}"
            f" comp={record['compressedSize']} uncomp={record['uncompressedSize']}"
            f" local_off={record['localHeaderOffset']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
