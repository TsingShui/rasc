#!/usr/bin/env python3
"""D075 case-local DEX builder: the 255-parameter-word boundary in six roles.

Builds six independent 035 owners, each a self-contained `Lfixtures/Case;`
carrying one method `f` with the proto the specification binds. The boundary
counts implicit `this` plus the formal parameter words of the binary proto
(JVMS 4.3.3 / ALGORITHMS 12.4): every formal is `I`, so each contributes one
word.

    static255    (I x 255)I  static,   255 words  -> accepted, body returns p0
    static256    (I x 256)I  static,   256 words  -> over the Java limit
    instance254  (I x 254)I  instance, 255 words  -> accepted, body returns p0
    instance255  (I x 255)I  instance, 256 words  -> over the Java limit
    abstract254  (I x 254)I  abstract, 255 words  -> accepted declaration only
    abstract255  (I x 255)I  abstract, 256 words  -> over the Java limit

The DEX field and register range are themselves valid for every role: a
256-word `ins_size` is a legal u2 value, so the over-limit roles are rejected
by the Java parameter-word check and never by DEX structural integrity. Every
owner, including the abstract variant, keeps the canonical `Object.<init>()V`
constructor with real code, as the specification requires; only `f` differs,
being a code-less declaration in the abstract roles and a body in the others.

Bodies use exactly one `return` of the first int parameter, so the accepted
roles have real, executable behavior and the Driver can observe a return
value. Nothing is truncated, renamed or split.

Standard library only; no network, no subprocess.

Usage: python3 fixtures/build_dex.py [output-directory]
Default output-directory is .cache/e2e/D075/.
"""

from __future__ import annotations

import hashlib
import struct
import sys
import zlib
from pathlib import Path

ACC_PUBLIC = 0x0001
ACC_STATIC = 0x0009
ACC_ABSTRACT = 0x0400
ACC_CONSTRUCTOR = 0x10001

TYPE_HEADER_ITEM = 0x0000
TYPE_STRING_ID_ITEM = 0x0001
TYPE_TYPE_ID_ITEM = 0x0002
TYPE_PROTO_ID_ITEM = 0x0003
# Published map_item code for type_list is 0x1001; 0x1003 is annotation_set_item.
TYPE_TYPE_LIST = 0x1001
TYPE_METHOD_ID_ITEM = 0x0005
TYPE_CLASS_DEF_ITEM = 0x0006
TYPE_MAP_LIST = 0x1000
TYPE_CLASS_DATA_ITEM = 0x2000
TYPE_CODE_ITEM = 0x2001
TYPE_STRING_DATA_ITEM = 0x2002

OP_RETURN = 0x000f
OP_INVOKE_DIRECT = 0x1070
OP_RETURN_VOID = 0x000e

# role, kind, parameter count. kind decides this-words, flags and the body.
ROLES = [
    ("static255", "static", 255),
    ("static256", "static", 256),
    ("instance254", "instance", 254),
    ("instance255", "instance", 255),
    ("abstract254", "abstract", 254),
    ("abstract255", "abstract", 255),
]

OWNER = "Lfixtures/Case;"
OBJECT = "Ljava/lang/Object;"
INT = "I"


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


def proto_for(kind: str, params: int) -> tuple[str, str]:
    """Return (proto descriptor, shorty) for this role."""
    descriptor = "(" + INT * params + ")" + INT
    return descriptor, INT + INT * params


def body_units(kind: str) -> list[int]:
    # The first int parameter sits in v0 for static methods and in v1 for
    # instance methods, because v0 then holds `this`.
    register = 0 if kind == "static" else 1
    return [OP_RETURN | (register << 8)]


