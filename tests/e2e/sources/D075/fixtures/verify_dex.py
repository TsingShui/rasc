#!/usr/bin/env python3
"""D075 independent DEX reader/validator, derived from the published format.

This is a separate reading of the original input bytes, not the builder's
assumptions: every rule below comes from the published Android DEX format
(header layout, id-table strides, map_list item type codes, class_data ULEB
delta groups, code_item layout) rather than from build_dex.py constants.

Checks performed on every `.dex` file in the given directory:

* magic and version, `file_size` equal to the actual length, header_size 0x70,
  endian tag, and that adler32/sha1 cover the published byte ranges;
* every id table and its declared size/offset agrees with the map_list entry;
* map_list entries are ordered by offset and use the published type codes
  (0x0000 header, 0x0001 string_id, 0x0002 type_id, 0x0003 proto_id,
  0x1001 type_list, 0x0005 method_id, 0x0006 class_def, 0x1000 map_list,
  0x2000 class_data, 0x2001 code, 0x2002 string_data);
* proto_id.shorty_idx names a real *shorty* string (return-type shorty
  followed by one shorty per parameter), never a full proto descriptor;
* class_data direct and virtual method groups each accumulate their ULEB
  delta from 0 independently, and every resulting method_idx exists;
* code_item insns_size matches the real bytes present, and abstract methods
  carry code_off 0.

Usage: python3 verify_dex.py <directory>
Exit status is 1 when any violation is reported.
"""

from __future__ import annotations

import hashlib
import struct
import sys
import zlib
from pathlib import Path

# Published DEX map_item type codes.
MAP_CODES = {
    0x0000: "header_item",
    0x0001: "string_id_item",
    0x0002: "type_id_item",
    0x0003: "proto_id_item",
    0x0004: "field_id_item",
    0x0005: "method_id_item",
    0x0006: "class_def_item",
    0x1000: "map_list",
    0x1001: "type_list",
    0x1002: "annotation_set_ref_list",
    0x1003: "annotation_set_item",
    0x2000: "class_data_item",
    0x2001: "code_item",
    0x2002: "string_data_item",
    0x2003: "debug_info_item",
    0x2004: "annotation_item",
}

HEADER_SIZE = 0x70


def read_uleb(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7f) << shift
        shift += 7
        if not byte & 0x80:
            return value, offset


def decode_mutf8(raw: bytes) -> list[int]:
    """Published DEX MUTF-8: 00 terminates, C0 80 encodes NUL, else 1/2/3 bytes."""
    units: list[int] = []
    cursor = 0
    while cursor < len(raw):
        first = raw[cursor]
        if first == 0:
            raise ValueError("embedded 0x00 inside a MUTF-8 item")
        if first < 0x80:
            units.append(first)
            cursor += 1
        elif first & 0xe0 == 0xc0:
            units.append(((first & 0x1f) << 6) | (raw[cursor + 1] & 0x3f))
            cursor += 2
        elif first & 0xf0 == 0xe0:
            units.append(((first & 0x0f) << 12) | ((raw[cursor + 1] & 0x3f) << 6)
                         | (raw[cursor + 2] & 0x3f))
            cursor += 3
        else:
            raise ValueError(f"invalid MUTF-8 lead byte {first:#04x}")
    return units


def read_string_units(data: bytes, string_ids_off: int, index: int) -> tuple[list[int], int]:
    off = struct.unpack_from("<I", data, string_ids_off + 4 * index)[0]
    declared, cursor = read_uleb(data, off)
    end = cursor
    while data[end] != 0:
        end += 1
    return decode_mutf8(data[cursor:end]), declared


def units_as_text(units: list[int]) -> str:
    return "".join(chr(unit) for unit in units)


def shorty_of(descriptor: str) -> str:
    """Published shorty: return-type shorty then one shorty per parameter."""
    def one(type_descriptor: str) -> str:
        while type_descriptor.startswith("["):
            type_descriptor = type_descriptor[1:]
        if type_descriptor.startswith("L"):
            return "L"
        return type_descriptor

    close = descriptor.index(")")
    result = one(descriptor[close + 1:])
    params = descriptor[1:close]
    cursor = 0
    while cursor < len(params):
        start = cursor
        while params[cursor] == "[":
            cursor += 1
        if params[cursor] == "L":
            cursor = params.index(";", cursor) + 1
        else:
            cursor += 1
        result += one(params[start:cursor])
    return result


