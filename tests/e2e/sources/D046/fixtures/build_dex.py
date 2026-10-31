#!/usr/bin/env python3
"""D046 case-local DEX builder: explicit encoded static default values.

D046 needs a 035 DEX whose class_data static_fields are b:Z, i:I, s:Ljava/lang/String;
in that order and whose static_values encoded_array holds exactly three entries:

    size = 3
    [0] VALUE_BOOLEAN(false)
    [1] VALUE_INT(0)
    [2] VALUE_NULL

A Java-to-DEX compiler may omit an all-default static_values array, so the case
cannot be built from Case.java. This script writes the DEX directly and prints
the anchors the recipe records (offsets, tags and the SHA-256 of the result).

The output goes to .cache/e2e/D046/case.dex; the checked-in artifact is this
script only.  Standard library only, no network, no subprocess.

Usage: python3 tests/e2e/sources/D046/fixtures/build_dex.py [output-path]
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

VALUE_INT = 0x04
VALUE_NULL = 0x1e
VALUE_BOOLEAN = 0x1f

TYPE_HEADER_ITEM = 0x0000
TYPE_STRING_ID_ITEM = 0x0001
TYPE_TYPE_ID_ITEM = 0x0002
TYPE_PROTO_ID_ITEM = 0x0003
TYPE_FIELD_ID_ITEM = 0x0004
TYPE_METHOD_ID_ITEM = 0x0005
TYPE_CLASS_DEF_ITEM = 0x0006
TYPE_MAP_LIST = 0x1000
TYPE_TYPE_LIST = 0x1001
TYPE_CLASS_DATA_ITEM = 0x2000
TYPE_CODE_ITEM = 0x2001
TYPE_STRING_DATA_ITEM = 0x2002
TYPE_ENCODED_ARRAY_ITEM = 0x2005


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


def mutf8(text: str) -> bytes:
    out = bytearray()
    for unit in encode_utf16_units(text):
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


def encode_utf16_units(text: str) -> list[int]:
    data = text.encode("utf-16-le")
    return [int.from_bytes(data[i : i + 2], "little") for i in range(0, len(data), 2)]


def align(blob: bytearray, base: int, alignment: int) -> int:
    while (base + len(blob)) % alignment:
        blob.append(0)
    return base + len(blob)


def main() -> int:
    out_path = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(".cache/e2e/D046/case.dex")
    )

    # string_ids order is the UTF-16 code-point order of the values.
    strings = [
        "()V",
        "<init>",
        "Case.java",
        "I",
        "Lfixtures/Case;",
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "V",
        "Z",
        "b",
        "i",
        "s",
    ]
    string_idx = {value: index for index, value in enumerate(strings)}

    # type_ids order follows the descriptor's string index.
    types = [
        "I",
        "Lfixtures/Case;",
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "V",
        "Z",
    ]
    type_idx = {value: index for index, value in enumerate(types)}

    proto_shorty = "V"
    proto_return = "V"

    fields = [("b", "Z"), ("i", "I"), ("s", "Ljava/lang/String;")]
    field_owner = "Lfixtures/Case;"

    methods = [
        ("Lfixtures/Case;", "<init>"),
        ("Ljava/lang/Object;", "<init>"),
    ]

    header_size = 0x70
    string_ids_off = header_size
    type_ids_off = string_ids_off + 4 * len(strings)
    proto_ids_off = type_ids_off + 4 * len(types)
    field_ids_off = proto_ids_off + 12 * 1
    method_ids_off = field_ids_off + 8 * len(fields)
    class_defs_off = method_ids_off + 8 * len(methods)
    data_off = class_defs_off + 32 * 1

    data = bytearray()

    # string_data_item: uleb utf16_size, MUTF-8 bytes, 0x00 terminator.
    string_data_offsets: list[int] = []
    for value in strings:
        string_data_offsets.append(data_off + len(data))
        data += uleb(len(encode_utf16_units(value)))
        data += mutf8(value)
        data.append(0x00)

    # code_item for <init>()V: invoke-direct {p0}, Object.<init>()V; return-void.
    code_off = align(data, data_off, 4)
    insns_size = 4
    code = bytearray()
    code += struct.pack(
        "<HHHHII",
        1,  # registers_size
        1,  # ins_size
        1,  # outs_size
        0,  # tries_size
        0,  # debug_info_off
        insns_size,
    )
    code += struct.pack("<H", 0x1070)  # invoke-direct, count=1, G=v0
    code += struct.pack("<H", methods.index(("Ljava/lang/Object;", "<init>")))
    code += struct.pack("<H", 0x0000)  # C=v0, D/E/F unused
    code += struct.pack("<H", 0x000e)  # return-void
    assert len(code) == 16 + 2 * insns_size
    data += code

    # class_data_item: 3 static fields (b, i, s), 0 instance fields,
    # 1 direct method (<init>), 0 virtual methods.
    class_data_off = data_off + len(data)
    class_data = bytearray()
    class_data += uleb(3)
    class_data += uleb(0)
    class_data += uleb(1)
    class_data += uleb(0)
    previous = 0
    for name, type_descriptor in fields:
        index = fields.index((name, type_descriptor))
        class_data += uleb(index - previous)
        class_data += uleb(ACC_PUBLIC | ACC_STATIC)
        previous = index
    class_data += uleb(0)  # method_idx_diff for <init> (method index 0)
    class_data += uleb(ACC_CONSTRUCTOR)
    class_data += uleb(code_off)
    data += class_data

    # static_values encoded_array: size 3, boolean false, int 0, null.
    # Each element is one encoded_value: (value_arg << 5) | value_type, followed
    # by value_arg+1 data bytes only for the sized kinds. VALUE_BOOLEAN and
    # VALUE_NULL carry no data bytes, so a stray byte after VALUE_BOOLEAN would
    # start a new encoded_value instead of staying inside element 0.
    static_values_off = data_off + len(data)
    encoded = bytearray()
    encoded += uleb(3)
    encoded.append(VALUE_BOOLEAN)
    encoded.append(VALUE_INT)
    encoded.append(0x00)
    encoded.append(VALUE_NULL)
    data += encoded
    assert len(encoded) == 5

    map_off = align(data, data_off, 4)
    map_entries = [
        (TYPE_HEADER_ITEM, 1, 0),
        (TYPE_STRING_ID_ITEM, len(strings), string_ids_off),
        (TYPE_TYPE_ID_ITEM, len(types), type_ids_off),
        (TYPE_PROTO_ID_ITEM, 1, proto_ids_off),
        (TYPE_FIELD_ID_ITEM, len(fields), field_ids_off),
        (TYPE_METHOD_ID_ITEM, len(methods), method_ids_off),
        (TYPE_CLASS_DEF_ITEM, 1, class_defs_off),
        (TYPE_STRING_DATA_ITEM, len(strings), string_data_offsets[0]),
        (TYPE_CODE_ITEM, 1, code_off),
        (TYPE_CLASS_DATA_ITEM, 1, class_data_off),
        (TYPE_ENCODED_ARRAY_ITEM, 1, static_values_off),
        (TYPE_MAP_LIST, 1, map_off),
    ]
    map_blob = bytearray()
    map_blob += struct.pack("<I", len(map_entries))
    for type_code, size, offset in map_entries:
        map_blob += struct.pack("<HHI", type_code, 0, size)
        map_blob += struct.pack("<I", offset)
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
    struct.pack_into("<II", dex, 0x48, 1, proto_ids_off)
    struct.pack_into("<II", dex, 0x50, len(fields), field_ids_off)
    struct.pack_into("<II", dex, 0x58, len(methods), method_ids_off)
    struct.pack_into("<II", dex, 0x60, 1, class_defs_off)
    struct.pack_into("<II", dex, 0x68, len(data), data_off)

    for index, offset in enumerate(string_data_offsets):
        struct.pack_into("<I", dex, string_ids_off + 4 * index, offset)
    for index, descriptor in enumerate(types):
        struct.pack_into("<I", dex, type_ids_off + 4 * index, string_idx[descriptor])
    struct.pack_into(
        "<III",
        dex,
        proto_ids_off,
        string_idx[proto_shorty],
        type_idx[proto_return],
        0,
    )
    for index, (name, type_descriptor) in enumerate(fields):
        struct.pack_into(
            "<HHI",
            dex,
            field_ids_off + 8 * index,
            type_idx[field_owner],
            type_idx[type_descriptor],
            string_idx[name],
        )
    for index, (owner, name) in enumerate(methods):
        # method_id_item layout is class_idx:u16, proto_idx:u16, name_idx:u32.
        struct.pack_into(
            "<HHI",
            dex,
            method_ids_off + 8 * index,
            type_idx[owner],
            0,  # the single proto ()V is proto_ids[0]
            string_idx[name],
        )
    struct.pack_into(
        "<IIIIIIII",
        dex,
        class_defs_off,
        type_idx[field_owner],
        ACC_PUBLIC,
        type_idx["Ljava/lang/Object;"],
        0,
        string_idx["Case.java"],
        0,
        class_data_off,
        static_values_off,
    )
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_bytes(dex)

    digest = hashlib.sha256(dex).hexdigest()
    print(f"output={out_path}")
    print(f"file_size={file_size} sha256={digest}")
    print(f"string_ids_off=0x{string_ids_off:x} count={len(strings)}")
    print(f"type_ids_off=0x{type_ids_off:x} count={len(types)}")
    print(f"proto_ids_off=0x{proto_ids_off:x} count=1")
    print(f"field_ids_off=0x{field_ids_off:x} count={len(fields)}")
    print(f"method_ids_off=0x{method_ids_off:x} count={len(methods)}")
    print(f"class_defs_off=0x{class_defs_off:x} count=1")
    print(f"code_off=0x{code_off:x} insns_size={insns_size}")
    print(f"class_data_off=0x{class_data_off:x} static_fields=[b:Z,i:I,s:Ljava/lang/String;]")
    print(f"static_values_off=0x{static_values_off:x} size=3 tags=[0x1f:false,0x04:0,0x1e:null]")
    print(f"map_off=0x{map_off:x} entries={len(map_entries)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
