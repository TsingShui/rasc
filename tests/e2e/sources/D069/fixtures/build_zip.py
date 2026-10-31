#!/usr/bin/env python3
"""D069 case-local ZIP builder: four independent rejection scenarios.

Writes four separate single-entry ZIPs under .cache/e2e/D069/, one per
invocation, so each scenario is independent:

    bad-crc.zip        bad.bin        method 0 (stored)
                       payload [0,255,65,0,128,10]; the local and central CRC
                       agree with each other but equal the correct CRC xor 1.

    over-budget.zip    large.bin      method 8 (deflate)
                       1024 bytes of 0x41, correct CRC and sizes.

    unsupported.zip    unsupported.bin method 12 (bzip2)
                       a real bzip2 stream of [0,255,65,0,128,10], correct CRC,
                       version-needed 46, general-purpose bit0 = 0.

    encrypted.zip      encrypted.bin  method 0 (stored), general-purpose bit0 = 1,
                       bit3 = bit6 = 0, version-needed 20, a real traditional
                       ZipCrypto stream: 12-byte encryption header (plaintext
                       bytes 0..10 then the high byte of the plaintext CRC)
                       followed by the 6 ciphertext bytes of
                       [0,255,65,0,128,10]. compressed_size = 18 includes the
                       header. The fixture password is "fixture".

Every record is verified after writing: bzip2 and deflate payloads with the
standard-library decompressors, the ZipCrypto stream with CPython's own
zipfile ZipCrypto decryption path (independent of this builder), and the CRC
mismatch by recomputing the true CRC.

Standard library only, no network, no subprocess.
Usage: python3 build_zip.py [output-directory]
"""

from __future__ import annotations

import bz2
import hashlib
import struct
import sys
import zipfile
import zlib
from pathlib import Path

DOS_TIME = 0
DOS_DATE = (1 << 5) | 1  # 1980-01-01, the earliest legal MS-DOS date
PAYLOAD = bytes([0, 255, 65, 0, 128, 10])
PASSWORD = b"fixture"

CRC_KEY_0 = 0x12345678
CRC_KEY_1 = 0x23456789
CRC_KEY_2 = 0x34567890


def deflate_raw(data: bytes) -> bytes:
    compressor = zlib.compressobj(9, zlib.DEFLATED, -15)
    return compressor.compress(data) + compressor.flush()


def build_crc32_table() -> list[int]:
    table = []
    for index in range(256):
        value = index
        for _ in range(8):
            if value & 1:
                value = 0xedb88320 ^ (value >> 1)
            else:
                value >>= 1
        table.append(value)
    return table


CRC32_TABLE = build_crc32_table()


def crc32_step(crc: int, byte: int) -> int:
    """The raw CRC-32 table step ZipCrypto uses, without the pre/post complement.

    zlib.crc32() complements the incoming and outgoing register, so it is not
    the same function; using it here would derive the wrong keys.
    """
    return ((crc >> 8) ^ CRC32_TABLE[(crc ^ byte) & 0xff]) & 0xffffffff


def zipcrypto_encrypt(plain: bytes, password: bytes) -> bytes:
    keys = [CRC_KEY_0, CRC_KEY_1, CRC_KEY_2]

    def update(byte: int) -> None:
        keys[0] = crc32_step(keys[0], byte)
        keys[1] = (keys[1] + (keys[0] & 0xff)) * 134775813 + 1
        keys[1] &= 0xffffffff
        keys[2] = crc32_step(keys[2], keys[1] >> 24)

    def keystream_byte() -> int:
        temp = (keys[2] | 2) & 0xffff
        return ((temp * (temp ^ 1)) >> 8) & 0xff

    for byte in password:
        update(byte)
    out = bytearray()
    for byte in plain:
        cipher = byte ^ keystream_byte()
        out.append(cipher)
        update(byte)
    return bytes(out)


