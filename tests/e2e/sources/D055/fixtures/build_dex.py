#!/usr/bin/env python3
"""D055 case-local DEX builder: oversized counts validated before allocation.

Builds one reviewed 035 owner and two independent single-fault variants.

Original (valid) input, class Lfixtures/Case;:
  - canonical Object constructor;
  - static f()I with registers_size=2, ins/outs=0, tries=1 and six code units:

        0  0x1155  const/4 v0, 5        (11n)
        1  0x000f  return v0            (11x)   <- protected region [0,2)
        2  0x0d01  move-exception v1    (11x)   <- typed handler target
        3  0x0013  const/16 v1, 7       (21s)
        4  0x0007
        5  0x010f  return v1            (11x)

    one try_item {start=0, count=2, handler_off=0} and one encoded typed handler
    with size=1, type Ljava/lang/RuntimeException;, addr 2, no catch-all.

Variant "insns-count": only the code_item insns_size u32 becomes 2147483647.
Variant "handler-count": only the encoded typed-handler count becomes the
positive SLEB32 2147483647; because that encoding is five bytes instead of one,
the handler record grows and every dependent offset/file_size/map is rebuilt
before the single count fault is applied.

Both variants keep every register word and reference in range, so the only
malformed field is the patched count.

Outputs go to .cache/e2e/D055/.  Standard library only, no network, no
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

OVERFLOW_COUNT = 2147483647

F_CODE_UNITS = [
    0x5012,  # const/4 v0, 5     (11n: op 0x12, A=0, B=5 -> (5<<12)|(0<<8)|0x12)
    0x000f,  # return v0         (11x: op 0x0f, AA=0); normal path, inside the try range
    0x010d,  # move-exception v1 (11x: op 0x0d, AA=1); handler entry at addr 2
    0x0113,  # const/16 v1, 7    (21s: op 0x13, AA=1, literal word next); overwrites v1
    0x0007,  # literal word 7 for the const/16 above (not a standalone instruction)
    0x010f,  # return v1         (11x: op 0x0f, AA=1); returns the separately defined int 7
]
# The handler must return a separately defined int rather than the caught reference.
# const/16 therefore targets v1, the same register move-exception filled and return
# reads, so f()I returns the int 7 on both paths and never returns a RuntimeException.
F_REGISTERS_SIZE = 2
# try_item.handler_off counts from the start of encoded_catch_handler_list, whose
# first byte is the ULEB list size, so the single encoded_catch_handler begins at
# offset 1 rather than 0.
F_TRY_ITEM = {"start_addr": 0, "insn_count": 2, "handler_off": 1}
F_HANDLER_TYPE = "Ljava/lang/RuntimeException;"
F_HANDLER_ADDR = 2


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


def sleb(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7f
        value >>= 7
        if (value == 0 and not byte & 0x40) or (value == -1 and byte & 0x40):
            out.append(byte)
            return bytes(out)
        out.append(byte | 0x80)


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


def code_item(registers: int, ins: int, outs: int, units: list[int],
              tries: list[dict] | None, handler_list: bytes | None) -> bytearray:
    tries = tries or []
    blob = bytearray(struct.pack("<HHHHII", registers, ins, outs, len(tries), 0, len(units)))
    for unit in units:
        blob += struct.pack("<H", unit)
    if len(units) % 2 and tries:
        blob += struct.pack("<H", 0)
    for item in tries:
        blob += struct.pack("<IHH", item["start_addr"], item["insn_count"], item["handler_off"])
    if tries:
        assert handler_list is not None
        blob += handler_list
    return blob


def handler_list(typed_count_encoding: bytes, include_typed_pair: bool) -> bytes:
    """encoded_catch_handler_list with one encoded_catch_handler."""
    blob = bytearray(uleb(1))
    blob += typed_count_encoding
    if include_typed_pair:
        blob += uleb(F_HANDLER_TYPE_INDEX) + uleb(F_HANDLER_ADDR)
    return bytes(blob)


def build(typed_count_encoding: bytes, include_typed_pair: bool,
          insns_size_override: int | None) -> tuple[bytearray, dict]:
    global F_HANDLER_TYPE_INDEX
    strings = ["()V", "<init>", "Case.smali", "I", "Lfixtures/Case;",
               "Ljava/lang/Object;", "Ljava/lang/RuntimeException;", "V", "f"]
    string_idx = {value: index for index, value in enumerate(strings)}
    types = ["I", "Lfixtures/Case;", "Ljava/lang/Object;",
             "Ljava/lang/RuntimeException;", "V"]
    type_idx = {value: index for index, value in enumerate(types)}
    F_HANDLER_TYPE_INDEX = type_idx[F_HANDLER_TYPE]
    # proto_ids sort by return_type_idx, then by parameter type_idx list. Type
    # indices are I=0 and V=4, so ()I precedes ()V.
    protos = [("I", "I"), ("V", "V")]
    proto_idx = {shorty: index for index, (shorty, _) in enumerate(protos)}
    # method_ids sort by class_idx, then name_idx, then proto_idx. Case (type 1)
    # precedes Object (type 2); within Case, name "<init>" (string 1) precedes
    # "f" (string 8).
    methods = [
        ("Lfixtures/Case;", "<init>", proto_idx["V"]),   # 0
        ("Lfixtures/Case;", "f", proto_idx["I"]),        # 1
        ("Ljava/lang/Object;", "<init>", proto_idx["V"]),  # 2
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
    ctor_units = [0x1070, object_ctor_method_idx, 0x0000, 0x000e]
    data += code_item(1, 1, 1, ctor_units, None, None)

    f_code_off = align(data, data_off, 4)
    data += code_item(
        F_REGISTERS_SIZE,
        0,
        0,
        F_CODE_UNITS,
        [F_TRY_ITEM],
        handler_list(typed_count_encoding, include_typed_pair),
    )

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

    if insns_size_override is not None:
        struct.pack_into("<I", dex, f_code_off + 12, insns_size_override)

    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)

    anchors = {
        "f_code_off": f_code_off,
        "f_insns_size_off": f_code_off + 12,
        "f_try_item_off": f_code_off + 16 + 2 * len(F_CODE_UNITS),
        "f_handler_list_off": f_code_off + 16 + 2 * len(F_CODE_UNITS) + 8,
        "f_handler_size_off": f_code_off + 16 + 2 * len(F_CODE_UNITS) + 8 + 1,
        "class_data_off": class_data_off,
        "map_off": map_off,
        "file_size": file_size,
    }
    return dex, anchors


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D055")
    out_dir.mkdir(parents=True, exist_ok=True)

    original, original_anchors = build(sleb(1), True, None)
    (out_dir / "original.dex").write_bytes(original)
    original_hash = hashlib.sha256(original).hexdigest()

    insns_variant, insns_anchors = build(sleb(1), True, OVERFLOW_COUNT)
    (out_dir / "insns-count.dex").write_bytes(insns_variant)
    insns_hash = hashlib.sha256(insns_variant).hexdigest()

    handler_variant, handler_anchors = build(sleb(OVERFLOW_COUNT), True, None)
    (out_dir / "handler-count.dex").write_bytes(handler_variant)
    handler_hash = hashlib.sha256(handler_variant).hexdigest()

    print(f"original file_size={original_anchors['file_size']} sha256={original_hash}")
    print(f"  f_code_off=0x{original_anchors['f_code_off']:x}"
          f" insns_size_off=0x{original_anchors['f_insns_size_off']:x}"
          f" try_item_off=0x{original_anchors['f_try_item_off']:x}"
          f" handler_list_off=0x{original_anchors['f_handler_list_off']:x}"
          f" handler_size_off=0x{original_anchors['f_handler_size_off']:x}")
    print(f"  class_data_off=0x{original_anchors['class_data_off']:x}"
          f" map_off=0x{original_anchors['map_off']:x}")

    insns_changed = [i for i in range(len(original)) if original[i] != insns_variant[i]]
    remaining_insns = insns_anchors["file_size"] - (insns_anchors["f_insns_size_off"] + 4)
    print(f"insns-count file_size={insns_anchors['file_size']} sha256={insns_hash}")
    print(f"  field=insns_size u32 at 0x{insns_anchors['f_insns_size_off']:x}"
          f" original=6 patched={OVERFLOW_COUNT}")
    print(f"  required_insn_bytes={OVERFLOW_COUNT * 2} remaining_after_field={remaining_insns}"
          f" overrun={OVERFLOW_COUNT * 2 > remaining_insns}")
    print(f"  changed_bytes={len(insns_changed)} at={[hex(i) for i in insns_changed]}"
          f" (4 patched bytes plus header checksum/signature)")

    remaining_handler = handler_anchors["file_size"] - handler_anchors["f_handler_size_off"] - 5
    print(f"handler-count file_size={handler_anchors['file_size']} sha256={handler_hash}")
    print(f"  field=encoded_catch_handler.size sleb32 at 0x{handler_anchors['f_handler_size_off']:x}"
          f" original=1 patched={OVERFLOW_COUNT}")
    print(f"  handler_record_growth=4 bytes; every later offset/file_size/map was rebuilt")
    print(f"  required_pair_bytes_min={OVERFLOW_COUNT * 2} remaining_after_sleb={remaining_handler}"
          f" overrun={OVERFLOW_COUNT * 2 > remaining_handler}")
    print("  rebuilt anchors:"
          f" try_item_off=0x{handler_anchors['f_try_item_off']:x}"
          f" handler_list_off=0x{handler_anchors['f_handler_list_off']:x}"
          f" class_data_off=0x{handler_anchors['class_data_off']:x}"
          f" map_off=0x{handler_anchors['map_off']:x}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
