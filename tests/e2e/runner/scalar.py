"""Tests-only pure-register DEX oracle; not a VM and never an effect oracle.

Supported: constants, word-correct moves, I32/I64 arithmetic and bitwise/shift,
integer conversions, cmp-long, conditional/unconditional branches, integer switch,
nop and return. Nonzero div/rem only. No FP, heap, calls, try, throw or monitors.
Input is the independent dexlib2 projection, not Diana IR. Every run is bounded.
"""
from __future__ import annotations


class Unsupported(ValueError):
    pass


def signed(value, bits):
    value &= (1 << bits) - 1
    return value - (1 << bits) if value & (1 << (bits - 1)) else value


def divide(a, b):
    if b == 0:
        raise Unsupported('zero divisor: no exception oracle')
    magnitude = abs(a) // abs(b)
    return -magnitude if (a < 0) != (b < 0) else magnitude


def execute(method, ins_size, arguments, maximum_steps=1_000_000):
    body = method['code']
    if body is None or body['tries']:
        raise Unsupported('no code or protected method')
    registers = [None] * body['registers_size']
    high = object()
    pairs = {}
    def write(index, value, wide=False):
        if index < 0 or index + (1 if wide else 0) >= len(registers):
            raise Unsupported('register range')
        for low in (index - 1, index, index + 1) if wide else (index - 1, index):
            pairs.pop(low, None)
        registers[index] = value
        if wide:
            token = object()
            pairs[index] = token
            registers[index + 1] = (high, index, token)
    def read(index, wide=False):
        if index < 0 or index + (1 if wide else 0) >= len(registers) or type(registers[index]) is not int:
            raise Unsupported('undefined/partial word read')
        value = registers[index]
        if wide and registers[index + 1] != (high, index, pairs.get(index)):
            raise Unsupported('wide pair was overwritten')
        return value
    parameter_types = [p['type'] for p in method['parameters']]
    if not method['flags'] & 8 or len(parameter_types) != len(arguments):
        raise Unsupported('only static pure entrypoints with exact arguments')
    cursor = len(registers) - ins_size
    for typ, value in zip(parameter_types, arguments):
        if typ not in {'Z','B','S','C','I','J'} or type(value) not in {int,bool}:
            raise Unsupported('non-integer entry type')
        wide = typ == 'J'
        write(cursor, signed(int(value), 64 if wide else 32), wide)
        cursor += 2 if wide else 1
    if cursor != len(registers):
        raise Unsupported('entry word count differs from proto')
    instructions = {i['pc']:i for i in body['instructions']}
    if len(instructions) != len(body['instructions']):
        raise Unsupported('duplicate original PC')
    pc = 0
    for step in range(maximum_steps):
        if pc not in instructions:
            raise Unsupported('not an instruction boundary')
        insn = instructions[pc]
        opcode = insn['opcode']
        regs = insn['registers']
        next_pc = pc + insn['width']
        if opcode == 'nop':
            pass
        elif opcode == 'return-void':
            return {'kind':'void','steps':step + 1}
        elif opcode in {'return','return-wide'}:
            value = read(regs[0], opcode == 'return-wide')
            if method['proto'].endswith('Z'):
                value = bool(value)
            return {'kind':'long' if opcode == 'return-wide' else 'boolean' if method['proto'].endswith('Z') else 'int',
                    'value':value,'steps':step + 1}
        elif opcode.startswith('const') and 'string' not in opcode and 'class' not in opcode:
            wide = opcode.startswith('const-wide')
            write(regs[0], signed(insn['literal'], 64 if wide else 32), wide)
        elif opcode.startswith('move') and not opcode.startswith(('move-result','move-exception','move-object')):
            wide = opcode.startswith('move-wide')
            write(regs[0], read(regs[1], wide), wide)
        elif opcode.startswith('goto'):
            next_pc = insn['target']
        elif opcode.startswith('if-'):
            test = opcode[3:]
            zero = test.endswith('z')
            if zero:
                test = test[:-1]
            a, b = read(regs[0]), 0 if zero else read(regs[1])
            predicate = {'eq':a==b,'ne':a!=b,'lt':a<b,'le':a<=b,'gt':a>b,'ge':a>=b}.get(test)
            if predicate is None:
                raise Unsupported('unknown branch comparison')
            if predicate:
                next_pc = insn['target']
        elif opcode in {'packed-switch','sparse-switch'}:
            payload = instructions.get(insn['target'])
            if payload is None or 'cases' not in payload:
                raise Unsupported('missing switch payload')
            value = read(regs[0])
            for item in payload['cases']:
                if value == item['key']:
                    next_pc = pc + item['offset']
                    break
        elif opcode == 'cmp-long':
            a, b = read(regs[1], True), read(regs[2], True)
            write(regs[0], (a>b) - (a<b))
        elif opcode in {'int-to-byte','int-to-short','int-to-char','int-to-long','long-to-int'}:
            value = read(regs[1], opcode.startswith('long-'))
            if opcode == 'int-to-byte':
                value = signed(value,8)
            elif opcode == 'int-to-short':
                value = signed(value,16)
            elif opcode == 'int-to-char':
                value &= 0xffff
            write(regs[0], signed(value,64 if opcode.endswith('long') else 32), opcode.endswith('long'))
        elif opcode in {'neg-int','not-int','neg-long','not-long'}:
            wide = opcode.endswith('long')
            value = read(regs[1], wide)
            write(regs[0], signed(-value if opcode.startswith('neg') else ~value,64 if wide else 32),wide)
        elif '-' in opcode:
            operation, family = opcode.split('-',1)
            family = family.split('/')[0]
            if operation not in {'add','sub','rsub','mul','div','rem','and','or','xor','shl','shr','ushr'} or family not in {'int','long'}:
                raise Unsupported('effect/FP/unsupported opcode: ' + opcode)
            wide = family == 'long'
            bits = 64 if wide else 32
            if '/2addr' in opcode:
                a, b = read(regs[0],wide), read(regs[1],wide and operation not in {'shl','shr','ushr'})
            elif 'literal' in insn:
                a, b = read(regs[1],wide), insn['literal']
            else:
                a, b = read(regs[1],wide), read(regs[2],wide and operation not in {'shl','shr','ushr'})
            if operation == 'add': value = a+b
            elif operation == 'sub': value = a-b
            elif operation == 'rsub': value = b-a
            elif operation == 'mul': value = a*b
            elif operation == 'div': value = divide(a,b)
            elif operation == 'rem': value = a-divide(a,b)*b
            elif operation == 'and': value = a&b
            elif operation == 'or': value = a|b
            elif operation == 'xor': value = a^b
            elif operation == 'shl': value = a << (b & (bits-1))
            elif operation == 'shr': value = a >> (b & (bits-1))
            else: value = (a & ((1 << bits)-1)) >> (b & (bits-1))
            write(regs[0],signed(value,bits),wide)
        else:
            raise Unsupported('unsupported opcode: ' + opcode)
        pc = next_pc
    raise Unsupported('execution step allowance exhausted')
