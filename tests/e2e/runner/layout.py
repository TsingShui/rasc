"""Independent published-format projection, including native 041 containers.

This tests-only reader imports no fixture builders or Diana code. 041 is read at
its original physical offsets; bytes are never overlaid as a different version.
Instruction semantics for ordinary DEX come separately from locked dexlib2.
"""
from __future__ import annotations
import hashlib
from pathlib import Path
import struct
import zlib


class FormatError(ValueError):
    def __init__(self, kind, offset):
        self.kind, self.offset = kind, offset
        super().__init__(f'{kind} at {offset:#x}')


class Dex:
    def __init__(self, data):
        self.data = data

    def check(self, offset, size, minimum=0):
        if offset < minimum or size < 0 or offset > len(self.data) or size > len(self.data) - offset:
            raise FormatError('range', offset)
        return offset

    def u16(self, offset):
        return struct.unpack_from('<H', self.data, self.check(offset, 2))[0]

    def u32(self, offset):
        return struct.unpack_from('<I', self.data, self.check(offset, 4))[0]

    def leb(self, offset, signed=False):
        value, start = 0, offset
        for shift in range(0, 35, 7):
            self.check(offset, 1)
            byte = self.data[offset]
            offset += 1
            value |= (byte & 127) << shift
            if byte < 128:
                if signed and byte & 64:
                    value -= 1 << (shift + 7)
                if not (-2147483648 <= value <= 2147483647 if signed else 0 <= value <= 0xffffffff):
                    raise FormatError('leb_overflow', start)
                return value, offset
        raise FormatError('unterminated_leb', start)

    def string(self, offset, minimum):
        self.check(offset, 1, minimum)
        count, cursor = self.leb(offset)
        units, start = [], cursor
        if count > len(self.data) - cursor:
            raise FormatError('string_count', offset)
        while True:
            self.check(cursor, 1)
            first = self.data[cursor]
            cursor += 1
            if not first:
                break
            if first < 128:
                unit = first
            elif first & 0xe0 == 0xc0:
                self.check(cursor, 1)
                second = self.data[cursor]; cursor += 1
                if second & 0xc0 != 0x80:
                    raise FormatError('mutf8_continuation', cursor - 1)
                unit = ((first & 31) << 6) | (second & 63)
                if unit and unit < 128:
                    raise FormatError('mutf8_overlong', cursor - 2)
            elif first & 0xf0 == 0xe0:
                self.check(cursor, 2)
                second, third = self.data[cursor:cursor + 2]; cursor += 2
                if second & 0xc0 != 0x80 or third & 0xc0 != 0x80:
                    raise FormatError('mutf8_continuation', cursor - 2)
                unit = ((first & 15) << 12) | ((second & 63) << 6) | (third & 63)
                if unit < 2048:
                    raise FormatError('mutf8_overlong', cursor - 3)
            else:
                raise FormatError('mutf8_lead', cursor - 1)
            units.append(unit)
        if len(units) != count:
            raise FormatError('mutf8_unit_count', offset)
        raw = b''.join(struct.pack('<H', x) for x in units)
        return {'offset':offset, 'units':units, 'mutf8_bytes':cursor - start - 1,
                'text':raw.decode('utf-16-le', errors='surrogatepass')}

    def encoded(self, cursor, depth=0):
        if depth > 64:
            raise FormatError('encoded_depth', cursor)
        self.check(cursor, 1)
        tag = self.data[cursor] & 31
        argument = self.data[cursor] >> 5
        cursor += 1
        if tag in (0x1c, 0x1d):
            if argument:
                raise FormatError('encoded_arg', cursor - 1)
            annotation_type = None
            if tag == 0x1d:
                annotation_type, cursor = self.leb(cursor)
            count, cursor = self.leb(cursor)
            if count > len(self.data) - cursor:
                raise FormatError('encoded_count', cursor)
            value = []
            for _ in range(count):
                name = None
                if tag == 0x1d:
                    name, cursor = self.leb(cursor)
                entry, cursor = self.encoded(cursor, depth + 1)
                value.append(entry if name is None else {'name_idx':name, 'value':entry})
            return {'tag':tag, 'value':value, 'annotation_type_idx':annotation_type}, cursor
        if tag in (0x1e, 0x1f):
            if argument > (1 if tag == 0x1f else 0):
                raise FormatError('encoded_arg', cursor - 1)
            return {'tag':tag, 'value':bool(argument) if tag == 0x1f else None}, cursor
        maximum = {0:0,2:1,3:1,4:3,6:7,0x10:3,0x11:7,0x15:3,0x16:3,0x17:3,0x18:3,0x19:3,0x1a:3,0x1b:3}.get(tag)
        if maximum is None or argument > maximum:
            raise FormatError('encoded_tag_or_arg', cursor - 1)
        size = argument + 1
        self.check(cursor, size)
        value = int.from_bytes(self.data[cursor:cursor + size], 'little', signed=tag in (0,2,4,6))
        if tag in (0x10,0x11):
            value <<= ((4 if tag == 0x10 else 8) - size) * 8
        return {'tag':tag, 'value':value}, cursor + size

    def code(self, offset, minimum):
        self.check(offset, 16, minimum)
        if offset % 4:
            raise FormatError('code_alignment', offset)
        registers, ins, outs, tries, debug, count = struct.unpack_from('<HHHHII', self.data, offset)
        if count > (len(self.data) - offset - 16) // 2:
            raise FormatError('code_insns_out_of_range', offset + 12)
        if ins > registers:
            raise FormatError('ins_exceeds_registers', offset)
        start, end = offset + 16, offset + 16 + 2 * count
        code = {'offset':offset,'insns_offset':start,'registers_size':registers,'ins_size':ins,
                'outs_size':outs,'insns_size':count,'debug_info_off':debug,
                'units':list(struct.unpack_from('<' + str(count) + 'H', self.data, start)), 'tries':[]}
        if not tries:
            return code
        cursor = end + (2 if count % 2 else 0)
        self.check(cursor, tries * 8)
        handler_start = cursor + tries * 8
        handler_count, pos = self.leb(handler_start)
        if handler_count > len(self.data) - pos:
            raise FormatError('handler_list_count_out_of_range', handler_start)
        handlers = {}
        for _ in range(handler_count):
            relative = pos - handler_start
            size_offset = pos
            size, pos = self.leb(pos, signed=True)
            if abs(size) > (len(self.data) - pos) // 2:
                raise FormatError('typed_handler_count_out_of_range', size_offset)
            items = []
            for _ in range(abs(size)):
                type_idx, pos = self.leb(pos)
                address, pos = self.leb(pos)
                items.append({'type_idx':type_idx, 'target':address})
            if size <= 0:
                address, pos = self.leb(pos)
                items.append({'type_idx':None,'target':address})
            handlers[relative] = items
        for index in range(tries):
            base = cursor + 8 * index
            first, length, handler_offset = struct.unpack_from('<IHH', self.data, base)
            if first + length > count or handler_offset not in handlers:
                raise FormatError('try_range_or_handler', base)
            items = handlers[handler_offset]
            if any(h['target'] >= count for h in items):
                raise FormatError('handler_target', base)
            code['tries'].append({'start':first, 'count':length, 'handlers':items})
        return code

    def member(self, header, ordinal):
        self.check(header, 112)
        magic = self.data[header:header + 8]
        if magic not in [b'dex\n035\0',b'dex\n037\0',b'dex\n038\0',b'dex\n039\0',b'dex\n040\0',b'dex\n041\0']:
            raise FormatError('magic', header)
        version = magic[4:7].decode('ascii')
        size, header_size, endian = (self.u32(header + x) for x in (32,36,40))
        expected_header = 120 if version == '041' else 112
        if header_size != expected_header or endian != 0x12345678 or size < header_size:
            raise FormatError('header_size_or_endian', header)
        self.check(header, size)
        if version == '041':
            self.check(header, 120)
            if self.u32(header + 112) != len(self.data) or self.u32(header + 116) != header:
                raise FormatError('container_size_or_header_offset', header)
        elif header or size != len(self.data):
            raise FormatError('file_size', header)
        # 041 integrity fields cover this member's slice, not its shared extents.
        if hashlib.sha1(self.data[header + 32:header + size]).digest() != self.data[header + 12:header + 32]:
            raise FormatError('sha1', header + 12)
        if zlib.adler32(self.data[header + 12:header + size]) & 0xffffffff != self.u32(header + 8):
            raise FormatError('adler32', header + 8)
        sections = {}
        for index, (name, width) in enumerate([('strings',4),('types',4),('protos',12),('fields',8),('methods',8),('classes',32)]):
            count = self.u32(header + 56 + 8 * index)
            offset = self.u32(header + 60 + 8 * index)
            if count:
                self.check(offset, count * width, header + header_size)
                if offset % 4:
                    raise FormatError('section_alignment', offset)
            elif offset:
                raise FormatError('empty_section_offset', offset)
            sections[name] = {'count':count, 'offset':offset}
        map_offset = self.u32(header + 52)
        self.check(map_offset, 4, header + header_size)
        count = self.u32(map_offset)
        self.check(map_offset + 4, count * 12)
        entries = []
        for i in range(count):
            kind, unused, amount, offset = struct.unpack_from('<HHII', self.data, map_offset + 4 + 12 * i)
            self.check(offset, 1, header)
            if unused or not amount:
                raise FormatError('map_entry', map_offset + 4 + 12 * i)
            entries.append({'type':kind, 'count':amount, 'offset':offset})
        if len({e['type'] for e in entries}) != count or [e['offset'] for e in entries] != sorted(e['offset'] for e in entries):
            raise FormatError('map_order_or_duplicate', map_offset)
        for index, name in enumerate(['strings','types','protos','fields','methods','classes'], 1):
            section = sections[name]
            matches = [e for e in entries if e['type'] == index]
            if section['count'] and matches != [{'type':index, **section}]:
                raise FormatError('map_section_disagrees', map_offset)
        strings = [self.string(self.u32(sections['strings']['offset'] + 4 * i), header + header_size) for i in range(sections['strings']['count'])]
        values = [s['units'] for s in strings]
        if any(a >= b for a,b in zip(values, values[1:])):
            raise FormatError('string_order_or_duplicate', sections['strings']['offset'])
        text = [s['text'] for s in strings]
        type_indices = [self.u32(sections['types']['offset'] + 4 * i) for i in range(sections['types']['count'])]
        if any(i >= len(text) for i in type_indices) or type_indices != sorted(set(type_indices)):
            raise FormatError('type_index_or_order', sections['types']['offset'])
        types = [text[i] for i in type_indices]
        protos = []
        for i in range(sections['protos']['count']):
            base = sections['protos']['offset'] + i * 12
            shorty, result, params = (self.u32(base + x) for x in (0,4,8))
            if shorty >= len(text) or result >= len(types):
                raise FormatError('proto_index', base)
            param_indices = []
            if params:
                self.check(params, 4, header + header_size)
                n = self.u32(params)
                self.check(params + 4, n * 2)
                param_indices = [self.u16(params + 4 + j * 2) for j in range(n)]
                if any(t >= len(types) for t in param_indices):
                    raise FormatError('parameter_type_idx', params)
            params_text = [types[j] for j in param_indices]
            short = ''.join(t if len(t) == 1 else 'L' for t in [types[result], *params_text])
            if text[shorty] != short:
                raise FormatError('shorty', base)
            protos.append({'parameters':params_text,'return':types[result], 'proto':'(' + ''.join(params_text) + ')' + types[result],
                           'sort_key':(result,tuple(param_indices))})
        keys = [p['sort_key'] for p in protos]
        if keys != sorted(set(keys)):
            raise FormatError('proto_order_or_duplicate', sections['protos']['offset'])
        methods, fields = [], []
        for name, target in [('methods',methods),('fields',fields)]:
            keys = []
            for i in range(sections[name]['count']):
                base = sections[name]['offset'] + i * 8
                owner, other, string_idx = struct.unpack_from('<HHI', self.data, base)
                limit = len(protos) if name == 'methods' else len(types)
                if owner >= len(types) or other >= limit or string_idx >= len(text):
                    raise FormatError('member_id_index', base)
                keys.append((owner,string_idx,other))
                item = {'owner':types[owner], 'name':text[string_idx]}
                if name == 'methods':
                    item.update({k:v for k,v in protos[other].items() if k != 'sort_key'})
                else:
                    item['type'] = types[other]
                target.append(item)
            if keys != sorted(set(keys)):
                raise FormatError('member_id_order_or_duplicate', sections[name]['offset'])
        classes = []
        for i in range(sections['classes']['count']):
            base = sections['classes']['offset'] + i * 32
            type_idx, flags, parent, interfaces, source_file, annotations, data, static_values = struct.unpack_from('<8I', self.data, base)
            if type_idx >= len(types) or parent != 0xffffffff and parent >= len(types):
                raise FormatError('class_type_idx', base)
            cls = {'class_def_idx':i,'descriptor':types[type_idx],'flags':flags,'super':None if parent == 0xffffffff else types[parent],
                   'class_data_off':data,'static_values_off':static_values,'methods':[],'fields':[],'encoded_values':[]}
            if static_values:
                self.check(static_values, 1, header + header_size)
                n, cursor = self.leb(static_values)
                if n > len(self.data) - cursor:
                    raise FormatError('static_values_count', static_values)
                for _ in range(n):
                    value, cursor = self.encoded(cursor)
                    cls['encoded_values'].append(value)
            if data:
                self.check(data, 1, header + header_size)
                counts = []
                cursor = data
                for _ in range(4):
                    n, cursor = self.leb(cursor); counts.append(n)
                for group, n in enumerate(counts):
                    if n > (len(self.data) - cursor) // (2 if group < 2 else 3):
                        raise FormatError('class_data_count', cursor)
                    index = 0
                    for j in range(n):
                        delta, cursor = self.leb(cursor)
                        if j and not delta:
                            raise FormatError('duplicate_defined_member', cursor)
                        index += delta
                        access, cursor = self.leb(cursor)
                        table = fields if group < 2 else methods
                        if index >= len(table) or table[index]['owner'] != cls['descriptor']:
                            raise FormatError('defined_member_owner_or_idx', cursor)
                        item = {**table[index], 'flags':access,'group':group}
                        if group < 2:
                            item['field_id_idx'] = index
                            cls['fields'].append(item)
                        else:
                            code_offset, cursor = self.leb(cursor)
                            item['method_id_idx'] = index
                            item['code'] = self.code(code_offset, header + header_size) if code_offset else None
                            if bool(code_offset) == bool(access & (0x100 | 0x400)):
                                raise FormatError('abstract_native_code_presence', cursor)
                            words = sum(2 if p in ('J','D') else 1 for p in item['parameters']) + (0 if access & 8 else 1)
                            item['parameter_words'] = words
                            if item['code'] is not None and item['code']['ins_size'] != words:
                                raise FormatError('ins_size_proto', code_offset)
                            cls['methods'].append(item)
            classes.append(cls)
        if len({c['descriptor'] for c in classes}) != len(classes):
            raise FormatError('duplicate_class', sections['classes']['offset'])
        return {'version':version,'logical_ordinal':ordinal,'header_offset':header,'file_size':size,
                'sections':sections,'map':entries,'strings':strings,'types':types,'protos':protos,
                'method_refs':methods,'field_refs':fields,'classes':classes}

    def project(self):
        views, cursor = [], 0
        while cursor < len(self.data):
            member = self.member(cursor, len(views))
            views.append(member)
            cursor += member['file_size']
            if member['version'] != '041':
                break
        if cursor != len(self.data):
            raise FormatError('header_chain_end', cursor)
        return {'producer':'independent-published-dex-format-reader','members':views}


def project(path):
    return Dex(Path(path).read_bytes()).project()
