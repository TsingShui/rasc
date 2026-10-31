"""Strict semantic comparisons shared by baseline certification and the runner.

No normalization of program order, UTF-16, FP zero, absent fields or failure
kinds is permitted. Extra unasserted object metadata is not an output golden.
"""
from __future__ import annotations


class Mismatch(ValueError):
    pass


def exact(expected, actual, path='$'):
    if type(expected) is not type(actual):
        raise Mismatch(f'{path}: type {type(actual).__name__}, expected {type(expected).__name__}')
    if isinstance(expected, dict):
        for key, value in expected.items():
            if key not in actual:
                raise Mismatch(f'{path}.{key}: absent')
            exact(value, actual[key], path + '.' + key)
    elif isinstance(expected, list):
        if len(expected) != len(actual):
            raise Mismatch(f'{path}: length {len(actual)}, expected {len(expected)}')
        for index, (left, right) in enumerate(zip(expected, actual)):
            exact(left, right, f'{path}[{index}]')
    elif expected != actual:
        raise Mismatch(f'{path}: {actual!r}, expected {expected!r}')


def driver_values(observation):
    """Project real Driver output without consulting the expected result."""
    out = {k:v for k,v in observation.items() if k not in {'case','invocation','role','result','observations','events','codeUnits','codeUnitsHex','utf16Units','returned','completed'}}
    events = observation.get('events')
    if events is not None:
        if any(type(e.get('seq')) is not int or e['seq'] != n or e.get('kind') != 'call' or not isinstance(e.get('label'), str)
               for n,e in enumerate(events)):
            raise Mismatch('invalid or unordered Driver event sequence')
        out['events'] = [e['label'] for e in events]
    result = observation.get('result')
    if isinstance(result, dict):
        kind = result.get('kind')
        if kind in {'int','boolean','byte','short','char'}:
            out['return'] = result['value']
        elif kind == 'long':
            out['returnLong'] = str(result['value'])
        elif kind == 'float':
            out['returnFloatBits'] = result['bits']
        elif kind == 'string':
            out['returnStringUnits'] = result['units']
        elif kind == 'void':
            out['returnVoid'] = True
        elif kind not in {'null','int[]','float[]'}:
            raise Mismatch('unknown Driver result kind: ' + str(kind))
    for item in observation.get('observations', []):
        method = item['method'].split('(')[0]
        if method in out:
            raise Mismatch('duplicate method observation: ' + method)
        out[method] = item['result']['value']
    if 'codeUnits' in observation:
        units = observation['codeUnits']
        if not isinstance(units, list) or any(type(u) is not int or not 0 <= u <= 65535 for u in units):
            raise Mismatch('not a UTF-16 vector')
        if observation['utf16Units'] != len(units) or observation['codeUnitsHex'] != ''.join(f'{u:04x}' for u in units):
            raise Mismatch('inconsistent complete UTF-16 observation')
        out['returnStringUnits'] = units
    if observation.get('exception') is not None:
        if observation.get('completed') is not False:
            raise Mismatch('exception with a completed Driver execution')
    elif observation.get('completed') is not True:
        raise Mismatch('incomplete Driver execution')
    return out


def driver_assertions(spec, expected, observation):
    actual = driver_values(observation)
    fields = {k:v for k,v in expected.items() if spec['assertionSources'][k]['source'] == 'driver'}
    for key, value in fields.items():
        if key == 'stringPattern':
            units = [0] * value['zeroPrefixUnits']
            if 'lastUnit' in value:
                units.append(value['lastUnit'])
            exact(units, actual.get('returnStringUnits'), '$.stringPattern')
        else:
            if key not in actual:
                raise Mismatch('missing Driver observation: ' + key)
            exact(value, actual[key], '$.' + key)
    return list(fields)
