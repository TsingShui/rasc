"""Case-authored original-input predicates over independent B03 projections.

Recovered-result obligations are explicitly deferred to B04/T/H execution. A
projection, source comment, builder success or expected boolean cannot certify an
input predicate. Unknown predicates fail closed.
"""
from __future__ import annotations
import json
import os
from pathlib import Path

from . import classfile
from .compare import exact
from .scalar import execute


def need(condition, message):
    if not condition:
        raise ValueError(message)


def method(doc, name, proto=None, owner='Lfixtures/Case;'):
    matches = [m for c in doc['classes'] if c['descriptor'] == owner for m in c['methods']
               if m['name'] == name and (proto is None or m['proto'] == proto)]
    need(len(matches) == 1, 'non-unique/missing original method: ' + owner + name + str(proto))
    return matches[0]


def ops(doc, name, proto=None, owner='Lfixtures/Case;'):
    body = method(doc, name, proto, owner)['code']
    need(body is not None, 'expected original code')
    return body['instructions']


def check_case(cid, views, files, root, cache, tools):
    """Return concrete witnesses, not a boolean copied from an expected field."""
    from e2e import PreparationError, run, save
    def doc(name=None):
        matches = [v for v in views if v.get('dex') is not None and (name is None or v['name'] == name)]
        need(bool(matches), 'missing independent decoder evidence')
        return matches[0]['dex']
    def raw(name=None):
        matches = [v for v in views if v.get('layout') is not None and (name is None or v['name'] == name)]
        need(bool(matches), 'missing layout evidence')
        return matches[0]['layout']['members'][0]
    d = doc() if any(v.get('dex') for v in views) else None
    witness = {}
    if cid == 'D015':
        for name, cmp_, branch in [('lt','cmpg-float','if-gez'),('le','cmpg-float','if-gtz'),('gt','cmpl-float','if-lez'),('ge','cmpl-float','if-ltz')]:
            code = ops(d,name)
            need(code[0]['opcode'] == cmp_ and code[1]['opcode'] == branch and code[0]['registers'][1:] == [0,1], 'lost FP comparison bias/order')
            witness[name] = code[:2]
    elif cid == 'D025':
        code = ops(d,'f')
        need([i['opcode'] for i in code] == ['check-cast','return-void'], 'unused reachable cast was removed or made observable')
        witness['code'] = code
    elif cid == 'D027':
        for name, component, count in [('a','[I',3),('b','[F',2)]:
            body = method(d,name)['code']; code = body['instructions']
            need([i['opcode'] for i in code[:4]] == ['const/4','new-array','fill-array-data','return-object'] and not body['tries'], 'FreshArray chain changed')
            payload = next(i for i in code if i['pc'] == code[2]['target'])
            need(code[0]['literal'] == count and code[1]['reference']['type'] == component and payload['element_width'] == 4 and len(payload['elements']) == count, 'fill dimensions changed')
            witness[name] = {'allocation':code[:2],'fill':code[2],'payload':payload,
                             'derivation':'One successful fresh allocation dominates the fill; sufficient constant length excludes bounds failure; no intervening escape/effect/handler. Constant in-place stores produce the identical primitive heap. Allocation failure occurs before either fill form.'}
    elif cid == 'D029':
        code = ops(d,'f'); body = method(d,'f')['code']
        call = next(i for i in code if i.get('reference',{}).get('name') == 'alwaysThrows')
        initial = next((i for i in code if i.get('literal') == 7 and i['pc'] < call['pc']),None)
        record = next(i for i in code if i.get('reference',{}).get('name') == 'record')
        need(initial is not None and initial['registers'] == record['registers'], 'compiler erased the pre-failure definition')
        target = body['tries'][0]['handlers'][0]['target']
        need(not any(i.get('literal') == 7 and i['pc'] >= target for i in code), 'catch rematerialized the old value')
        need(any(i['opcode'] == 'move-result' and i['pc'] > call['pc'] for i in code), 'missing normal result commit')
        witness = {'initial':initial,'throw_site':call,'try':body['tries'],'catch_read':record,'normal_commits':[i for i in code if i['opcode'].startswith('move')]}
    elif cid == 'D030':
        code = ops(d,'f'); body = method(d,'f')['code']
        calls = [i for i in code if i.get('reference',{}).get('name') in {'failA','failB'}]
        handlers = [[t['handlers'] for t in body['tries'] if t['start'] <= i['pc'] < t['start'] + t['count']] for i in calls]
        need(len(calls) == 2 and handlers[0] == handlers[1] and len(handlers[0]) == 1, 'throw sites no longer share the tested handler')
        constants = [i for i in code if i.get('literal') in (7,9)]
        record = next(i for i in code if i.get('reference',{}).get('name') == 'record')
        need(len(constants) == 2 and all(i['registers'] == record['registers'] for i in constants), 'old values no longer flow to handler')
        witness = {'sites':calls,'definitions':constants,'catch_read':record,'handlers':handlers}
    elif cid in {'D040','D082'}:
        names = ['inst','stat'] if cid == 'D040' else ['f']
        for name in names:
            m = method(d,name); code = m['code']['instructions']
            enters = [i for i in code if i['opcode'] == 'monitor-enter']; exits = [i for i in code if i['opcode'] == 'monitor-exit']
            need(len(enters) == 1 and len(exits) == 2 and all(i['registers'] == enters[0]['registers'] for i in exits) and m['code']['tries'], 'monitor/release identity or handler missing')
            if cid == 'D040': need(m['flags'] & 0x20000, 'DECLARED_SYNCHRONIZED missing')
            else: need(any(i['opcode'].startswith('goto') and i['target'] < i['pc'] for i in code), 'bounded loop backedge missing')
            witness[name] = {'flags':m['flags'],'enters':enters,'exits':exits,'tries':m['code']['tries']}
    elif cid == 'D041':
        m = method(d,'<init>'); code = m['code']['instructions']
        super_call = next(i for i in code if i.get('reference',{}).get('name') == '<init>')
        body_call = next(i for i in code if i.get('reference',{}).get('name') == 'log')
        need(super_call['opcode'] == 'invoke-direct' and super_call['pc'] < body_call['pc'] and m['proto'] == '()V', 'constructor order/proto changed')
        witness = {'this_word':m['code']['registers_size']-1,'super_call':super_call,'body_effect':body_call}
    elif cid == 'D046':
        c = raw()['classes'][0]
        need([(f['name'],f['type']) for f in c['fields']] == [('b','Z'),('i','I'),('s','Ljava/lang/String;')], 'static_fields binding changed')
        exact([{'tag':31,'value':False},{'tag':4,'value':0},{'tag':30,'value':None}], c['encoded_values'])
        witness = {'fields':c['fields'],'encoded_values':c['encoded_values'],'static_values_off':c['static_values_off']}
    elif cid == 'D048':
        bridge = method(d,'get','()Ljava/lang/Object;'); code = bridge['code']['instructions']
        need(bridge['flags'] & 0x1040 == 0x1040 and [i['opcode'] for i in code] == ['invoke-virtual','move-result-object','return-object'], 'noncanonical bridge')
        need(code[0]['reference']['proto'] == '()Ljava/lang/String;' and code[0]['reference']['owner'] == 'Lfixtures/Case;', 'wrong bridge receiver/target')
        witness = {'bridge':bridge}
    elif cid == 'D049':
        inner = next(c for c in d['classes'] if c['descriptor'] == 'Lfixtures/Outer$Inner;')
        need({a['type'] for a in inner['annotations']} >= {'Ldalvik/annotation/EnclosingClass;','Ldalvik/annotation/InnerClass;'}, 'inner metadata missing')
        captures = [f for f in inner['fields'] if f['flags'] & 0x1000 and f['type'] == 'Lfixtures/Outer;']
        code = ops(d,'<init>',owner=inner['descriptor'])
        need(len(captures) == 1 and code[0]['opcode'] == 'iput-object' and code[1]['opcode'] == 'invoke-direct', 'outer capture/pre-super order missing')
        witness = {'capture':captures,'constructor':code,'ownership':inner['annotations']}
    elif cid == 'D050':
        code = ops(d,'<clinit>')
        allocations = [i for i in code if i['opcode'] == 'new-instance']
        strings = [i['reference']['units'] for i in code if i['opcode'] == 'const-string']
        fields = [i['reference']['name'] for i in code if i['opcode'] == 'sput-object']
        need(len(allocations) == 2 and strings == [[90],[65]] and fields[:2] == ['Z','A'], 'enum constant initialization order lost')
        need(any(i.get('reference',{}).get('name') == 'clone' for i in ops(d,'values')), 'values does not clone')
        need(any(i.get('reference',{}).get('owner') == 'Ljava/lang/Enum;' and i['reference']['name'] == 'valueOf' for i in ops(d,'valueOf')), 'noncanonical valueOf')
        witness = {'initialization':code,'values':ops(d,'values'),'valueOf':ops(d,'valueOf'),'hidden_members':[m['name']+m['proto'] for c in d['classes'] for m in c['methods']]}
    elif cid == 'D055':
        valid = raw('original.dex'); need(valid['classes'][0]['methods'], 'missing original class')
        invalid = [v['record']['invalid'] for v in views if 'invalid' in v['record']]
        need({i['kind'] for i in invalid} == {'code_insns_out_of_range','typed_handler_count_out_of_range'}, 'one count variant missing')
        from .layout import Dex
        from e2e import sha
        import hashlib,struct,zlib
        faults=[]
        for view in views:
            if 'invalid' not in view['record']:continue
            record=view['record'];blob=(root/record['path']).read_bytes();fault=record['invalid'];offset=fault['offset']
            reader=Dex(blob)
            count,end=reader.leb(offset,signed=True) if fault['kind'].startswith('typed_') else (reader.u32(offset),offset+4)
            faults.append({'field':fault['kind'],'anchor':offset,'value':count,'minimum_required_bytes':abs(count)*2,
                           'remaining_bytes':len(blob)-end,'changed_byte_offsets':[i for i,(a,b) in enumerate(zip((root/next(v['record']['path'] for v in views if v['name']=='original.dex')).read_bytes(),blob)) if a!=b],
                           'original_sha256':next(v['record']['sha256'] for v in views if v['name']=='original.dex'),'mutated_sha256':record['sha256']})
            need(abs(count)*2>len(blob)-end,'count fault no longer proves a format overrun')
            repaired=bytearray(blob)
            if fault['kind'].startswith('typed_'):
                repaired[offset:end]=bytes([0x81,0x80,0x80,0x80,0]) # valid SLEB32 1 in the same five-byte extent
            else:struct.pack_into('<I',repaired,offset,6)
            repaired[12:32]=hashlib.sha1(repaired[32:]).digest();struct.pack_into('<I',repaired,8,zlib.adler32(repaired[12:])&0xffffffff)
            Dex(repaired).project()
        witness = {'original':valid['classes'][0],'invalid_fields':faults,'isolation':'Restoring only the offending count in its existing extent, plus header integrity fields, passes the independent layout reader; the rebuilt dependent ranges stay valid.'}
    elif cid in {'D056','D057','D058','D059'}:
        if cid == 'D056':
            before = raw('baseline.dex'); after = raw('mutated.dex')
            left = next(m for c in before['classes'] for m in c['methods'] if m['name']=='f')
            right = next(m for c in after['classes'] for m in c['methods'] if m['name']=='f')
            need(left['code']['units']==[0x14,7,0,0xfd28,0xf] and right['code']['units']==[0x14,7,0,0xfe28,0xf], 'branch mutation is not the named operand-word target')
            witness = {'original_units':left['code']['units'],'mutated_units':right['code']['units'],'anchor':right['code']['insns_offset']+6,'target_code_unit':1,'instruction_starts':[0,3,4]}
        elif cid in {'D057','D058'}:
            m = next(m for c in raw()['classes'] for m in c['methods'] if m['name']=='f')
            wanted = [0xf] if cid=='D057' else [0x16,0,0x1112,0x10]
            need(m['code']['units']==wanted and m['code']['ins_size']==0, 'undefined/overlapping-wide fault changed')
            witness = {'original_units':m['code']['units'],'entry_words':m['code']['ins_size'],'derivation':'Reachable return reads v0 with no entry or instruction definition.' if cid=='D057' else 'const-wide defines words 0/1; const/4 overwrites word 1; return-wide reads the invalidated pair.'}
        else:
            before = raw('original.dex'); after = raw('mutated.dex')
            original = next(m for c in before['classes'] for m in c['methods'] if m['name']=='f')
            changed = next(m for c in after['classes'] for m in c['methods'] if m['name']=='f')
            need(original['code']['units']==[14] and changed['code']['units']==[0x3e], 'unknown-width opcode mutation changed')
            witness = {'original_opcode':14,'mutated_opcode':0x3e,'anchor':changed['code']['insns_offset'],'width':'unknown in 035; no standard raw instruction is claimed'}
    elif cid == 'D060':
        ms = [m for c in d['classes'] for m in c['methods']]
        need({m['name']+m['proto'] for m in ms} == {'<init>()V','f()I','f()J'} and all(not m['flags'] & 0x1040 for m in ms), 'return-only overload or flags changed')
        witness = {'members':[{'name':m['name'],'proto':m['proto'],'flags':m['flags']} for m in ms]}
    elif cid == 'D061':
        m = method(d,'f'); code = m['code']['instructions']; catalog = json.loads((root/'tests/e2e/sources/D061/fixtures/catalog.json').read_text())
        gaps = [e for e in catalog['entries'] if e.get('declared') is False]
        invoke = [i for i in code if i.get('reference',{}).get('owner') == 'Lfixtures/External;']
        need(len(gaps) == 1 and len(invoke) == 1 and gaps[0]['descriptor'] == invoke[0]['reference']['owner'], 'used catalog gap missing')
        need(any(i['opcode'] == 'packed-switch-payload' for i in code) and any(i.get('reference',{}).get('units') == [55296,65] for i in code), 'payload/surrogate missing')
        need([h['type'] for h in m['code']['tries'][0]['handlers']] == ['Lfixtures/ErrA;','Lfixtures/ErrB;',None], 'handler order changed')
        witness = {'invoke':invoke,'missing_binding':gaps,'tries':m['code']['tries'],'payloads':[i for i in code if 'cases' in i]}
    elif cid == 'D063':
        code = ops(d,'f'); arithmetic = [i for i in code if i['opcode'].startswith('add-int')]
        need(len(arithmetic) >= 256 and not method(d,'f')['code']['tries'] and all(i['opcode'].startswith('add-int') or i['opcode'] == 'return' for i in code), 'compiler collapsed cancellation work')
        witness = {'arithmetic_sites':[i['pc'] for i in arithmetic],'code_units':method(d,'f')['code']['insns_size']}
    elif cid == 'D064':
        names = [m['name'] for c in d['classes'] if c['descriptor'] == 'Lfixtures/ReportCase;' for m in c['methods'] if m['name'] != '<init>']
        need(set(names) == {f'method_{n}' for n in range(8)} and len(names) == 8, 'report input declarations missing')
        witness = {'report_methods':names, 'minimum_declaration_bytes':sum(len('int '+n+'(){return 0;}') for n in names)}
        need(witness['minimum_declaration_bytes'] > 128, 'report does not exceed explicit limit')
        literal=next(i['reference']['units'] for i in ops(d,'f') if i['opcode'].startswith('const-string'))
        need(literal==[0,0,0,0],'member escaped-output input changed')
        witness['member_string_units']=literal
    elif cid == 'D065':
        members = views[0]['layout']['members']
        need(len(members) == 2 and all(m['version'] == '041' for m in members), 'not a native two-member 041')
        need(members[0]['sections']['strings'] == members[1]['sections']['strings'] and members[0]['sections']['methods'] == members[1]['sections']['methods'], 'shared tables missing')
        fs = [next(m for c in member['classes'] for m in c['methods'] if m['name'] == 'f') for member in members]
        need(fs[0]['method_id_idx'] == fs[1]['method_id_idx'] and [m['code']['units'] for m in fs] == [[0x13,11,0xf],[0x13,22,0xf]], 'numeric identity isolation/returns changed')
        witness = {'members':[{'logical_ordinal':m['logical_ordinal'],'header_offset':m['header_offset'],'sections':m['sections']} for m in members], 'methods':fs}
    elif cid == 'D066':
        ms = [(c['descriptor'],m['name'],m['proto']) for c in d['classes'] for m in c['methods']]
        need(ms == [('Lfixtures/Own;','<init>','()V'),('Lfixtures/Own;','f','()V')], 'definition list changed')
        need(any(m['owner'] == 'Lfixtures/External;' for m in d['method_refs']), 'external ref disappeared')
        witness = {'definitions':ms,'references':d['method_refs']}
    elif cid == 'D067':
        sites = [i for i in ops(d,'target') if i.get('reference',{}).get('name') == 'f' and i['reference']['owner'] == 'Lfixtures/Dep;']
        need(len(sites) == 2 and sites[0]['pc'] != sites[1]['pc'], 'xref sites collapsed')
        witness = {'sites':sites}
    elif cid in {'D068','D069','D070'}:
        entries = [e for v in views for e in v['zip']['entries']]
        if cid == 'D068':
            need([(e['ordinal'],e['name'],e['method']) for e in entries] == [(0,'stored.bin',0),(1,'deflated.bin',8),(2,'dup.bin',0),(3,'dup.bin',0)] and all(e['crc_valid'] for e in entries), 'physical ZIP identity/CRC changed')
        elif cid == 'D069':
            need(len(entries) == 4 and sum(not e['crc_valid'] for e in entries) == 1, 'CRC rejection isolation failed')
            encrypted = next(e for e in entries if e['flags'] & 1)
            need(encrypted['encryption_header'] == list(range(11)) + [encrypted['actual_crc'] >> 24] and encrypted['plaintext'] == [0,255,65,0,128,10] and encrypted['compressed_size'] == 18, 'ZipCrypto plaintext/header invalid')
            need(any(e['method'] == 12 and e['version_needed'] == 46 and e['plaintext'] == [0,255,65,0,128,10] for e in entries), 'not a real valid BZIP2 stream')
            need(any(e['method'] == 8 and e['plaintext'] == [65]*1024 and e['crc_valid'] for e in entries), 'inflate budget input invalid')
        else:
            capabilities = json.loads((cache/'prepared/D070/capabilities.json').read_text())
            need(capabilities['allRequiredCapabilitiesPresent'], 'missing required file platform capability')
            patterns = [e['pattern'] for e in entries if 'pattern' in e]
            exact([{'length':16777216,'multiplier':17,'add':3},{'length':16777216,'multiplier':29,'add':11}],patterns)
            need(any(e['name'] == '../escape' for e in entries), 'traversal entry missing')
            witness['capabilities'] = capabilities
        witness['entries'] = entries
    elif cid in {'D071','D072'}:
        methods = [m for c in d['classes'] if c['descriptor'] == 'Lfixtures/Case;' for m in c['methods'] if m['name'] != '<init>']
        need(all(not any(i['opcode'] in {'int-to-byte','int-to-char','int-to-short'} for i in m['code']['instructions']) for m in methods), 'unexpected narrowing in original input')
        if cid == 'D071': need({m['proto'] for m in methods} == {'(ZBB)B','(ZSS)S','(ZCC)C','(ZBB)I'}, 'narrow original proto lost')
        else: need({m['proto'] for m in methods} == {'(BB)I','(ZBC)I'} and any(i['opcode'].startswith('add-int') for i in ops(d,'sum')), 'int arithmetic/Phi input changed')
        witness = {'methods':methods}
    elif cid == 'D073':
        body = method(d,'f')['code']; code = body['instructions']
        need([i['opcode'] for i in code[:2]] == ['fill-array-data','return-void'] and not body['tries'], 'zero-fill effect shape changed')
        payload = next(i for i in code if i['pc'] == code[0]['target'])
        need(payload['element_width'] == 4 and payload['elements'] == [], 'not a real zero-count primitive fill')
        witness = {'fill':code[0],'payload':payload,'derivation':{'nonnull':'The single count-zero fill checks the array, writes no element and returns, as does the independent length-read mirror.','null':'Both operations throw NPE at the sole effect site before return.','scope':'Single known primitive component, no handler or additional effect; not an exception interpreter.'}}
    elif cid == 'D074':
        for role in ['parameter','too-short','unknown-length','escaped']:
            v = doc(role+'.dex'); code = ops(v,'f'); body = method(v,'f')['code']
            fill = next(i for i in code if i['opcode'] == 'fill-array-data'); payload = next(i for i in code if i['pc'] == fill['target'])
            need(payload['element_width'] == 4 and payload['elements'] == [7,8] and not body['tries'], 'array rejection payload changed')
            prefix = [i['opcode'] for i in code if i['pc'] < fill['pc']]
            expected = {'parameter':[], 'too-short':['const/4','new-array'], 'unknown-length':['new-array'], 'escaped':['const/4','new-array','invoke-static']}[role]
            need(prefix == expected, 'FreshArray falsification shape changed')
            if role == 'too-short': need(code[0]['literal'] == 1, 'sufficient-length boundary changed')
            if role == 'escaped': need(code[2]['reference']['name'] == 'publish', 'escape missing')
            witness[role] = {'prefix':code[:len(prefix)],'fill':fill,'payload':payload}
    elif cid == 'D075':
        for view in views:
            role = view['name'].removesuffix('.dex'); r = view['layout']['members'][0]
            m = next(m for c in r['classes'] for m in c['methods'] if m['name'] == 'f')
            formals = int(role[-3:]); is_static = role.startswith('static')
            need(m['parameter_words'] == formals + (0 if is_static else 1) and bool(m['code']) != role.startswith('abstract'), 'parameter words/body boundary changed')
            if role in {'static255','instance254','abstract254'}:
                cf = files[role + '/classes/fixtures/Case.class']
                need(any(x['name']=='f' and x['proto']==m['proto'] and ('code' in x)==bool(m['code']) for x in cf['methods']), 'reference compiler changed proto/body')
            witness[role] = {'proto':m['proto'],'parameter_words':m['parameter_words'],'code':m['code'] is not None}
    elif cid == 'D076':
        for view in views:
            role = view['name'].removesuffix('.dex'); v = view['dex']; code = ops(v,'f')
            units = next(i['reference']['units'] for i in code if i['opcode'].startswith('const-string'))
            literal = next(s for s in view['layout']['members'][0]['strings'] if s['units']==units)
            need(literal['mutf8_bytes'] == sum(2 if u==0 else 1 if u<128 else 2 if u<2048 else 3 for u in units), 'MUTF8 byte count differs')
            witness[role] = {'utf16_units':len(units),'mutf8_bytes':literal['mutf8_bytes']}
        need([witness[r]['mutf8_bytes'] for r in ['nul32767','nul32767-A','nul32768']] == [65534,65535,65536], 'MUTF8 boundary missing')
    elif cid == 'D077':
        code = ops(d,'f'); invokes = [i for i in code if i['opcode'] == 'invoke-static']
        need(len(invokes) == 10000 and len(code)==10001 and code[-1]['opcode']=='return-void' and all(i['reference']=={'kind':'method','owner':'Lfixtures/Dep;','name':'tick','proto':'()V'} for i in invokes), 'straight-line call input was rewritten')
        cf = files['reference/classes/fixtures/Case.class']; cm = next(m for m in cf['methods'] if m['name']=='f')
        need(len(cm['code']['sites']) == 10000 and all(i['opcode']==0xb8 and i['reference']['name']=='tick' for i in cm['code']['sites']), 'classfile call sequence missing')
        witness = {'invoke_pcs':[i['pc'] for i in invokes],'classfile_code_length':cm['code']['length']}
    elif cid == 'D078':
        c = d['classes'][0]; cf = files['reference/classes/fixtures/Case.class']
        need(c['flags'] & 0x600 == 0x600 and len(c['methods']) == 2200 and {m['name'] for m in c['methods']} == {f'm{n:04d}' for n in range(2200)} and all(m['code'] is None for m in c['methods']), 'complete interface input missing')
        need(len(cf['methods']) == 2200 and all('code' not in m and m['proto']=='()I' for m in cf['methods']), 'reference interface compiler artifact invalid')
        witness = {'declaration_keys':[(m['name'],m['proto']) for m in c['methods']], 'classfile_constant_pool_count':cf['constant_pool_count']}
    elif cid == 'D079':
        reader = next(c for c in d['classes'] if c['descriptor']=='Lfixtures/Reader;')
        reads = [i for m in reader['methods'] if m['code'] for i in m['code']['instructions'] if i['opcode'].startswith('sget')]
        need({i['reference']['name'] for i in reads} == {'N','S'}, 'original class-init field reads removed')
        cf = files['reference/classes/fixtures/Case.class']
        need(all('ConstantValue' not in f['attributes'] for f in cf['fields']), 'baseline constantness is not NonConstant')
        witness = {'original_reads':reads,'clinit':ops(d,'<clinit>'),'reference_fields':cf['fields']}
    elif cid == 'D080':
        code = ops(d,'f'); cf = files['constant-proof/fixtures/External.class']
        need([i['opcode'] for i in code] == ['sget','return'] and cf['fields'][0]['constant_value']['bits']==1, 'constant field reference/proof missing')
        witness = {'code':code,'classfile_fields':cf['fields']}
    elif cid == 'D081':
        code = ops(d,'f'); allocation = next(i for i in code if i['opcode']=='new-instance'); back = next(i for i in code if i['opcode'].startswith('goto'))
        enter = next(i for i in code if i['opcode']=='monitor-enter'); exit_ = next(i for i in code if i['opcode']=='monitor-exit')
        need(back['target']==allocation['pc'] and allocation['pc']<enter['pc']<back['pc']<exit_['pc'] and enter['registers']==exit_['registers'] and not method(d,'f')['code']['tries'], 'freshness counterexample changed')
        witness = {'allocation':allocation,'backedge':back,'enter':enter,'exit':exit_, 'derivation':'The first path enters object O1. The backedge executes new/init again, producing O2 while O1 remains held. The next exit operand is O2, so one static allocation/register identity cannot prove the dynamic lock identity. No ART-verifier acceptance or monitor interpreter is claimed.'}
    elif cid == 'D083':
        code = ops(d,'f'); payload = next(i for i in code if 'cases' in i)
        need([c['key'] for c in payload['cases']] == [-2147483648,2147483647], 'sparse signed extremes changed')
        witness = {'payload':payload,'span_i64':2147483647-(-2147483648)+1}
    elif cid == 'D084':
        m = method(d,'get','()Ljava/lang/Object;'); code = m['code']['instructions']
        need(m['flags'] & 0x1040 == 0x1040 and [i['reference']['name'] for i in code if i.get('reference',{}).get('kind')=='method']==['log','get'], 'bridge extra effect/order missing')
        witness = {'bridge':m,'members':[(m['name'],m['proto'],m['flags']) for c in d['classes'] for m in c['methods']]}
    elif cid == 'D085':
        need(raw()['version']=='038' and len(d['call_sites'])==1 and len(d['method_handles'])==1, 'dynamic metadata absent')
        code = ops(d,'aDynamic'); names = [m['name'] for c in raw()['classes'] for m in c['methods']]
        need(names==['<init>','aDynamic','zAdd'] and [i['opcode'] for i in code]==['const-string','invoke-custom','move-result','return'], 'dynamic failed-prefix input changed')
        need(code[0]['reference']['units']==[0,55296,42,47,92,117,48,48,48,97], 'dynamic surrogate/comment units lost')
        need(d['call_sites'][0]['proto']['parameters']==[] and d['call_sites'][0]['proto']['return']=='I' and d['method_handles'][0]['handle_type']==4, 'wrong bootstrap/proto kind')
        witness = {'methods':names,'call_sites':d['call_sites'],'method_handles':d['method_handles'],'dynamic':code}
    elif cid == 'D086':
        r = raw(); m = next(m for c in r['classes'] for m in c['methods'] if m['name']=='f'); units=m['code']['units']
        need(len(units)==131073 and units[0]==14 and all(u==0 for u in units[1:]) and not m['code']['tries'], 'large original code range truncated or changed')
        pure = execute(method(d,'f'),0,[])
        need(pure['kind']=='void' and pure['steps']==1, 'dead range became reachable')
        witness = {'insns_offset':m['code']['insns_offset'],'insns_size':len(units),'dead_nops':len(units)-1,'scalar':pure}
    return witness


