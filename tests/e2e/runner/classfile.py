"""Small independent JVM classfile declaration/Code projection for B03/B04."""
from __future__ import annotations
from pathlib import Path
import struct


def mutf8(data):
    units, cursor = [], 0
    while cursor < len(data):
        first = data[cursor]; cursor += 1
        if 0 < first < 128:
            value = first
        elif first & 0xe0 == 0xc0:
            if cursor >= len(data) or data[cursor] & 0xc0 != 0x80:
                raise ValueError('bad classfile MUTF8')
            value = (first & 31) << 6 | data[cursor] & 63; cursor += 1
        elif first & 0xf0 == 0xe0:
            if cursor + 2 > len(data) or any(b & 0xc0 != 0x80 for b in data[cursor:cursor + 2]):
                raise ValueError('bad classfile MUTF8')
            value = (first & 15) << 12 | (data[cursor] & 63) << 6 | data[cursor + 1] & 63; cursor += 2
        else:
            raise ValueError('bad classfile MUTF8 lead')
        units.append(value)
    return b''.join(struct.pack('<H', u) for u in units).decode('utf-16-le', 'surrogatepass')


class Reader:
    def __init__(self, data):
        self.data, self.cursor = data, 0

    def take(self, size):
        if size < 0 or size > len(self.data) - self.cursor:
            raise ValueError('truncated classfile')
        value = self.data[self.cursor:self.cursor + size]; self.cursor += size
        return value

    def u1(self): return self.take(1)[0]
    def u2(self): return int.from_bytes(self.take(2), 'big')
    def u4(self): return int.from_bytes(self.take(4), 'big')


