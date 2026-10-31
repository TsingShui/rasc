#!/usr/bin/env python3
"""D085 case-local 038 DEX builder: reachable invoke-custom in one member.

Writes .cache/e2e/D085/case.dex, a 038 DEX with:

  Lfixtures/Case;  (canonical Object constructor)
    static zAdd(II)I        add-int v0, p0, p1 ; return v0
    static aDynamic()I      const-string v0, <isolated-surrogate literal>
                            invoke-custom {}, call_site#0 ; move-result v0
                            return v0

The call site encodes exactly [method_handle Bootstrap.link, name "link",
method_type ()I]; the bootstrap handle is an invoke-static handle on

    Lfixtures/Bootstrap;->link(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;

so the call_site/method_handle/proto metadata is complete and legal. The
const-string value has the UTF-16 units [0,55296,42,47,92,117,48,48,48,97]:
U+0000, an isolated high surrogate U+D800, then '*', '/', '\\', 'u', '0','0','0','a'.
It is written as MUTF-8 (C0 80 for the NUL, ED A0 80 for the lone surrogate), so
a lossy UTF-8 round trip changes the u16 vector.

In class_data direct_methods, aDynamic precedes zAdd; that is the legal input
order the case certifies, so stopping analysis of the remaining methods on a
failure becomes visible.

Standard library only, no network, no subprocess.
Usage: python3 build_dex.py [output-path]
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
TYPE_CALL_SITE_ID_ITEM = 0x0007
TYPE_METHOD_HANDLE_ITEM = 0x0008
TYPE_MAP_LIST = 0x1000
TYPE_TYPE_LIST = 0x1001
TYPE_CLASS_DATA_ITEM = 0x2000
TYPE_CODE_ITEM = 0x2001
TYPE_STRING_DATA_ITEM = 0x2002
TYPE_ENCODED_ARRAY_ITEM = 0x2005

VALUE_METHOD_TYPE = 0x15
VALUE_METHOD_HANDLE = 0x16
VALUE_STRING = 0x17

HANDLE_INVOKE_STATIC = 0x04

DYNAMIC_LITERAL_UNITS = [0x0000, 0xD800, 0x002A, 0x002F, 0x005C,
                         0x0075, 0x0030, 0x0030, 0x0030, 0x0061]

BOOTSTRAP_OWNER = "Lfixtures/Bootstrap;"
BOOTSTRAP_NAME = "link"
BOOTSTRAP_PROTO = (
    "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;"
    "Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;"
)
BOOTSTRAP_SHORTY = "LLLL"
BOOTSTRAP_PARAMS = [
    "Ljava/lang/invoke/MethodHandles$Lookup;",
    "Ljava/lang/String;",
    "Ljava/lang/invoke/MethodType;",
]
BOOTSTRAP_RETURN = "Ljava/lang/invoke/CallSite;"

CALL_SITE_NAME = "link"
CALL_SITE_PROTO = "()I"
CALL_SITE_SHORTY = "I"


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
    """UTF-16 code units of a surrogate-free string."""
    data = text.encode("utf-16-le")
    return [int.from_bytes(data[i : i + 2], "little") for i in range(0, len(data), 2)]


def mutf8_from_units(units: list[int]) -> bytes:
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


def mutf8(text: str) -> bytes:
    return mutf8_from_units(utf16_units(text))


def code_item(registers: int, ins: int, outs: int, units: list[int]) -> bytes:
    blob = bytearray(struct.pack("<HHHHII", registers, ins, outs, 0, 0, len(units)))
    for unit in units:
        blob += struct.pack("<H", unit)
    return bytes(blob)


def build() -> tuple[bytes, dict]:
    # The string pool is a list of UTF-16 unit vectors so the isolated surrogate
    # literal can be carried exactly. DEX requires string_ids ordered by UTF-16
    # code-point sequence, so the pool is sorted after collecting the entries.
    string_units = [utf16_units(text) for text in [
        "()I",
        "()V",
        "<init>",
        BOOTSTRAP_PROTO,
        "Case.smali",
        "I",
        "III",
        "LLLL",
        "Lfixtures/Bootstrap;",
        "Lfixtures/Case;",
        "Ljava/lang/Object;",
        "Ljava/lang/invoke/CallSite;",
        "Ljava/lang/invoke/MethodHandles$Lookup;",
        "Ljava/lang/invoke/MethodType;",
        "Ljava/lang/String;",
        "V",
        "aDynamic",
        "link",
        "zAdd",
    ]]
    string_units.append(list(DYNAMIC_LITERAL_UNITS))
    strings = sorted(string_units, key=lambda units: units)
    literal_index = strings.index(list(DYNAMIC_LITERAL_UNITS))

    def string_id(text: str) -> int:
        units = utf16_units(text)
        return strings.index(units)

    type_descriptors = [
        "I",
        "Lfixtures/Bootstrap;",
        "Lfixtures/Case;",
        "Ljava/lang/Object;",
        "Ljava/lang/invoke/CallSite;",
        "Ljava/lang/invoke/MethodHandles$Lookup;",
        "Ljava/lang/invoke/MethodType;",
        "Ljava/lang/String;",
        "V",
    ]
    # DEX requires type_ids ordered by the descriptor's string index.
    types = sorted(type_descriptors, key=string_id)
    type_idx = {value: index for index, value in enumerate(types)}

    # proto_ids must be sorted by (return_type_idx, parameter type_idx list) and
    # deduplicated: the call-site type ()I and aDynamic's ()I are the same proto,
    # so one entry serves both. With I=0, CallSite=5 and V=8 this order is
    # ()I, (II)I, bootstrap(CallSite), ()V.
    proto_specs = [
        {"shorty": CALL_SITE_SHORTY, "return": "I", "params": []},              # ()I
        {"shorty": "III", "return": "I", "params": ["I", "I"]},               # (II)I
        {"shorty": BOOTSTRAP_SHORTY, "return": BOOTSTRAP_RETURN,
         "params": BOOTSTRAP_PARAMS},                                             # bootstrap
        {"shorty": "V", "return": "V", "params": []},                          # ()V
    ]
    proto_specs.sort(key=lambda p: (type_idx[p["return"]],
                                    [type_idx[x] for x in p["params"]]))
    protos = proto_specs
    def proto_index(shorty: str, return_type: str, params: list[str]) -> int:
        for index, proto in enumerate(protos):
            if (proto["shorty"], proto["return"], proto["params"]) == (shorty, return_type, params):
                return index
        raise AssertionError(f"missing proto {shorty} {return_type} {params}")

    CALL_SITE_PROTO_INDEX = proto_index(CALL_SITE_SHORTY, "I", [])
    DYNAMIC_PROTO_INDEX = proto_index(CALL_SITE_SHORTY, "I", [])
    ZADD_PROTO_INDEX = proto_index("III", "I", ["I", "I"])
    BOOTSTRAP_PROTO_INDEX = proto_index(BOOTSTRAP_SHORTY, BOOTSTRAP_RETURN, BOOTSTRAP_PARAMS)
    CTOR_PROTO_INDEX = proto_index("V", "V", [])

    # method_ids must be sorted by (class_idx, name_idx, proto_idx).
    method_specs = [
        (BOOTSTRAP_OWNER, BOOTSTRAP_NAME, BOOTSTRAP_PROTO_INDEX),
        ("Lfixtures/Case;", "<init>", CTOR_PROTO_INDEX),
        ("Lfixtures/Case;", "aDynamic", DYNAMIC_PROTO_INDEX),
        ("Lfixtures/Case;", "zAdd", ZADD_PROTO_INDEX),
        ("Ljava/lang/Object;", "<init>", CTOR_PROTO_INDEX),
    ]
    method_specs.sort(key=lambda m: (type_idx[m[0]], string_id(m[1]), m[2]))
    methods = method_specs

    def method_index(owner: str, name: str, proto: int) -> int:
        for index, entry in enumerate(methods):
            if entry == (owner, name, proto):
                return index
        raise AssertionError(f"missing method {owner}->{name} proto {proto}")

    BOOTSTRAP_METHOD_INDEX = method_index(BOOTSTRAP_OWNER, BOOTSTRAP_NAME, BOOTSTRAP_PROTO_INDEX)
    CTOR_METHOD_INDEX = method_index("Lfixtures/Case;", "<init>", CTOR_PROTO_INDEX)
    DYNAMIC_METHOD_INDEX = method_index("Lfixtures/Case;", "aDynamic", DYNAMIC_PROTO_INDEX)
    ZADD_METHOD_INDEX = method_index("Lfixtures/Case;", "zAdd", ZADD_PROTO_INDEX)
    OBJECT_CTOR_METHOD_INDEX = method_index("Ljava/lang/Object;", "<init>", CTOR_PROTO_INDEX)

    header_size = 0x70
    string_ids_off = header_size
    type_ids_off = string_ids_off + 4 * len(strings)
    proto_ids_off = type_ids_off + 4 * len(types)
    method_ids_off = proto_ids_off + 12 * len(protos)
    class_defs_off = method_ids_off + 8 * len(methods)
    call_site_ids_off = class_defs_off + 32
    method_handles_off = call_site_ids_off + 4
    data_off = method_handles_off + 8

    data = bytearray()
    string_data_offsets = []
    for units in strings:
        string_data_offsets.append(data_off + len(data))
        data += uleb(len(units))
        data += mutf8_from_units(units)
        data.append(0x00)

    # One type_list per proto that has parameters.
    type_list_offsets = {}
    for proto in protos:
        if not proto["params"]:
            continue
        while (data_off + len(data)) % 4:
            data.append(0)
        offset = data_off + len(data)
        data += struct.pack("<I", len(proto["params"]))
        for descriptor in proto["params"]:
            data += struct.pack("<H", type_idx[descriptor])
        type_list_offsets[tuple(proto["params"])] = offset
    params_off = type_list_offsets[tuple(BOOTSTRAP_PARAMS)]
    zadd_params_off = type_list_offsets[("I", "I")]

    # code items.
    while (data_off + len(data)) % 4:
        data.append(0)
    ctor_code_off = data_off + len(data)
    # invoke-direct {p0}, Ljava/lang/Object;-><init>()V: 35c count 1, G=v0.
    data += code_item(1, 1, 1, [0x1070, OBJECT_CTOR_METHOD_INDEX, 0x0000, 0x000e])

    while (data_off + len(data)) % 4:
        data.append(0)
    zadd_code_off = data_off + len(data)
    # add-int v0, p0, p1: 23x, A=v0, B=p0=v0, C=p1=v1
    data += code_item(2, 2, 0, [0x0090, 0x0100, 0x000f])

    while (data_off + len(data)) % 4:
        data.append(0)
    dynamic_code_off = data_off + len(data)
    data += code_item(
        1,
        0,
        0,
        [
            0x001a, literal_index,  # const-string v0, <literal>
            0x00fc, 0x0000, 0x0000,  # invoke-custom {}, call_site#0
            0x000a,                 # move-result v0
            0x000f,                 # return v0
        ],
    )

    # class_data direct_methods are ordered by ascending method_idx: aDynamic
    # precedes zAdd, which is the legal input order the case certifies.
    class_data_off = data_off + len(data)
    class_data = bytearray(uleb(0) + uleb(0) + uleb(3) + uleb(0))
    class_data += uleb(CTOR_METHOD_INDEX) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_code_off)
    class_data += uleb(DYNAMIC_METHOD_INDEX - CTOR_METHOD_INDEX) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(dynamic_code_off)
    class_data += uleb(ZADD_METHOD_INDEX - DYNAMIC_METHOD_INDEX) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(zadd_code_off)
    data += class_data

    # call_site encoded_array: [method_handle, name, method_type].
    call_site_encoded_off = data_off + len(data)
    encoded = bytearray(uleb(3))
    encoded.append(VALUE_METHOD_HANDLE)
    encoded += uleb(0)  # method_handles index 0
    encoded.append(VALUE_STRING)
    encoded += uleb(string_id(CALL_SITE_NAME))
    encoded.append(VALUE_METHOD_TYPE)
    encoded += uleb(CALL_SITE_PROTO_INDEX)
    data += encoded

    while (data_off + len(data)) % 4:
        data.append(0)
    map_off = data_off + len(data)
    map_entries = [
        (TYPE_HEADER_ITEM, 1, 0),
        (TYPE_STRING_ID_ITEM, len(strings), string_ids_off),
        (TYPE_TYPE_ID_ITEM, len(types), type_ids_off),
        (TYPE_PROTO_ID_ITEM, len(protos), proto_ids_off),
        (TYPE_METHOD_ID_ITEM, len(methods), method_ids_off),
        (TYPE_CLASS_DEF_ITEM, 1, class_defs_off),
        (TYPE_CALL_SITE_ID_ITEM, 1, call_site_ids_off),
        (TYPE_METHOD_HANDLE_ITEM, 1, method_handles_off),
        (TYPE_STRING_DATA_ITEM, len(strings), string_data_offsets[0]),
        (TYPE_TYPE_LIST, 2, min(params_off, zadd_params_off)),
        (TYPE_CODE_ITEM, 3, ctor_code_off),
        (TYPE_CLASS_DATA_ITEM, 1, class_data_off),
        (TYPE_ENCODED_ARRAY_ITEM, 1, call_site_encoded_off),
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
    dex[0:8] = b"dex\n038\x00"
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
        struct.pack_into("<I", dex, type_ids_off + 4 * index, string_id(descriptor))
    for index, proto in enumerate(protos):
        struct.pack_into(
            "<III", dex, proto_ids_off + 12 * index,
            string_id(proto["shorty"]), type_idx[proto["return"]],
            type_list_offsets[tuple(proto["params"])] if proto["params"] else 0,
        )
    for index, (owner, name, proto_index) in enumerate(methods):
        # method_id_item layout is class_idx:u16, proto_idx:u16, name_idx:u32.
        struct.pack_into("<HHI", dex, method_ids_off + 8 * index,
                         type_idx[owner], proto_index, string_id(name))
    struct.pack_into(
        "<IIIIIIII", dex, class_defs_off,
        type_idx["Lfixtures/Case;"], ACC_PUBLIC,
        type_idx["Ljava/lang/Object;"], 0,
        string_id("Case.smali"), 0, class_data_off, 0,
    )

    struct.pack_into("<I", dex, call_site_ids_off, call_site_encoded_off)
    struct.pack_into("<HHHH", dex, method_handles_off,
                     HANDLE_INVOKE_STATIC, 0,
                     BOOTSTRAP_METHOD_INDEX,
                     BOOTSTRAP_PROTO_INDEX)

    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    anchors = {
        "file_size": file_size,
        "string_ids_off": string_ids_off,
        "type_ids_off": type_ids_off,
        "proto_ids_off": proto_ids_off,
        "method_ids_off": method_ids_off,
        "class_defs_off": class_defs_off,
        "call_site_ids_off": call_site_ids_off,
        "method_handles_off": method_handles_off,
        "params_off": params_off,
        "zadd_params_off": zadd_params_off,
        "ctor_code_off": ctor_code_off,
        "dynamic_code_off": dynamic_code_off,
        "zadd_code_off": zadd_code_off,
        "class_data_off": class_data_off,
        "call_site_encoded_off": call_site_encoded_off,
        "map_off": map_off,
        "literal_index": literal_index,
        "literal_units": DYNAMIC_LITERAL_UNITS,
    }
    return bytes(dex), anchors


def main() -> int:
    out_path = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D085/case.dex")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    blob, anchors = build()
    out_path.write_bytes(blob)
    print(f"output={out_path} file_size={anchors['file_size']}"
          f" sha256={hashlib.sha256(blob).hexdigest()}")
    for key in ("string_ids_off", "type_ids_off", "proto_ids_off", "method_ids_off",
                "class_defs_off", "call_site_ids_off", "method_handles_off",
                "params_off", "ctor_code_off", "dynamic_code_off", "zadd_code_off",
                "class_data_off", "call_site_encoded_off", "map_off"):
        print(f"{key}=0x{anchors[key]:x}")
    print(f"literal string_id_idx={anchors['literal_index']}"
          f" units={anchors['literal_units']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
