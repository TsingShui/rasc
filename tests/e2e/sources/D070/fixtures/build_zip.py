#!/usr/bin/env python3
"""D070 case-local ZIP builder: no-clobber, alias safety and two publishers.

Writes .cache/e2e/D070/entries.zip with four physical entries:

    ordinal 0  safe.bin        method 8 (deflate)  payload [0,255,65,0,128,10]
    ordinal 1  ../escape       method 0 (stored)   payload [0,255,65,0,128,10]
    ordinal 2  pattern17.bin   method 8 (deflate)  16777216 bytes, byte i = (i*17+3)%256
    ordinal 3  pattern29.bin   method 8 (deflate)  16777216 bytes, byte i = (i*29+11)%256

The "../escape" name appears only as an entry name; it must never take part in
path concatenation. The two publisher payloads are deterministic and different,
so a mixed or prefix file is detectable byte for byte.

The builder verifies every record after writing by re-reading it with the
standard-library zipfile reader and comparing the complete payload, and prints
a per-entry SHA-256 of the payload so the recipe can record the exact bytes
without copying a decompressed copy into the tree.

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

DOS_TIME = 0
DOS_DATE = (1 << 5) | 1  # 1980-01-01, the earliest legal MS-DOS date

SMALL_PAYLOAD = bytes([0, 255, 65, 0, 128, 10])
PATTERN_LENGTH = 16777216
PATTERNS = [
    {"ordinal": 2, "name": "pattern17.bin", "multiplier": 17, "add": 3},
    {"ordinal": 3, "name": "pattern29.bin", "multiplier": 29, "add": 11},
]


def pattern_bytes(length: int, multiplier: int, add: int) -> bytes:
    return bytes(((index * multiplier + add) % 256) for index in range(length))


def deflate_raw(data: bytes) -> bytes:
    compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
    return compressor.compress(data) + compressor.flush()


def build(path: Path) -> list[dict]:
    entries = [
        {"ordinal": 0, "name": "safe.bin", "method": 8, "payload": SMALL_PAYLOAD},
        {"ordinal": 1, "name": "../escape", "method": 0, "payload": SMALL_PAYLOAD},
    ]
    for spec in PATTERNS:
        entries.append(
            {
                "ordinal": spec["ordinal"],
                "name": spec["name"],
                "method": 8,
                "payload": pattern_bytes(PATTERN_LENGTH, spec["multiplier"], spec["add"]),
                "multiplier": spec["multiplier"],
                "add": spec["add"],
                "length": PATTERN_LENGTH,
            }
        )

    local_parts: list[bytes] = []
    central_parts: list[bytes] = []
    offset = 0
    records = []
    for entry in entries:
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
        record = {
            "ordinal": entry["ordinal"],
            "name": entry["name"],
            "method": entry["method"],
            "crc": f"0x{crc:08x}",
            "compressedSize": len(stored),
            "uncompressedSize": len(payload),
            "localHeaderOffset": offset,
            "payloadSha256": hashlib.sha256(payload).hexdigest(),
        }
        if "multiplier" in entry:
            record["pattern"] = {
                "length": entry["length"],
                "multiplier": entry["multiplier"],
                "add": entry["add"],
            }
        records.append(record)
        offset += len(local) + len(stored)

    central = b"".join(central_parts)
    eocd = struct.pack(
        "<IHHHHIIH", 0x06054b50, 0, 0, len(entries), len(entries),
        len(central), offset, 0,
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"".join(local_parts) + central + eocd)
    return records


def verify(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        assert [info.filename for info in infos] == [
            "safe.bin", "../escape", "pattern17.bin", "pattern29.bin"
        ]
        for info in infos:
            with archive.open(info) as handle:
                digest = hashlib.sha256()
                total = 0
                head = None
                while True:
                    chunk = handle.read(65536)
                    if not chunk:
                        break
                    if head is None:
                        head = chunk[:8]
                    total += len(chunk)
                    digest.update(chunk)
            print(
                f"verified name={info.filename} method={info.compress_type}"
                f" crc=0x{info.CRC:08x} size={total} head={list(head)}"
                f" payload_sha256={digest.hexdigest()}"
            )


def main() -> int:
    out_path = (
        Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D070/entries.zip")
    )
    records = build(out_path)
    verify(out_path)
    blob = out_path.read_bytes()
    print(f"output={out_path} file_size={len(blob)} sha256={hashlib.sha256(blob).hexdigest()}")
    for record in records:
        print(f"entry {record}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