def project(path):
    reader = Reader(Path(path).read_bytes())
    if reader.u4() != 0xcafebabe:
        raise ValueError('classfile magic')
    minor, major, count = reader.u2(), reader.u2(), reader.u2()
    pool, index = [None], 1
    while index < count:
        tag = reader.u1()
        if tag == 1:
            size = reader.u2()
            value = {'tag':tag,'text':mutf8(reader.take(size)),'mutf8_bytes':size}
        elif tag in (3,4): value = {'tag':tag,'bits':reader.u4()}
        elif tag in (5,6): value = {'tag':tag,'bits':int.from_bytes(reader.take(8),'big')}
        elif tag in (7,8,16,19,20): value = {'tag':tag,'index':reader.u2()}
        elif tag in (9,10,11,12,17,18): value = {'tag':tag,'a':reader.u2(),'b':reader.u2()}
        elif tag == 15: value = {'tag':tag,'kind':reader.u1(),'index':reader.u2()}
        else: raise ValueError('unknown classfile constant tag')
        pool.append(value); index += 1
        if tag in (5,6): pool.append(None); index += 1
    def text(index): return pool[index]['text']
    def class_name(index): return text(pool[index]['index'])
    def reference(index):
        value = pool[index]
        if value['tag'] in (9,10,11):
            name_type = pool[value['b']]
            return {'owner':'L'+class_name(value['a'])+';','name':text(name_type['a']),'proto':text(name_type['b'])}
        return value
    def attributes(source=reader):
        result = {}
        for _ in range(source.u2()):
            name = text(source.u2()); data = source.take(source.u4())
            if name in result: raise ValueError('duplicate classfile attribute')
            result[name] = data
        return result
    flags, this, parent = reader.u2(), reader.u2(), reader.u2()
    interfaces = [class_name(reader.u2()) for _ in range(reader.u2())]
    fields, methods = [], []
    for target in (fields, methods):
        for _ in range(reader.u2()):
            access, name, descriptor = reader.u2(), text(reader.u2()), text(reader.u2())
            attrs = attributes()
            member = {'flags':access,'name':name,'proto':descriptor,'attributes':list(attrs), 'throws':[]}
            if 'Exceptions' in attrs:
                exceptions = Reader(attrs['Exceptions'])
                member['throws'] = ['L'+class_name(exceptions.u2())+';' for _ in range(exceptions.u2())]
                if exceptions.cursor != len(exceptions.data): raise ValueError('invalid Exceptions attribute')
            if 'ConstantValue' in attrs:
                member['constant_value'] = pool[int.from_bytes(attrs['ConstantValue'],'big')]
            if 'Code' in attrs:
                body = Reader(attrs['Code'])
                stack, locals_, size = body.u2(), body.u2(), body.u4()
                if not 0 < size < 65536:
                    raise ValueError('illegal JVM code_length')
                code = body.take(size)
                sites, starts, pc = [], set(), 0
                while pc < len(code):
                    starts.add(pc)
                    op = code[pc]
                    width = 1
                    if op in (0xaa,0xab):
                        base = (pc + 4) & ~3
                        if base + 8 > len(code): raise ValueError('truncated JVM switch')
                        if op == 0xaa:
                            if base + 12 > len(code): raise ValueError('truncated JVM tableswitch')
                            low, high = struct.unpack_from('>ii',code,base + 4)
                            if high < low: raise ValueError('invalid JVM switch range')
                            width = base - pc + 12 + 4 * (high - low + 1)
                        else:
                            pairs = struct.unpack_from('>i',code,base + 4)[0]
                            if pairs < 0: raise ValueError('invalid JVM switch count')
                            width = base - pc + 8 + 8 * pairs
                    elif op == 0xc4:
                        if pc + 1 >= len(code): raise ValueError('truncated JVM wide')
                        width = 6 if code[pc + 1] == 0x84 else 4
                    elif op in (0x10,0x12,0xbc) or 0x15 <= op <= 0x19 or 0x36 <= op <= 0x3a or op == 0xa9:
                        width = 2
                    elif op in (0x11,0x13,0x14,0x84) or 0x99 <= op <= 0xa8 or 0xb2 <= op <= 0xb8 or op in (0xbb,0xbd,0xc0,0xc1,0xc6,0xc7):
                        width = 3
                    elif op == 0xc5: width = 4
                    elif op in (0xb9,0xba,0xc8,0xc9): width = 5
                    if width > len(code) - pc:
                        raise ValueError('truncated JVM instruction')
                    if 0xb2 <= op <= 0xb9:
                        sites.append({'pc':pc,'opcode':op,'reference':reference(int.from_bytes(code[pc + 1:pc + 3],'big'))})
                    pc += width
                handlers = []
                for _ in range(body.u2()):
                    start, end, handler, catch = body.u2(), body.u2(), body.u2(), body.u2()
                    if not 0 <= start < end <= size or start not in starts or end != size and end not in starts or handler not in starts:
                        raise ValueError('illegal JVM exception handler range')
                    if catch and (catch >= count or not pool[catch] or pool[catch]['tag'] != 7):
                        raise ValueError('illegal JVM catch type reference')
                    handlers.append({'start':start,'end':end,'handler':handler,'type':None if catch == 0 else 'L'+class_name(catch)+';'})
                code_attributes = attributes(body)
                if body.cursor != len(body.data): raise ValueError('extra Code attribute data')
                member['code'] = {'length':size,'max_stack':stack,'max_locals':locals_,'handlers':handlers,'attributes':list(code_attributes),'sites':sites}
            target.append(member)
    attrs = attributes()
    if reader.cursor != len(reader.data): raise ValueError('extra classfile data')
    enclosing_scope = 'EnclosingMethod' in attrs or 'NestHost' in attrs
    if 'InnerClasses' in attrs:
        inner = Reader(attrs['InnerClasses'])
        for _ in range(inner.u2()):
            own, outer, name, access = inner.u2(), inner.u2(), inner.u2(), inner.u2()
            enclosing_scope |= own == this
        if inner.cursor != len(inner.data): raise ValueError('invalid InnerClasses attribute')
    return {'producer':'independent-JVMS-classfile-reader','descriptor':'L'+class_name(this)+';','flags':flags,
            'parent':None if not parent else 'L'+class_name(parent)+';','interfaces':interfaces,'major':major,'minor':minor,
            'constant_pool_count':count,'utf8_lengths':[p['mutf8_bytes'] for p in pool if p and p['tag']==1],
            'fields':fields,'methods':methods,'attributes':list(attrs),'enclosing_scope':enclosing_scope}
