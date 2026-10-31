#!/usr/bin/env python3
"""D076 case-local DEX builder: NUL-run const-string at the MUTF8 boundary.

Builds three independent 035 owners, each a self-contained `Lfixtures/Case;`
with the canonical Object constructor and one static `f()Ljava/lang/String;`
whose body is `const-string v0, <literal>` followed by `return-object v0`.
The three inputs differ only in the literal's code units:

    nul32767.dex     32767 NUL units              MUTF8 65534 bytes
    nul32767-A.dex   32767 NUL units + 'A'        MUTF8 65535 bytes
    nul32768.dex     32768 NUL units              MUTF8 65536 bytes

Per ALGORITHMS 4.8 the boundary is measured in MUTF8 encoded bytes, counting
NUL as 2 and ASCII as 1, not in u16 units and not in Java source escape
length. The 65536-byte item is legal as a DEX string_data_item; only the
JVMS CONSTANT_Utf8 form has the 65535-byte bound, which is why the two legal
roles also carry a compilable Java reference and the over-limit role does not.

Every other part of the file is independently valid: sorted string_ids,
type_ids, proto_ids and method_ids, a complete class_data, a real code_item
with registers_size=1, and a map_list. Nothing is truncated or synthesized.

Standard library only; no network, no subprocess.

Usage: python3 fixtures/build_dex.py [output-directory]
Default output-directory is .cache/e2e/D076/.
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

OP_CONST_STRING = 0x001a
OP_RETURN_OBJECT = 0x0011
OP_INVOKE_DIRECT = 0x1070
OP_RETURN_VOID = 0x000e

ROLES = [
    ("nul32767", [0] * 32767),
    ("nul32767-A", [0] * 32767 + [0x0041]),
    ("nul32768", [0] * 32768),
]

FIXED_STRINGS = [
    "()Ljava/lang/String;",
    "()V",
    "<init>",
    "L",
    "Lfixtures/Case;",
    "Ljava/lang/Object;",
    "Ljava/lang/String;",
    "V",
    "f",
]


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


def mutf8_units(units: list[int]) -> bytes:
    """Encode UTF-16 code units as MUTF8: NUL=2 bytes, ASCII=1, else 3."""
    out = bytearray()
    for unit in units:
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


def ascii_units(text: str) -> list[int]:
    data = text.encode("utf-16-le")
    return [int.from_bytes(data[i : i + 2], "little") for i in range(0, len(data), 2)]


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


def build(literal_units: list[int]) -> tuple[bytearray, dict[str, int]]:
    strings = list(FIXED_STRINGS) + ["".join(chr(u) for u in literal_units)]
    # string_ids are ordered by UTF-16 code-unit sequence.
    strings.sort(key=lambda value: ascii_units(value))
    string_idx = {value: index for index, value in enumerate(strings)}

    types = ["Lfixtures/Case;", "Ljava/lang/Object;", "Ljava/lang/String;", "V"]
    types.sort(key=lambda value: string_idx[value])
    type_idx = {value: index for index, value in enumerate(types)}

    # Published shorty: the return-type shorty followed by one shorty per
    # parameter. ()Ljava/lang/String; is "L" and ()V is "V"; the shorty_idx
    # must point at those, never at the full proto descriptor.
    protos = [("()Ljava/lang/String;", "Ljava/lang/String;", "L"),
              ("()V", "V", "V")]
    protos.sort(key=lambda entry: (type_idx[entry[1]], ()))
    proto_idx = {entry[0]: index for index, entry in enumerate(protos)}

    methods = [("Lfixtures/Case;", "<init>", "()V"),
               ("Lfixtures/Case;", "f", "()Ljava/lang/String;"),
               ("Ljava/lang/Object;", "<init>", "()V")]
    methods.sort(key=lambda m: (type_idx[m[0]], string_idx[m[1]], proto_idx[m[2]]))
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
        data += mutf8_units(ascii_units(value))
        data.append(0x00)

    literal = "".join(chr(u) for u in literal_units)
    literal_index = string_idx[literal]

    ctor_off = align(data, data_off, 4)
    ctor_units = [OP_INVOKE_DIRECT, method_idx[("Ljava/lang/Object;", "<init>", "()V")],
                  0x0000, OP_RETURN_VOID]
    data += code_item(1, 1, 1, ctor_units)

    f_off = align(data, data_off, 4)
    f_units = [OP_CONST_STRING, literal_index, OP_RETURN_OBJECT]
    data += code_item(1, 0, 0, f_units)

    class_data_off = data_off + len(data)
    class_data = bytearray(uleb(0) + uleb(0) + uleb(2) + uleb(0))
    ctor_index = method_idx[("Lfixtures/Case;", "<init>", "()V")]
    f_index = method_idx[("Lfixtures/Case;", "f", "()Ljava/lang/String;")]
    first, second = sorted([(ctor_index, ACC_CONSTRUCTOR, ctor_off),
                            (f_index, ACC_PUBLIC | ACC_STATIC, f_off)])
    class_data += uleb(first[0]) + uleb(first[1]) + uleb(first[2])
    class_data += uleb(second[0] - first[0]) + uleb(second[1]) + uleb(second[2])
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
    # The map_list entries are ordered by initial offset.
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
    for index, (proto_descriptor, return_type, shorty_value) in enumerate(protos):
        struct.pack_into("<III", dex, proto_ids_off + 12 * index,
                         string_idx[shorty_value], type_idx[return_type], 0)
    for index, (owner, name, proto_descriptor) in enumerate(methods):
        struct.pack_into("<HHI", dex, method_ids_off + 8 * index,
                         type_idx[owner], proto_idx[proto_descriptor], string_idx[name])
    struct.pack_into("<IIIIIIII", dex, class_defs_off,
                     type_idx["Lfixtures/Case;"], ACC_PUBLIC,
                     type_idx["Ljava/lang/Object;"], 0,
                     0, 0, class_data_off, 0)
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    facts = {
        "literal_string_idx": literal_index,
        "string_data_off": string_data_offsets[literal_index],
        "mutf8_bytes": len(mutf8_units(literal_units)),
        "utf16_units": len(literal_units),
        "f_code_off": f_off,
        "string_count": len(strings),
    }
    return dex, facts


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D076")
    out_dir.mkdir(parents=True, exist_ok=True)

    for role, units in ROLES:
        dex, facts = build(units)
        path = out_dir / f"{role}.dex"
        path.write_bytes(dex)
        print(f"{role}: {path} file_size={len(dex)} sha256={hashlib.sha256(dex).hexdigest()}")
        print(f"  utf16_units={facts['utf16_units']} mutf8_bytes={facts['mutf8_bytes']}"
              f" string_idx={facts['literal_string_idx']}"
              f" string_data_off=0x{facts['string_data_off']:x}"
              f" f_code_off=0x{facts['f_code_off']:x} strings={facts['string_count']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
