#!/usr/bin/env python3
"""D056 case-local DEX builder: branch into the middle of an instruction.

Builds the reviewed 035 baseline and then the single-field mutation.

Baseline static f()I: registers_size=1, ins/outs/tries=0, five code units

    unit 0..2  0x0014 0x0007 0x0000   const v0, 7        (31i, 3 words)
    unit 3     0xfd28                 goto -3            (10t, target unit 0)
    unit 4     0x000f                 return v0          (unreachable, format-valid)

Mutation: change only the goto delta from -3 (0xfd) to -2 (0xfe), i.e. the low
opcode unit becomes 0xfe28 and the target is unit 1, the operand word of the
multiword const. Nothing else in the file changes; only the header checksum and
signature are recomputed over the mutated bytes.

Outputs go to .cache/e2e/D056/.  Standard library only, no network, no
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

BASELINE_CODE_UNITS = [0x0014, 0x0007, 0x0000, 0xfd28, 0x000f]
MUTATED_CODE_UNIT_INDEX = 3
MUTATED_CODE_UNIT = 0xfe28


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


def build(code_units: list[int]) -> bytearray:
    strings = ["()V", "<init>", "Case.smali", "I", "Lfixtures/Case;",
               "Ljava/lang/Object;", "V", "f"]
    string_idx = {value: index for index, value in enumerate(strings)}
    types = ["I", "Lfixtures/Case;", "Ljava/lang/Object;", "V"]
    type_idx = {value: index for index, value in enumerate(types)}
    # proto_ids sort by return_type_idx, then by parameter type_idx list. Type
    # indices are I=0 and V=3, so ()I precedes ()V.
    protos = [("I", "I"), ("V", "V")]
    proto_idx = {shorty: index for index, (shorty, _) in enumerate(protos)}
    # method_ids sort by class_idx, then name_idx, then proto_idx. Case (type 1)
    # precedes Object (type 2); within Case, name "<init>" (string 1) precedes
    # "f" (string 7).
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
    data += code_item(1, 1, 1, ctor_units)

    f_code_off = align(data, data_off, 4)
    data += code_item(1, 0, 0, code_units)

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
    return dex


def rehash(dex: bytearray) -> None:
    file_size = struct.unpack_from("<I", dex, 0x20)[0]
    dex[0x0c:0x20] = hashlib.sha1(dex[0x20:file_size]).digest()
    struct.pack_into("<I", dex, 0x08, zlib.adler32(dex[0x0c:file_size]) & 0xffffffff)


def main() -> int:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".cache/e2e/D056")
    out_dir.mkdir(parents=True, exist_ok=True)

    baseline = build(BASELINE_CODE_UNITS)
    baseline_path = out_dir / "baseline.dex"
    baseline_path.write_bytes(baseline)
    baseline_hash = hashlib.sha256(baseline).hexdigest()

    # Locate the mutation anchor independently: walk the class_data to the code_item.
    class_defs_off = struct.unpack_from("<I", baseline, 0x64)[0]
    class_data_off = struct.unpack_from("<I", baseline, class_defs_off + 24)[0]

    def read_uleb(offset: int) -> tuple[int, int]:
        value = 0
        shift = 0
        while True:
            byte = baseline[offset]
            offset += 1
            value |= (byte & 0x7f) << shift
            shift += 7
            if not byte & 0x80:
                return value, offset

    cursor = class_data_off
    counts = []
    for _ in range(4):
        value, cursor = read_uleb(cursor)
        counts.append(value)
    method_index = 0
    direct_code_offsets = []
    for _ in range(counts[2]):
        diff, cursor = read_uleb(cursor)
        _flags, cursor = read_uleb(cursor)
        code_off, cursor = read_uleb(cursor)
        method_index += diff
        direct_code_offsets.append(code_off)
    f_code_off = direct_code_offsets[1]

    anchor = f_code_off + 16 + 2 * MUTATED_CODE_UNIT_INDEX
    mutated = bytearray(baseline)
    original_unit = struct.unpack_from("<H", mutated, anchor)[0]
    assert original_unit == BASELINE_CODE_UNITS[MUTATED_CODE_UNIT_INDEX]
    struct.pack_into("<H", mutated, anchor, MUTATED_CODE_UNIT)
    rehash(mutated)
    mutated_path = out_dir / "mutated.dex"
    mutated_path.write_bytes(mutated)
    mutated_hash = hashlib.sha256(mutated).hexdigest()

    changed = [index for index in range(len(baseline)) if baseline[index] != mutated[index]]
    print(f"baseline={baseline_path} file_size={len(baseline)} sha256={baseline_hash}")
    print(f"mutated={mutated_path} file_size={len(mutated)} sha256={mutated_hash}")
    print(f"f_code_off=0x{f_code_off:x} insns_size={len(BASELINE_CODE_UNITS)}")
    print(f"mutation_anchor=0x{anchor:x} field=code_unit[{MUTATED_CODE_UNIT_INDEX}]"
          f" original=0x{original_unit:04x} mutated=0x{MUTATED_CODE_UNIT:04x}")
    print(f"goto_delta original=-3 target=unit0 mutated=-2 target=unit1"
          f" (unit1 is the 0x0007 operand word of the 31i const)")
    print(f"changed_bytes={len(changed)} at={[hex(i) for i in changed]}")
    print("note: the only semantic change is the goto delta; the remaining changed bytes"
          " are the header checksum at 0x08 and the signature at 0x0c..0x1f")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