def build(kind: str, params: int) -> tuple[bytearray, dict[str, int]]:
    proto, shorty = proto_for(kind, params)
    abstract = kind == "abstract"

    strings = ["<init>", "()V", INT, OBJECT, OWNER, "V", "f", proto, shorty]
    strings.sort(key=lambda value: ascii_units(value))
    string_idx = {value: index for index, value in enumerate(strings)}

    types = [INT, OBJECT, OWNER, "V"]
    types.sort(key=lambda value: string_idx[value])
    type_idx = {value: index for index, value in enumerate(types)}

    # Proto list: the ctor proto ()V and the target proto, ordered by return type.
    protos = [("()V", "V", "V", 0), (proto, INT, shorty, params)]
    protos.sort(key=lambda entry: type_idx[entry[1]])
    proto_idx = {entry[2]: index for index, entry in enumerate(protos)}

    methods: list[tuple[str, str, str, str]] = []
    # Every owner, including the abstract variant, keeps the canonical
    # Object.<init>()V constructor; only `f` differs between the roles.
    methods.append((OWNER, "<init>", "()V", "V"))
    methods.append((OBJECT, "<init>", "()V", "V"))
    methods.append((OWNER, "f", proto, shorty))
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

    # type_list for the target proto's parameter list (4-byte aligned).
    params_off = align(data, data_off, 4)
    data += struct.pack("<I", params)
    for _ in range(params):
        data += struct.pack("<H", type_idx[INT])

    # Every owner, the abstract variant included, keeps the canonical
    # Object.<init>()V constructor with real code; only `f` is code-less there.
    ctor_off = align(data, data_off, 4)
    ctor_units = [OP_INVOKE_DIRECT, method_idx[(OBJECT, "<init>", "()V", "V")],
                  0x0000, OP_RETURN_VOID]
    data += code_item(1, 1, 1, ctor_units)

    f_off = 0
    registers = params
    if not abstract:
        f_off = align(data, data_off, 4)
        # A single return needs no local, so registers_size equals the parameter
        # words: the formals for a static method, plus `this` for an instance one.
        registers = params + (1 if kind == "instance" else 0)
        data += code_item(registers, registers, 0, body_units(kind))

    class_data_off = data_off + len(data)
    # Published rule: the direct and virtual method groups each accumulate their
    # ULEB method_idx delta from 0 independently, so the first virtual method
    # carries its absolute method_idx, not a delta from the last direct one.
    ctor_index = method_idx[(OWNER, "<init>", "()V", "V")]
    f_index = method_idx[(OWNER, "f", proto, shorty)]
    if kind == "static":
        class_data = bytearray(uleb(0) + uleb(0) + uleb(2) + uleb(0))
        class_data += uleb(ctor_index) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_off)
        class_data += uleb(f_index - ctor_index) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(f_off)
    else:
        class_data = bytearray(uleb(0) + uleb(0) + uleb(1) + uleb(1))
        class_data += uleb(ctor_index) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_off)
        flags = ACC_PUBLIC | (ACC_ABSTRACT if abstract else 0)
        class_data += uleb(f_index) + uleb(flags) + uleb(f_off)
    data += class_data

    map_off = align(data, data_off, 4)
    map_entries = [
        (TYPE_HEADER_ITEM, 1, 0),
        (TYPE_STRING_ID_ITEM, len(strings), string_ids_off),
        (TYPE_TYPE_ID_ITEM, len(types), type_ids_off),
        (TYPE_PROTO_ID_ITEM, len(protos), proto_ids_off),
        (TYPE_TYPE_LIST, 1, params_off),
        (TYPE_METHOD_ID_ITEM, len(methods), method_ids_off),
        (TYPE_CLASS_DEF_ITEM, 1, class_defs_off),
        (TYPE_STRING_DATA_ITEM, len(strings), string_data_offsets[0]),
        (TYPE_CLASS_DATA_ITEM, 1, class_data_off),
        (TYPE_MAP_LIST, 1, map_off),
    ]
    if not abstract:
        map_entries.append((TYPE_CODE_ITEM, 2, ctor_off))
    else:
        # Only the constructor carries code; `f` stays a code-less declaration.
        map_entries.append((TYPE_CODE_ITEM, 1, ctor_off))
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
    for index, (proto_descriptor, return_type, shorty_value, param_count) in enumerate(protos):
        struct.pack_into("<III", dex, proto_ids_off + 12 * index,
                         string_idx[shorty_value], type_idx[return_type],
                         params_off if param_count else 0)
    for index, (owner, name, proto_descriptor, shorty_value) in enumerate(methods):
        struct.pack_into("<HHI", dex, method_ids_off + 8 * index,
                         type_idx[owner], proto_idx[shorty_value], string_idx[name])
    class_flags = ACC_PUBLIC | (ACC_ABSTRACT if abstract else 0)
    struct.pack_into("<IIIIIIII", dex, class_defs_off,
                     type_idx[OWNER], class_flags, type_idx[OBJECT], 0,
                     0, 0, class_data_off, 0)
    dex[data_off:file_size] = data

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    facts = {
        "parameter_words": params + (0 if kind == "static" else 1),
        "proto_length": len(proto),
        "f_code_off": f_off,
        "registers_size": registers if not abstract else 0,
        "params_off": params_off,
    }
    return dex, facts


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D075")
    out_dir.mkdir(parents=True, exist_ok=True)

    for role, kind, params in ROLES:
        dex, facts = build(kind, params)
        path = out_dir / f"{role}.dex"
        path.write_bytes(dex)
        print(f"{role}: {path} file_size={len(dex)} sha256={hashlib.sha256(dex).hexdigest()}")
        print(f"  kind={kind} formals={params} parameter_words={facts['parameter_words']}"
              f" proto_len={facts['proto_length']}"
              f" f_code_off={('0x%x' % facts['f_code_off']) if facts['f_code_off'] else '0'}"
              f" registers_size={facts['registers_size']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