def certify_requirements(root, cache, tools, spec, certificates):
    from e2e import PreparationError, load, relative, save, sha
    supported = {'D015','D025','D027','D029','D030','D040','D041','D046','D048','D049','D050','D055','D060','D061','D063','D064','D065','D066','D067','D068','D069','D070','D071','D072','D073','D074','D075','D076','D077','D078','D079','D080','D081','D082','D083','D084','D085','D086'}
    result_only = {'D077':{1},'D078':{1},'D079':{1},'D081':{1},'D085':{1,2,3},'D086':{1,2}}
    both = {'D041':{0},'D048':{0},'D064':{0},'D065':{0},'D066':{0},'D067':{0},'D068':{0},'D069':{0},'D070':{0},'D074':{0},'D075':{1},'D076':{1},'D083':{0}}
    results = []
    for case, certificate in zip([c for c in spec['cases'] if c['required']], certificates):
        cid = case['id']; need(certificate['id']==cid,'case identity mismatch')
        work = cache / 'prepared' / cid
        try:
            views = []
            for a in certificate['assets']:
                views.append({'name':Path(a['path']).name,'record':a,
                              'layout':load(root/a['layout']) if 'layout' in a else None,
                              'dex':load(root/a['dexlib2']) if 'dexlib2' in a else None,
                              'zip':load(root/a['zip']) if 'zip' in a else None})
            files = {}
            for path in sorted(work.rglob('*.class')):
                if 'driver-classes' not in path.parts:
                    files[path.relative_to(work).as_posix()] = classfile.project(path)
            save(work/'classfiles.json', files)
            if case['inputRequirements']:
                need(cid in supported,'unknown input requirement predicate')
            witness = check_case(cid,views,files,root,cache,tools)
            save(work/'requirement-witness.json', witness)
            checks = []
            for index, _ in enumerate(case['inputRequirements']):
                if index not in result_only.get(cid,set()):
                    checks.append({'requirement_index':index,'stage':'input_certification',
                                   'evidence_source':'independent-projection-and-source-reference','projection_path':relative(work/'requirement-witness.json')})
                if index in result_only.get(cid,set()) or index in both.get(cid,set()):
                    checks.append({'requirement_index':index,'stage':'result_validation','evidence_source':'public-result-and-independent-output-artifacts',
                                   'projection_path':'/artifacts'})
            certificate.update(state='inputs-certified',checks=checks,classfiles=relative(work/'classfiles.json'),witness=relative(work/'requirement-witness.json'))
            save(work/'certificate.json',certificate)
            results.append(certificate)
        except (ValueError,KeyError,StopIteration,OSError) as exc:
            results.append({'id':cid,'state':'blocked','reason':str(exc)})
            print(cid,'requirement blocked:',str(exc),flush=True)
    save(cache/'certification-report.json', {'spec_sha256':sha(root/'docs/diana/e2e_cases.json'),'cases':results,
         'case_denominator':len(results),'invocation_denominator':sum(len(c['invocations']) for c in spec['cases'] if c['required']),
         'platform':tools.platform,'art':'not-run','engine':'not_implemented'})
    if any(r['state']!='inputs-certified' for r in results):
        raise PreparationError('input predicate failures; see certification-report.json')
    # Manifest construction also checks every Driver-bound expected observation and
    # symbol binding. No source-only success is accepted as engine execution.
    from .manifest import build_manifest
    build_manifest(root,cache,tools,spec,results)
