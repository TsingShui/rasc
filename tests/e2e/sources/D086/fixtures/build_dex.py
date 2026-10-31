#!/usr/bin/env python3
"""D086 case-local DEX builder: a 131073-code-unit method that is valid 035.

Builds one self-contained `Lfixtures/Case;` with the canonical Object
constructor and a static `f()V` whose code_item holds exactly 131073 code
units:

    unit 0        0x000e   return-void
    units 1..131072       0x0000   complete nop instructions (131072 of them)

`registers_size`, `ins_size`, `outs_size` and `tries_size` are all 0, and there
is no branch, no payload and no handler, so only unit 0 is reachable and the
remaining 131072 nops are complete, decodable and unreachable. Nothing is
truncated and no count exceeds the actual file: `insns_size` is 131073 and
131073 code units really exist.

This input exists to separate two contracts. The normal method code limit
`max_method_code_units` defaults to None, so the first invocation must recover
Java from the real range; the second invocation explicitly sets it to 131072,
which the 131073-unit range exceeds, and only then may a normal budget failure
be reported while the complete raw round trip keeps the return and every nop.
The builder never applies a limit itself and never shortens the dead range.

Standard library only; no network, no subprocess.

Usage: python3 fixtures/build_dex.py [output-directory]
Default output-directory is .cache/e2e/D086/.
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

OP_RETURN_VOID = 0x000e
OP_NOP = 0x0000
OP_INVOKE_DIRECT = 0x1070

DEAD_NOP_COUNT = 131072
INSNS_SIZE = 1 + DEAD_NOP_COUNT
EXPLICIT_NORMAL_LIMIT = 131072

FIXED_STRINGS = ["()V", "<init>", "I", "Lfixtures/Case;", "Ljava/lang/Object;",
                 "V", "f"]
TYPES = ["I", "Lfixtures/Case;", "Ljava/lang/Object;", "V"]


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


def ascii_units(text: str) -> list[int]:
    data = text.encode("utf-16-le")
    return [int.from_bytes(data[i : i + 2], "little") for i in range(0, len(data), 2)]


def mutf8(text: str) -> bytes:
    out = bytearray()
    for unit in ascii_units(text):
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


def build() -> tuple[bytearray, dict[str, int]]:
    strings = sorted(FIXED_STRINGS, key=ascii_units)
    string_idx = {value: index for index, value in enumerate(strings)}

    types = sorted(TYPES, key=lambda value: string_idx[value])
    type_idx = {value: index for index, value in enumerate(types)}

    # Protos ()V and ()V only exist once; the target f()V shares the ctor proto.
    protos = [("()V", "V", "V")]
    proto_idx = {entry[2]: index for index, entry in enumerate(protos)}

    methods = [("Lfixtures/Case;", "<init>", "()V", "V"),
               ("Lfixtures/Case;", "f", "()V", "V"),
               ("Ljava/lang/Object;", "<init>", "()V", "V")]
    methods.sort(key=lambda m: (type_idx[m[0]], string_idx[m[1]], proto_idx[m[3]]))
    method_idx = {m: index for index, m in enumerate(methods)}

    header_size = 0x70
    string_ids_off = header_size
    type_ids_off = string_ids_off + 4 * len(strings)
    proto_ids_off = type_ids_off + 4 * len(types)
    method_ids_off = proto_ids_off + 12 * len(protos)
    class_defs_off = method_ids_off + 8 * len(methods)
    data_off = class_defs_off + 32

    data = bytearray()
    string_data_offsets: list[int] = []
    for value in strings:
        string_data_offsets.append(data_off + len(data))
        data += uleb(len(ascii_units(value)))
        data += mutf8(value)
        data.append(0x00)

    ctor_off = align(data, data_off, 4)
    ctor_units = [OP_INVOKE_DIRECT, method_idx[("Ljava/lang/Object;", "<init>", "()V", "V")],
                  0x0000, OP_RETURN_VOID]
    data += code_item(1, 1, 1, ctor_units)

    f_off = align(data, data_off, 4)
    f_units = [OP_RETURN_VOID] + [OP_NOP] * DEAD_NOP_COUNT
    data += code_item(0, 0, 0, f_units)

    class_data_off = data_off + len(data)
    ctor_index = method_idx[("Lfixtures/Case;", "<init>", "()V", "V")]
    f_index = method_idx[("Lfixtures/Case;", "f", "()V", "V")]
    class_data = bytearray(uleb(0) + uleb(0) + uleb(2) + uleb(0))
    class_data += uleb(ctor_index) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_off)
    class_data += uleb(f_index - ctor_index) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(f_off)
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
        (TYPE_CODE_ITEM, 2, ctor_off),
        (TYPE_CLASS_DATA_ITEM, 1, class_data_off),
        (TYPE_MAP_LIST, 1, map_off),
    ]
    map_entries.sort(key=lambda entry: entry[2])
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
    struct.pack_into("<I", dex, 0x2c, 0)
    struct.pack_into("<I", dex, 0x30, 0)
    struct.pack_into("<I", dex, 0x34, map_off)
    struct.pack_into("<II", dex, 0x38, len(strings), string_ids_off)
    struct.pack_into("<II", dex, 0x40, len(types), type_ids_off)
    struct.pack_into("<II", dex, 0x48, len(protos), proto_ids_off)
    struct.pack_into("<II", dex, 0x50, 0, 0)
    struct.pack_into("<II", dex, 0x58, len(methods), method_ids_off)
    struct.pack_into("<II", dex, 0x60, 1, class_defs_off)
    struct.pack_into("<II", dex, 0x68, len(data), data_off)

    for index, offset in enumerate(string_data_offsets):
        struct.pack_into("<I", dex, string_ids_off + 4 * index, offset)
    for index, descriptor in enumerate(types):
        struct.pack_into("<I", dex, type_ids_off + 4 * index, string_idx[descriptor])
    for index, (proto, return_type, shorty) in enumerate(protos):
        struct.pack_into("<III", dex, proto_ids_off + 12 * index,
                         string_idx[shorty], type_idx[return_type], 0)
    for index, (owner, name, proto, shorty) in enumerate(methods):
        struct.pack_into("<HHI", dex, method_ids_off + 8 * index,
                         type_idx[owner], proto_idx[shorty], string_idx[name])
    struct.pack_into("<IIIIIIII", dex, class_defs_off,
                     type_idx["Lfixtures/Case;"], ACC_PUBLIC,
                     type_idx["Ljava/lang/Object;"], 0,
                     0, 0, class_data_off, 0)
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    facts = {
        "insns_size": INSNS_SIZE,
        "dead_nops": DEAD_NOP_COUNT,
        "f_code_off": f_off,
        "ctor_code_off": ctor_off,
    }
    return dex, facts


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D086")
    out_dir.mkdir(parents=True, exist_ok=True)

    dex, facts = build()
    path = out_dir / "case.dex"
    path.write_bytes(dex)
    print(f"{path} file_size={len(dex)} sha256={hashlib.sha256(dex).hexdigest()}")
    print(f"f_code_off=0x{facts['f_code_off']:x} insns_size={facts['insns_size']}"
          f" first=return-void dead_nops={facts['dead_nops']}")
    print(f"explicit normal limit for the second invocation:"
          f" max_method_code_units={EXPLICIT_NORMAL_LIMIT}"
          f" (range {facts['insns_size']} exceeds it; raw quotas stay None)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
