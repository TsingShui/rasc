#!/usr/bin/env python3
"""D078 case-local DEX builder: an interface with 2200 code-less declarations.

Builds one self-made 035 `Lfixtures/Case;` that is a public interface holding
exactly 2200 distinct abstract declarations `m0000()I .. m2199()I`, with no
constructor, no `Code` and no default method. The declaration count is the
point of the case, so it is generated from DECLARATION_COUNT rather than
hand-maintained.

The specification binds the DEX as the original input (`inputKind`
self-made-dex-builder-with-reference-java), so the Java source alone is not
the original artifact. Both are produced here: `fixtures/Case.java` is the
reference Java and this builder is the original DEX.

Published layout facts used:
  * a DEX interface has no direct methods and no code_item; every declaration
    sits in the virtual group with ACC_PUBLIC | ACC_ABSTRACT and code_off 0
  * class_data lists the virtual group's method_idx deltas accumulated from 0
  * a proto with an empty parameter list has parameters_off 0 and no type_list
  * proto_ids.shorty_idx points at the shorty string ("I" here), never at the
    full descriptor
  * map_list entries are ordered by their initial offset

Standard library only; no network, no subprocess.

Usage: python3 fixtures/build_dex.py [output-directory]
Default output-directory is .cache/e2e/D078/.
"""

from __future__ import annotations

import hashlib
import struct
import sys
import zlib
from pathlib import Path

ACC_PUBLIC = 0x0001
ACC_ABSTRACT = 0x0400

TYPE_HEADER_ITEM = 0x0000
TYPE_STRING_ID_ITEM = 0x0001
TYPE_TYPE_ID_ITEM = 0x0002
TYPE_PROTO_ID_ITEM = 0x0003
TYPE_METHOD_ID_ITEM = 0x0005
TYPE_CLASS_DEF_ITEM = 0x0006
TYPE_MAP_LIST = 0x1000
TYPE_CLASS_DATA_ITEM = 0x2000
TYPE_STRING_DATA_ITEM = 0x2002

DECLARATION_COUNT = 2200
OWNER = "Lfixtures/Case;"
OBJECT = "Ljava/lang/Object;"
INT = "I"
PROTO = "()I"
SHORTY = "I"


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


def method_names() -> list[str]:
    return ["m%04d" % index for index in range(DECLARATION_COUNT)]


def build() -> tuple[bytearray, dict[str, int]]:
    names = method_names()
    strings = list({INT, OBJECT, OWNER, SHORTY, PROTO}) + names
    strings.sort(key=lambda value: ascii_units(value))
    string_idx = {value: index for index, value in enumerate(strings)}

    types = [INT, OBJECT, OWNER]
    types.sort(key=lambda value: string_idx[value])
    type_idx = {value: index for index, value in enumerate(types)}

    # One proto only: ()I. The published shorty for it is "I".
    protos = [(PROTO, INT, SHORTY)]
    proto_idx = {PROTO: 0}

    methods = [(OWNER, name, PROTO) for name in names]
    # method_ids are ordered by (class_idx, name_idx, proto_idx).
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
        data += mutf8(value)
        data.append(0x00)

    # An interface has no fields and no direct methods. Every declaration is in
    # the virtual group with code_off 0, and the group's ULEB method_idx delta
    # accumulates from 0, so the first delta is the absolute method_idx.
    class_data_off = data_off + len(data)
    class_data = bytearray(uleb(0) + uleb(0) + uleb(0) + uleb(len(methods)))
    previous = 0
    for index, method in enumerate(methods):
        absolute = method_idx[method]
        delta = absolute if index == 0 else absolute - previous
        class_data += uleb(delta) + uleb(ACC_PUBLIC | ACC_ABSTRACT) + uleb(0)
        previous = absolute
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
    # The DEX declaration carries both ACC_INTERFACE and ACC_ABSTRACT.
    struct.pack_into("<IIIIIIII", dex, class_defs_off,
                     type_idx[OWNER], ACC_PUBLIC | 0x0200 | ACC_ABSTRACT, type_idx[OBJECT], 0,
                     0, 0, class_data_off, 0)
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    facts = {
        "declarations": len(methods),
        "distinct_names": len(set(name for _, name, _ in methods)),
        "class_data_off": class_data_off,
        "strings": len(strings),
        "methods": len(methods),
    }
    return dex, facts


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D078")
    out_dir.mkdir(parents=True, exist_ok=True)

    dex, facts = build()
    path = out_dir / "case.dex"
    path.write_bytes(dex)
    print(f"case: {path} file_size={len(dex)} sha256={hashlib.sha256(dex).hexdigest()}")
    print(f"  declarations={facts['declarations']} distinct_names={facts['distinct_names']}"
          f" strings={facts['strings']} class_data_off=0x{facts['class_data_off']:x}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