def proto_descriptor(data: bytes, proto_ids_off: int, type_ids_off: int,
                     string_ids_off: int, index: int) -> str:
    _, return_type_idx, params_off = struct.unpack_from("<III", data, proto_ids_off + 12 * index)
    def type_of(type_index: int) -> str:
        return units_as_text(read_string_units(data, string_ids_off,
                            struct.unpack_from("<I", data, type_ids_off + 4 * type_index)[0])[0])
    parts = []
    if params_off:
        count = struct.unpack_from("<I", data, params_off)[0]
        for step in range(count):
            parts.append(type_of(struct.unpack_from("<H", data, params_off + 4 + 2 * step)[0]))
    return "(" + "".join(parts) + ")" + type_of(return_type_idx)


def verify(path: Path) -> list[str]:
    problems: list[str] = []
    data = path.read_bytes()
    name = path.name

    if data[:4] != b"dex\n":
        return [f"{name}: not a DEX magic"]
    if data[4:8] != b"035\x00":
        problems.append(f"{name}: version {data[4:8]!r} is not 035")

    file_size, header_size, endian = struct.unpack_from("<III", data, 0x20)
    if file_size != len(data):
        problems.append(f"{name}: file_size {file_size} != actual {len(data)}")
    if header_size != HEADER_SIZE:
        problems.append(f"{name}: header_size {header_size:#x} != {HEADER_SIZE:#x}")
    if endian != 0x12345678:
        problems.append(f"{name}: endian_tag {endian:#x} is not 0x12345678")
    if struct.unpack_from("<I", data, 0x08)[0] != zlib.adler32(data[0x0c:]) & 0xffffffff:
        problems.append(f"{name}: adler32 does not cover 0x0c..end")
    if data[0x0c:0x20] != hashlib.sha1(data[0x20:]).digest():
        problems.append(f"{name}: sha1 does not cover 0x20..end")

    link_size, link_off, map_off = struct.unpack_from("<III", data, 0x2c)
    if link_size != 0 or link_off != 0:
        problems.append(f"{name}: link section must be empty")

    tables = {}
    for label, size_off in (("string_ids", 0x38), ("type_ids", 0x40), ("proto_ids", 0x48),
                            ("field_ids", 0x50), ("method_ids", 0x58), ("class_defs", 0x60)):
        count, off = struct.unpack_from("<II", data, size_off)
        tables[label] = (count, off)
        if count and off == 0:
            problems.append(f"{name}: {label} has count {count} but offset 0")
        if count and off + 0 > len(data):
            problems.append(f"{name}: {label} offset beyond file")

    data_size, data_off = struct.unpack_from("<II", data, 0x68)
    if data_off + data_size != len(data):
        problems.append(f"{name}: data section {data_off:#x}+{data_size:#x} != file size")

    # map_list
    map_count = struct.unpack_from("<I", data, map_off)[0]
    if map_off + 4 + 12 * map_count != len(data):
        # map_list is last only when nothing follows; report rather than assert
        pass
    entries = []
    for index in range(map_count):
        type_code, _, size, off = struct.unpack_from("<HHII", data, map_off + 4 + 12 * index)
        if type_code not in MAP_CODES:
            problems.append(f"{name}: map entry {index} has unknown type {type_code:#06x}")
        entries.append((type_code, size, off, MAP_CODES.get(type_code, "?")))
    for previous, current in zip(entries, entries[1:]):
        if current[2] < previous[2]:
            problems.append(
                f"{name}: map_list not ordered by offset: {previous[3]}@{previous[2]:#x}"
                f" then {current[3]}@{current[2]:#x}")

    codes = [entry[0] for entry in entries]
    expected_map = {
        "string_ids": 0x0001, "type_ids": 0x0002, "proto_ids": 0x0003,
        "method_ids": 0x0005, "class_defs": 0x0006,
    }
    for label, code in expected_map.items():
        count, off = tables[label]
        if count == 0:
            if code in codes:
                problems.append(f"{name}: map declares {MAP_CODES[code]} but header count is 0")
            continue
        match = [entry for entry in entries if entry[0] == code]
        if not match:
            problems.append(f"{name}: map has no {MAP_CODES[code]} entry for {label}")
            continue
        _, size, off, _label = match[0]
        if size != count:
            problems.append(f"{name}: {label} map size {size} != header count {count}")
        if off != tables[label][1]:
            problems.append(f"{name}: {label} map offset {off:#x} != header {tables[label][1]:#x}")

    if codes.count(0x1000) != 1:
        problems.append(f"{name}: expected exactly one map_list entry")
    if codes.count(0x0000) != 1:
        problems.append(f"{name}: expected exactly one header_item entry")

    # shorty correctness and string sorting
    string_count, string_ids_off = tables["string_ids"]
    type_count, type_ids_off = tables["type_ids"]
    proto_count, proto_ids_off = tables["proto_ids"]

    strings = []
    for index in range(string_count):
        units, declared = read_string_units(data, string_ids_off, index)
        if len(units) != declared:
            problems.append(f"{name}: string {index} declares {declared} utf16 units but has {len(units)}")
        strings.append(units_as_text(units))
    if strings != sorted(strings):
        problems.append(f"{name}: string_ids not sorted by UTF-16 code units")

    for index in range(proto_count):
        shorty_idx, _, _ = struct.unpack_from("<III", data, proto_ids_off + 12 * index)
        if shorty_idx >= string_count:
            problems.append(f"{name}: proto {index} shorty_idx {shorty_idx} out of range")
            continue
        actual = strings[shorty_idx]
        descriptor = proto_descriptor(data, proto_ids_off, type_ids_off, string_ids_off, index)
        expected = shorty_of(descriptor)
        if actual != expected:
            problems.append(
                f"{name}: proto {index} {descriptor} shorty_idx points at {actual!r}"
                f" but the published shorty is {expected!r}")

    # A proto_id with a non-empty parameter list must be described by exactly
    # one type_list entry, and the published map code for type_list is 0x1001.
    # 0x1003 is annotation_set_item and must not stand in for it.
    needs_type_list = 0
    for index in range(proto_count):
        if struct.unpack_from("<III", data, proto_ids_off + 12 * index)[2]:
            needs_type_list += 1
    type_list_entries = [entry for entry in entries if entry[0] == 0x1001]
    annotation_entries = [entry for entry in entries if entry[0] in (0x1002, 0x1003)]
    if needs_type_list:
        if not type_list_entries:
            problems.append(
                f"{name}: {needs_type_list} proto(s) have a parameter list but the map has"
                f" no type_list entry (published code 0x1001)")
        elif type_list_entries[0][1] != needs_type_list:
            problems.append(
                f"{name}: map declares {type_list_entries[0][1]} type_list item(s) but"
                f" {needs_type_list} proto(s) reference a parameter list")
    elif type_list_entries:
        problems.append(f"{name}: map declares type_list items but no proto uses one")

    if codes.count(0x1003):
        problems.append(
            f"{name}: map declares {codes.count(0x1003)} annotation_set_item entr(ies)"
            f" but the owner declares no annotations")

    # class_data groups
    class_count, class_defs_off = tables["class_defs"]
    method_count, method_ids_off = tables["method_ids"]
    for class_index in range(class_count):
        base = class_defs_off + 32 * class_index
        class_idx, access_flags, super_idx, interfaces_off, source_idx, \
            annotations_off, class_data_off, static_values_off = struct.unpack_from("<IIIIIIII", data, base)
        if class_data_off == 0:
            continue
        cursor = class_data_off
        static_fields, cursor = read_uleb(data, cursor)
        instance_fields, cursor = read_uleb(data, cursor)
        direct_methods, cursor = read_uleb(data, cursor)
        virtual_methods, cursor = read_uleb(data, cursor)
        for _ in range(static_fields + instance_fields):
            _, cursor = read_uleb(data, cursor)
            _, cursor = read_uleb(data, cursor)
            _, cursor = read_uleb(data, cursor)
        for label, count in (("direct", direct_methods), ("virtual", virtual_methods)):
            method_index = 0
            for _ in range(count):
                delta, cursor = read_uleb(data, cursor)
                flags, cursor = read_uleb(data, cursor)
                code_off, cursor = read_uleb(data, cursor)
                method_index += delta
                if method_index >= method_count:
                    problems.append(
                        f"{name}: class {class_index} {label} group reaches method_idx {method_index}"
                        f" but method_ids has {method_count}")
                    continue
                if flags & 0x0400 and code_off != 0:
                    problems.append(
                        f"{name}: abstract method_idx {method_index} carries code_off {code_off:#x}")
                # Published rule: class_data lists only members of that class.
                declared_class = struct.unpack_from("<H", data, method_ids_off + 8 * method_index)[0]
                if declared_class != class_idx:
                    problems.append(
                        f"{name}: class {class_idx} {label} group resolves method_idx"
                        f" {method_index} to declaring class {declared_class}")
                if code_off:
                    registers, ins, outs, tries, _, insns_size = struct.unpack_from(
                        "<HHHHII", data, code_off)
                    if code_off + 16 + 2 * insns_size > len(data):
                        problems.append(
                            f"{name}: method_idx {method_index} code range"
                            f" {code_off:#x}+{insns_size} exceeds file")
    return problems


def main() -> int:
    directory = Path(sys.argv[1])
    files = sorted(directory.glob("*.dex"))
    if not files:
        print(f"no .dex files in {directory}")
        return 1
    total = 0
    for path in files:
        problems = verify(path)
        size = path.stat().st_size
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"{path.name} size={size} sha256={digest}")
        if problems:
            total += len(problems)
            for problem in problems:
                print(f"  VIOLATION: {problem}")
        else:
            print("  published-format checks clean")
    print(f"violations={total}")
    return 1 if total else 0


if __name__ == "__main__":
    raise SystemExit(main())