def write_single_entry_zip(
    path: Path,
    name: str,
    method: int,
    stored: bytes,
    uncompressed: bytes,
    crc: int,
    flags: int = 0,
    version_needed: int = 20,
) -> None:
    name_bytes = name.encode("ascii")
    local = struct.pack(
        "<IHHHHHIIIHH",
        0x04034b50,
        version_needed,
        flags,
        method,
        DOS_TIME,
        DOS_DATE,
        crc,
        len(stored),
        len(uncompressed),
        len(name_bytes),
        0,
    ) + name_bytes
    central = struct.pack(
        "<IHHHHHHIIIHHHHHII",
        0x02014b50,
        20,
        version_needed,
        flags,
        method,
        DOS_TIME,
        DOS_DATE,
        crc,
        len(stored),
        len(uncompressed),
        len(name_bytes),
        0,
        0,
        0,
        0,
        0,
        0,
    ) + name_bytes
    eocd = struct.pack(
        "<IHHHHIIH",
        0x06054b50,
        0,
        0,
        1,
        1,
        len(central),
        len(local) + len(stored),
        0,
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(local + stored + central + eocd)


def build_bad_crc(path: Path) -> dict:
    correct = zlib.crc32(PAYLOAD) & 0xffffffff
    recorded = correct ^ 1
    write_single_entry_zip(path, "bad.bin", 0, PAYLOAD, PAYLOAD, recorded)
    # zipfile.read() rejects the mismatched CRC, which is exactly the input fault;
    # the stored bytes are read directly from the local record instead.
    with zipfile.ZipFile(path) as archive:
        info = archive.infolist()[0]
        assert info.CRC == recorded
        assert info.compress_type == 0
        assert info.compress_size == len(PAYLOAD)
        with open(path, "rb") as handle:
            handle.seek(info.header_offset)
            local_header = handle.read(30)
            assert struct.unpack("<I", local_header[:4])[0] == 0x04034b50
            name_length, extra_length = struct.unpack("<HH", local_header[26:30])
            handle.seek(info.header_offset + 30 + name_length + extra_length)
            stored_bytes = handle.read(info.compress_size)
    assert stored_bytes == PAYLOAD  # the bytes themselves are intact
    assert (zlib.crc32(stored_bytes) & 0xffffffff) == correct != recorded
    return {
        "name": "bad.bin",
        "method": 0,
        "recordedCrc": recorded,
        "correctCrc": correct,
        "crcXorDelta": recorded ^ correct,
        "payload": list(PAYLOAD),
        "verification": "recomputed CRC differs from both the local and the central recorded CRC, which agree with each other",
    }


def build_over_budget(path: Path) -> dict:
    payload = b"\x41" * 1024
    stored = deflate_raw(payload)
    write_single_entry_zip(path, "large.bin", 8, stored, payload,
                           zlib.crc32(payload) & 0xffffffff)
    with zipfile.ZipFile(path) as archive:
        info = archive.infolist()[0]
        assert archive.read(info) == payload
        assert info.file_size == 1024
    return {
        "name": "large.bin",
        "method": 8,
        "uncompressedSize": 1024,
        "compressedSize": len(stored),
        "crc": zlib.crc32(payload) & 0xffffffff,
        "inflateLimitBytes": 128,
        "payloadPattern": "0x41 repeated 1024 times",
        "verification": "the standard-library deflate reader returned exactly 1024 bytes of 0x41",
    }


def build_unsupported(path: Path) -> dict:
    stored = bz2.compress(PAYLOAD, compresslevel=9)
    write_single_entry_zip(path, "unsupported.bin", 12, stored, PAYLOAD,
                           zlib.crc32(PAYLOAD) & 0xffffffff, version_needed=46)
    with zipfile.ZipFile(path) as archive:
        info = archive.infolist()[0]
        assert info.compress_type == zipfile.ZIP_BZIP2 == 12
        assert info.extract_version == 46
        assert not info.flag_bits & 0x1
        assert archive.read(info) == PAYLOAD
    assert bz2.decompress(stored) == PAYLOAD
    return {
        "name": "unsupported.bin",
        "method": 12,
        "versionNeeded": 46,
        "flagBits": 0,
        "streamHeader": list(stored[:4]),
        "compressedSize": len(stored),
        "crc": zlib.crc32(PAYLOAD) & 0xffffffff,
        "verification": "the real bzip2 stream decompresses to [0,255,65,0,128,10] with the standard-library bz2 decompressor and through zipfile method 12",
    }


def build_encrypted(path: Path) -> dict:
    correct = zlib.crc32(PAYLOAD) & 0xffffffff
    header_plain = bytes(range(11)) + bytes([(correct >> 24) & 0xff])
    cipher = zipcrypto_encrypt(header_plain + PAYLOAD, PASSWORD)
    write_single_entry_zip(
        path,
        "encrypted.bin",
        0,
        cipher,
        PAYLOAD,
        correct,
        flags=0x0001,
        version_needed=20,
    )
    with zipfile.ZipFile(path) as archive:
        info = archive.infolist()[0]
        assert info.flag_bits & 0x1
        assert not info.flag_bits & 0x8
        assert not info.flag_bits & 0x40
        assert info.compress_size == 18 == info.file_size + 12
        assert info.extract_version == 20
        # CPython's own ZipCrypto path, independent of this builder.
        assert archive.read(info, pwd=PASSWORD) == PAYLOAD
    return {
        "name": "encrypted.bin",
        "method": 0,
        "flagBits": "0x0001 (bit0 only; bit3 and bit6 clear)",
        "versionNeeded": 20,
        "password": "fixture",
        "headerPlaintext": list(header_plain),
        "headerCrcHighByte": (correct >> 24) & 0xff,
        "payloadPlaintext": list(PAYLOAD),
        "compressedSize": 18,
        "uncompressedSize": 6,
        "crc": correct,
        "verification": "zipfile.read(info, pwd=b'fixture') decrypted the 12-byte header and returned exactly [0,255,65,0,128,10]; compressed_size 18 = 12-byte header + 6 payload bytes",
    }


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D069")
    out_dir.mkdir(parents=True, exist_ok=True)

    scenarios = [
        ("bad-crc", build_bad_crc, out_dir / "bad-crc.zip"),
        ("inflate-budget", build_over_budget, out_dir / "over-budget.zip"),
        ("unsupported-compression", build_unsupported, out_dir / "unsupported.zip"),
        ("encrypted-entry", build_encrypted, out_dir / "encrypted.zip"),
    ]
    for invocation, builder, path in scenarios:
        facts = builder(path)
        blob = path.read_bytes()
        facts["path"] = str(path)
        facts["fileSize"] = len(blob)
        facts["sha256"] = hashlib.sha256(blob).hexdigest()
        print(f"{invocation}: {facts}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
