#!/usr/bin/env python3
"""D059 case-local DEX builder: unknown opcode width is not guessed as nop.

Builds a 035 owner Lfixtures/Case; with the canonical Object constructor and
public static f()V whose original code is exactly one return-void, with
registers_size/ins_size/outs_size/tries_size all zero and insns_size=1.

The single mutation changes only the low opcode byte of code unit 0 from 0x0e
(return-void, 10x) to 0x3e, an unused value in the 035 opcode space. The high
byte stays 0x00, so the code range, declaration, version and (empty) reference
tables are unchanged. No standard instruction width is assigned to 0x3e.

Outputs go to .cache/e2e/D059/.  Standard library only, no network, no
subprocess.  Usage: python3 build_dex.py [output-directory]
"""

from __future__ import annotations

import hashlib
import struct
import sys
import zlib
from pathlib import Path

ACC_PUBLIC = 0x0001
ACC_STATIC = 0x0009
ACC_CONSTRUCTOR = 0x10001

TYPE_HEADER_ITEM = 0x0000
TYPE_STRING_ID_ITEM = 0x0001
TYPE_TYPE_ID_ITEM = 0x0002
TYPE_PROTO_ID_ITEM = 0x0003
TYPE_METHOD_ID_ITEM = 0x0005
TYPE_CLASS_DEF_ITEM = 0x0006
TYPE_MAP_LIST = 0x1000
TYPE_CLASS_DATA_ITEM = 0x2000
TYPE_CODE_ITEM = 0x2001
TYPE_STRING_DATA_ITEM = 0x2002

ORIGINAL_CODE_UNIT_0 = 0x000e
MUTATED_CODE_UNIT_0 = 0x003e


def uleb(value: int) -> bytes:
    if value < 0:
        raise ValueError("uleb needs a non-negative value")
    out = bytearray()
    while True:
        byte = value & 0x7f
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def utf16_units(text: str) -> list[int]:
    data = text.encode("utf-16-le")
    return [int.from_bytes(data[i : i + 2], "little") for i in range(0, len(data), 2)]


def mutf8(text: str) -> bytes:
    out = bytearray()
    for unit in utf16_units(text):
        if 0x0001 <= unit <= 0x007f:
            out.append(unit)
        elif unit <= 0x07ff:
            out.append(0xc0 | (unit >> 6))
            out.append(0x80 | (unit & 0x3f))
        else:
            out.append(0xe0 | (unit >> 12))
            out.append(0x80 | ((unit >> 6) & 0x3f))
            out.append(0x80 | (unit & 0x3f))
    return bytes(out)


def align(blob: bytearray, base: int, alignment: int) -> int:
    while (base + len(blob)) % alignment:
        blob.append(0)
    return base + len(blob)


def code_item(registers: int, ins: int, outs: int, units: list[int]) -> bytearray:
    blob = bytearray(struct.pack("<HHHHII", registers, ins, outs, 0, 0, len(units)))
    for unit in units:
        blob += struct.pack("<H", unit)
    assert len(blob) == 16 + 2 * len(units)
    return blob


