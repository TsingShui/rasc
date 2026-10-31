#!/usr/bin/env python3
"""D065 case-local 041 container builder: two logical members, shared tables.

Writes .cache/e2e/D065/container.dex: one physical 041 container with two
logical members whose class/method table indices are numerically identical
because the string/type/proto/method tables are physically shared, while each
member has its own class_defs, class_data and code so that Case.f()I returns 11
in logical member 0 and 22 in logical member 1.

Physical layout (all offsets absolute from the container start):

    0x000  header of logical member 0        (0x78 bytes, file_size = 0x78)
    0x078  header of logical member 1        (0x78 bytes, file_size = total-0x78)
    0x0f0  string_ids   (shared by both members)
           type_ids     (shared)
           proto_ids    (shared)
           method_ids   (shared)
           string_data  (shared)
           member 0: class_defs, class_data, code_item <init>, code_item f(), map_list
           member 1: class_defs, class_data, code_item <init>, code_item f(), map_list

Every section offset a member declares is at or after its own header, which is
the ALGORITHMS 1.1 rule for 041 ("MAY reference data of a later logical member
and MUST NOT reference data before the current header"). The shared tables sit
after both headers so both members may reference them: member 0 forward, member
1 at its own position.

Both headers declare container_size = physical length and header_offset = their
actual position; the two headers are chained by file_size and the chain ends
exactly at container_size. The header checksum/signature are computed over each
member's own slice and are advisory original fields, not a memory-safety proof.

Standard library only, no network, no subprocess.
Usage: python3 build_dex041.py [output-path]
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

HEADER_SIZE_041 = 0x78

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

STRINGS = ["()V", "<init>", "Case.smali", "I", "Lfixtures/Case;",
           "Ljava/lang/Object;", "V", "f"]
STRING_IDX = {value: index for index, value in enumerate(STRINGS)}
TYPES = ["I", "Lfixtures/Case;", "Ljava/lang/Object;", "V"]
TYPE_IDX = {value: index for index, value in enumerate(TYPES)}
# proto_ids sort by return_type_idx, then by parameter type_idx list. Type
# indices are I=0 and V=3, so ()I precedes ()V.
PROTOS = [("I", "I"), ("V", "V")]
PROTO_IDX = {shorty: index for index, (shorty, _) in enumerate(PROTOS)}
# method_ids sort by class_idx, then name_idx, then proto_idx. Case (type 1)
# precedes Object (type 2); within Case, name "<init>" (string 1) precedes
# "f" (string 7).
METHODS = [
    ("Lfixtures/Case;", "<init>", PROTO_IDX["V"]),     # 0
    ("Lfixtures/Case;", "f", PROTO_IDX["I"]),          # 1
    ("Ljava/lang/Object;", "<init>", PROTO_IDX["V"]),  # 2
]
OBJECT_CTOR_METHOD_IDX = 2

MAP_ENTRY_COUNT = 10

LOGICAL_MEMBERS = [
    {"logical_ordinal": 0, "f_return": 11},
    {"logical_ordinal": 1, "f_return": 22},
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


def code_item(registers: int, ins: int, outs: int, units: list[int]) -> bytes:
    blob = bytearray(struct.pack("<HHHHII", registers, ins, outs, 0, 0, len(units)))
    for unit in units:
        blob += struct.pack("<H", unit)
    return bytes(blob)


class Container:
    def __init__(self) -> None:
        self.blob = bytearray()

    def offset(self) -> int:
        return len(self.blob)

    def pad_to(self, alignment: int) -> None:
        while len(self.blob) % alignment:
            self.blob.append(0)

    def append(self, data: bytes) -> int:
        start = len(self.blob)
        self.blob += data
        return start


def build() -> tuple[bytes, dict]:
    container = Container()
    header_offsets = []
    for _ in LOGICAL_MEMBERS:
        header_offsets.append(container.append(bytes(HEADER_SIZE_041)))

    container.pad_to(4)
    string_ids_off = container.append(bytes(4 * len(STRINGS)))
    type_ids_off = container.append(bytes(4 * len(TYPES)))
    proto_ids_off = container.append(bytes(12 * len(PROTOS)))
    method_ids_off = container.append(bytes(8 * len(METHODS)))

    string_data_offsets = []
    for value in STRINGS:
        string_data_offsets.append(container.append(uleb(len(utf16_units(value)))))
        container.append(mutf8(value) + b"\x00")
    string_data_first = string_data_offsets[0]

    # Per-member items. Each member's own sections start at or after its header.
    member_items = []
    for index, member in enumerate(LOGICAL_MEMBERS):
        container.pad_to(4)
        class_defs_off = container.append(bytes(32))
        class_data_off = container.append(b"")
        container.blob = container.blob[:class_data_off]

        # invoke-direct {p0}, Ljava/lang/Object;-><init>()V: 35c count 1, G=v0.
        ctor_code_units = [0x1070, OBJECT_CTOR_METHOD_IDX, 0x0000, 0x000e]
        f_code_units = [
            0x0013, member["f_return"],  # const/16 v0, <return>
            0x000f,                      # return v0
        ]
        container.pad_to(4)
        ctor_code_off = container.append(code_item(1, 1, 1, ctor_code_units))
        container.pad_to(4)
        f_code_off = container.append(code_item(1, 0, 0, f_code_units))

        class_data = bytearray(uleb(0) + uleb(0) + uleb(2) + uleb(0))
        class_data += uleb(0) + uleb(ACC_CONSTRUCTOR) + uleb(ctor_code_off)
        class_data += uleb(1) + uleb(ACC_PUBLIC | ACC_STATIC) + uleb(f_code_off)
        class_data_off = container.append(bytes(class_data))

        container.pad_to(4)
        map_off = container.append(bytes(4 + 12 * MAP_ENTRY_COUNT))

        member_items.append(
            {
                "logical_ordinal": index,
                "header_offset": header_offsets[index],
                "f_return": member["f_return"],
                "class_defs_off": class_defs_off,
                "class_data_off": class_data_off,
                "ctor_code_off": ctor_code_off,
                "f_code_off": f_code_off,
                "f_code_units": f_code_units,
                "map_off": map_off,
            }
        )

    total = len(container.blob)
    assert total % 4 == 0, total
    assert header_offsets[1] == HEADER_SIZE_041

    blob = container.blob

    # Shared tables.
    for index, offset in enumerate(string_data_offsets):
        struct.pack_into("<I", blob, string_ids_off + 4 * index, offset)
    for index, descriptor in enumerate(TYPES):
        struct.pack_into("<I", blob, type_ids_off + 4 * index, STRING_IDX[descriptor])
    for index, (shorty, return_type) in enumerate(PROTOS):
        struct.pack_into("<III", blob, proto_ids_off + 12 * index,
                         STRING_IDX[shorty], TYPE_IDX[return_type], 0)
    for index, (owner, name, proto_index) in enumerate(METHODS):
        # method_id_item layout is class_idx:u16, proto_idx:u16, name_idx:u32.
        struct.pack_into("<HHI", blob, method_ids_off + 8 * index,
                         TYPE_IDX[owner], proto_index, STRING_IDX[name])

    for item in member_items:
        struct.pack_into("<IIIIIIII", blob, item["class_defs_off"],
                         TYPE_IDX["Lfixtures/Case;"], ACC_PUBLIC,
                         TYPE_IDX["Ljava/lang/Object;"], 0,
                         STRING_IDX["Case.smali"], 0,
                         item["class_data_off"], 0)
        # Map entries must follow physical offset order within one member's map.
        entries = [
            (TYPE_HEADER_ITEM, 1, item["header_offset"]),
            (TYPE_STRING_ID_ITEM, len(STRINGS), string_ids_off),
            (TYPE_TYPE_ID_ITEM, len(TYPES), type_ids_off),
            (TYPE_PROTO_ID_ITEM, len(PROTOS), proto_ids_off),
            (TYPE_METHOD_ID_ITEM, len(METHODS), method_ids_off),
            (TYPE_STRING_DATA_ITEM, len(STRINGS), string_data_first),
            (TYPE_CLASS_DEF_ITEM, 1, item["class_defs_off"]),
            (TYPE_CODE_ITEM, 2, item["ctor_code_off"]),
            (TYPE_CLASS_DATA_ITEM, 1, item["class_data_off"]),
            (TYPE_MAP_LIST, 1, item["map_off"]),
        ]
        entries.sort(key=lambda entry: entry[2])
        item["map_entries"] = entries
        struct.pack_into("<I", blob, item["map_off"], len(entries))
        cursor = item["map_off"] + 4
        for type_code, size, offset in entries:
            struct.pack_into("<HHII", blob, cursor, type_code, 0, size, offset)
            cursor += 12

    # Headers. file_size is the step to the next header; the last step ends at
    # container_size.
    for item in member_items:
        header = item["header_offset"]
        next_header = (
            header_offsets[item["logical_ordinal"] + 1]
            if item["logical_ordinal"] + 1 < len(member_items)
            else total
        )
        data_off = string_data_first if item["logical_ordinal"] == 0 else item["class_data_off"]
        blob[header:header + 8] = b"dex\n041\x00"
        struct.pack_into("<I", blob, header + 0x20, next_header - header)
        struct.pack_into("<I", blob, header + 0x24, HEADER_SIZE_041)
        struct.pack_into("<I", blob, header + 0x28, 0x12345678)
        struct.pack_into("<I", blob, header + 0x2c, 0)
        struct.pack_into("<I", blob, header + 0x30, 0)
        struct.pack_into("<I", blob, header + 0x34, item["map_off"])
        struct.pack_into("<II", blob, header + 0x38, len(STRINGS), string_ids_off)
        struct.pack_into("<II", blob, header + 0x40, len(TYPES), type_ids_off)
        struct.pack_into("<II", blob, header + 0x48, len(PROTOS), proto_ids_off)
        struct.pack_into("<II", blob, header + 0x50, 0, 0)
        struct.pack_into("<II", blob, header + 0x58, len(METHODS), method_ids_off)
        struct.pack_into("<II", blob, header + 0x60, 1, item["class_defs_off"])
        struct.pack_into("<II", blob, header + 0x68, total - data_off, data_off)
        struct.pack_into("<I", blob, header + 0x70, total)
        struct.pack_into("<I", blob, header + 0x74, header)
        # Advisory original fields over this member's own slice.
        blob[header + 0x0c:header + 0x20] = hashlib.sha1(
            bytes(blob[header + 0x20:header + next_header - header])
        ).digest()
        struct.pack_into(
            "<I", blob, header + 0x08,
            zlib.adler32(bytes(blob[header + 0x0c:header + next_header - header])) & 0xffffffff,
        )

    facts = {
        "totalSize": total,
        "headerOffsets": header_offsets,
        "stringIdsOff": string_ids_off,
        "typeIdsOff": type_ids_off,
        "protoIdsOff": proto_ids_off,
        "methodIdsOff": method_ids_off,
        "stringDataFirst": string_data_first,
        "members": member_items,
    }
    return bytes(blob), facts


def main() -> int:
    out_path = (
        Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D065/container.dex")
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    blob, facts = build()
    out_path.write_bytes(blob)
    digest = hashlib.sha256(blob).hexdigest()
    print(f"output={out_path} file_size={facts['totalSize']} sha256={digest}")
    print(f"shared string_ids_off=0x{facts['stringIdsOff']:x} count={len(STRINGS)}")
    print(f"shared type_ids_off=0x{facts['typeIdsOff']:x} count={len(TYPES)}")
    print(f"shared proto_ids_off=0x{facts['protoIdsOff']:x} count={len(PROTOS)}")
    print(f"shared method_ids_off=0x{facts['methodIdsOff']:x} count={len(METHODS)}")
    for item in facts["members"]:
        print(
            f"logical {item['logical_ordinal']}: header_offset=0x{item['header_offset']:x}"
            f" class_defs_off=0x{item['class_defs_off']:x}"
            f" class_data_off=0x{item['class_data_off']:x}"
            f" f_code_off=0x{item['f_code_off']:x}"
            f" f_code_units={[hex(u) for u in item['f_code_units']]}"
            f" map_off=0x{item['map_off']:x}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