def build(f_code_unit_0: int) -> tuple[bytearray, dict]:
    strings = ["()V", "<init>", "Case.smali", "Lfixtures/Case;",
               "Ljava/lang/Object;", "V", "f"]
    string_idx = {value: index for index, value in enumerate(strings)}
    types = ["Lfixtures/Case;", "Ljava/lang/Object;", "V"]
    type_idx = {value: index for index, value in enumerate(types)}
    protos = [("V", "V")]
    # method_ids sort by class_idx, then name_idx, then proto_idx. Case (type 0)
    # precedes Object (type 1); within Case, name "<init>" (string 1) precedes
    # "f" (string 6).
    methods = [
        ("Lfixtures/Case;", "<init>", 0),   # 0
        ("Lfixtures/Case;", "f", 0),        # 1
        ("Ljava/lang/Object;", "<init>", 0),  # 2
    ]
    object_ctor_method_idx = 2

    header_size = 0x70
    string_ids_off = header_size
    type_ids_off = string_ids_off + 4 * len(strings)
    proto_ids_off = type_ids_off + 4 * len(types)
    method_ids_off = proto_ids_off + 12 * len(protos)
    class_defs_off = method_ids_off + 8 * len(methods)
    data_off = class_defs_off + 32

    data = bytearray()
    string_data_offsets = []
    for value in strings:
        string_data_offsets.append(data_off + len(data))
        data += uleb(len(utf16_units(value)))
        data += mutf8(value)
        data.append(0x00)

    ctor_code_off = align(data, data_off, 4)
    # invoke-direct {p0}, Ljava/lang/Object;-><init>()V: 35c with count 1, G=v0.
    data += code_item(1, 1, 1, [0x1070, object_ctor_method_idx, 0x0000, 0x000e])

    f_code_off = align(data, data_off, 4)
    data += code_item(0, 0, 0, [f_code_unit_0])

    class_data_off = data_off + len(data)
    class_data = bytearray(uleb(0) + uleb(0) + uleb(2) + uleb(0))
    class_data += uleb(0) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_code_off)
    class_data += uleb(1) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(f_code_off)
    data += class_data

    map_off = align(data, data_off, 4)
    map_entries = [
        (TYPE_HEADER_ITEM, 1, 0),
        (TYPE_STRING_ID_ITEM, len(strings), string_ids_off),
        (TYPE_TYPE_ID_ITEM, len(types), type_ids_off),
        (TYPE_PROTO_ID_ITEM, len(protos), proto_ids_off),
        (TYPE_METHOD_ID_ITEM, len(methods), method_ids_off),
        (TYPE_CLASS_DEF_ITEM, 1, class_defs_off),
        (TYPE_STRING_DATA_ITEM, len(strings), string_data_offsets[0]),
        (TYPE_CODE_ITEM, 2, ctor_code_off),
        (TYPE_CLASS_DATA_ITEM, 1, class_data_off),
        (TYPE_MAP_LIST, 1, map_off),
    ]
    map_blob = bytearray(struct.pack("<I", len(map_entries)))
    for type_code, size, offset in map_entries:
        map_blob += struct.pack("<HHII", type_code, 0, size, offset)
    data += map_blob

    file_size = data_off + len(data)
    assert file_size % 4 == 0

    dex = bytearray(file_size)
    dex[0:8] = b"dex\n035\x00"
    struct.pack_into("<I", dex, 0x20, file_size)
    struct.pack_into("<I", dex, 0x24, header_size)
    struct.pack_into("<I", dex, 0x28, 0x12345678)
    struct.pack_into("<I", dex, 0x34, map_off)
    struct.pack_into("<II", dex, 0x38, len(strings), string_ids_off)
    struct.pack_into("<II", dex, 0x40, len(types), type_ids_off)
    struct.pack_into("<II", dex, 0x48, len(protos), proto_ids_off)
    struct.pack_into("<II", dex, 0x58, len(methods), method_ids_off)
    struct.pack_into("<II", dex, 0x60, 1, class_defs_off)
    struct.pack_into("<II", dex, 0x68, len(data), data_off)

    for index, offset in enumerate(string_data_offsets):
        struct.pack_into("<I", dex, string_ids_off + 4 * index, offset)
    for index, descriptor in enumerate(types):
        struct.pack_into("<I", dex, type_ids_off + 4 * index, string_idx[descriptor])
    for index, (shorty, return_type) in enumerate(protos):
        struct.pack_into("<III", dex, proto_ids_off + 12 * index,
                         string_idx[shorty], type_idx[return_type], 0)
    for index, (owner, name, proto_index) in enumerate(methods):
        # method_id_item layout is class_idx:u16, proto_idx:u16, name_idx:u32.
        struct.pack_into("<HHI", dex, method_ids_off + 8 * index,
                         type_idx[owner], proto_index, string_idx[name])
    struct.pack_into("<IIIIIIII", dex, class_defs_off,
                     type_idx["Lfixtures/Case;"], ACC_PUBLIC,
                     type_idx["Ljava/lang/Object;"], 0,
                     string_idx["Case.smali"], 0, class_data_off, 0)
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    anchors = {
        "f_code_off": f_code_off,
        "f_insn0_off": f_code_off + 16,
        "class_data_off": class_data_off,
        "map_off": map_off,
        "file_size": file_size,
    }
    return dex, anchors


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D059")
    out_dir.mkdir(parents=True, exist_ok=True)

    original, original_anchors = build(ORIGINAL_CODE_UNIT_0)
    (out_dir / "original.dex").write_bytes(original)
    original_hash = hashlib.sha256(original).hexdigest()

    mutated = bytearray(original)
    anchor = original_anchors["f_insn0_off"]
    original_low_byte = mutated[anchor]
    assert original_low_byte == 0x0e
    mutated[anchor] = 0x3e
    file_size = len(mutated)
    mutated[0x0c:0x20] = hashlib.sha1(mutated[0x20:file_size]).digest()
    struct.pack_into("<I", mutated, 0x08,
                     zlib.adler32(mutated[0x0c:file_size]) & 0xffffffff)
    (out_dir / "mutated.dex").write_bytes(mutated)
    mutated_hash = hashlib.sha256(mutated).hexdigest()

    changed = [index for index in range(len(original)) if original[index] != mutated[index]]
    print(f"original file_size={original_anchors['file_size']} sha256={original_hash}")
    print(f"  f_code_off=0x{original_anchors['f_code_off']:x}"
          f" registers_size=0 ins_size=0 outs_size=0 tries_size=0 insns_size=1"
          f" code_unit0=0x{ORIGINAL_CODE_UNIT_0:04x} (return-void 10x)")
    print(f"mutated file_size={len(mutated)} sha256={mutated_hash}")
    print(f"  field=code_unit[0] low opcode byte at file offset 0x{anchor:x}"
          f" original=0x{original_low_byte:02x} mutated=0x3e; high byte stays 0x00")
    print(f"  changed_bytes={len(changed)} at={[hex(i) for i in changed]}"
          f" (1 opcode byte plus header checksum/signature)")
    print("  note: 0x3e is unused in 035, so no standard instruction width is assigned")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
