"""B04 result-set and evidence comparator. Engine results remain unavailable until T05.

The public-library adapter supplies domain observations, not expected booleans.
This module links results by original identity and computes assertions from the
observations. Reference-falsification records are never engine E2E evidence.
"""
from __future__ import annotations
from collections import Counter
import json
from pathlib import Path

from .adapter import required_runs
from .compare import Mismatch, driver_assertions, exact


def original_debug_facts(original, layout):
    # Attach original code words/counts from the independent physical reader.
    classes=[]
    for c in original['classes']:
        raw_class=next(x for x in layout['classes'] if x['descriptor']==c['descriptor'])
        methods=[]
        for m in c['methods']:
            raw=next(x for x in raw_class['methods'] if x['name']==m['name'] and x['proto']==m['proto'])
            code=m['code']
            if code is not None:
                code={**code,'raw_words':raw['code']['units'],'ins_size':raw['code']['ins_size'],'outs_size':raw['code']['outs_size']}
            methods.append({**m,'code':code})
        classes.append({**c,'methods':methods})
    return {**original,'classes':classes}


def debug_view(original, selected):
    wanted={(s['descriptor'],s['name'],s['proto']) for s in selected}
    classes=[]
    for c in original['classes']:
        methods=[m for m in c['methods'] if (c['descriptor'],m['name'],m['proto']) in wanted]
        if methods:classes.append({**c,'methods':methods})
    if sum(len(c['methods']) for c in classes)!=len(wanted):raise Mismatch('missing selected debug method')
    return {'schema':'diana.dex-debug/1','complete':True,'classes':classes,'undecoded_ranges':[]}


def normalized_debug(doc):
    # Only original analysis facts are mandatory, not assembly shell/debug syntax.
    classes={}
    for c in doc['classes']:
        if c['descriptor'] in classes:raise Mismatch('duplicate original debug class')
        value={k:c[k] for k in ['class_def_idx','flags','super','interfaces']}
        members={}
        for m in c['methods']:
            key=m['name']+m['proto']
            if key in members:raise Mismatch('duplicate original debug method')
            member={k:m[k] for k in ['method_id_idx','name','proto','flags']}
            member['code']=None if m['code'] is None else {k:m['code'][k] for k in ['registers_size','ins_size','outs_size','insns_size','raw_words','instructions','tries']}
            members[key]=member
        value['methods']=members
        fields={}
        for f in c['fields']:
            key=f['name']+':'+f['type']
            if key in fields:raise Mismatch('duplicate original debug field')
            fields[key]={k:f[k] for k in ['name','type','flags','initial']}
        value['fields']=fields;classes[c['descriptor']]=value
    return classes


def debug_assertions(original, actual, selected):
    if actual.get('schema')!='diana.dex-debug/1' or actual.get('complete') is not True or actual.get('undecoded_ranges')!=[]:
        raise Mismatch('missing/partial debug snapshot cannot claim complete original facts')
    expected=debug_view(original,selected)
    left,right=normalized_debug(expected),normalized_debug(actual)
    if left!=right:raise Mismatch('original debug instruction/word/payload/handler/reference mismatch')
    exact(left,right,'$.original_debug_facts')
    methods=[m for c in actual['classes'] for m in c['methods'] if m['name']!='<init>']
    bridge=[m for m in methods if m['flags'] & 0x1040 == 0x1040]
    log_units=[i['reference']['units'] for m in bridge if m['code'] for i in m['code']['instructions'] if i.get('reference',{}).get('kind')=='string']
    return {k:True for k in ['debugFactsExact','debugDeclarationExact','debugDynamicExact','allInstructionSites','payloadBitsExact','handlerPriorityExact','stringUnitsExact','memberRefsExact','effectfulBridgeDebugPreserved']} | {
        'debugMethodsPreserved':len(methods),'debugBridgeLogArgumentUnits':log_units[0] if log_units else None}


def public_assertions(value):
    outcome=value['outcome']
    members=value.get('members',[])
    kinds={d.get('kind') for d in value.get('diagnostics',[])}
    if outcome=='java' and kinds & {'malformed','unsupported','budget_exceeded','cancelled','internal_invariant'}:
        raise Mismatch('failed Java cannot be counted as a success')
    if outcome in {'debug','mixed'} and kinds & {'malformed','cancelled','internal_invariant'}:
        raise Mismatch('hard failure cannot become trusted raw success')
    allowed={'java':{'java','java_unit'},'debug':{'debug'},'mixed':{'mixed'},'unavailable':{'unavailable'},
             'declaration-only':{'declaration_only'},'error':{'request_error'},'resource-failure':{'request_error','unavailable'},'data':{'data'}}
    if outcome not in allowed or value.get('status') not in allowed[outcome]:raise Mismatch('inconsistent public classification')
    if any(d.get('kind')=='internal_invariant' or d.get('bug') for d in value.get('diagnostics',[])):
        raise Mismatch('BUG/InternalInvariant is a hard failure')
    return {'outcome':outcome,'java':outcome=='java','decodedDebug':outcome=='debug',
            'noJava':not any(m['language']=='java' for m in members) and outcome!='java',
            'noExecutableSource':not any(m['language']=='java' for m in members) and outcome!='java',
            'noFakeJavaUnit':outcome!='java' and value.get('status')!='java_unit',
            'noFallbackReport':'report' in value and value['report'] is None,
            'resourceFailureExplicit':outcome=='resource-failure' and any(d.get('kind')=='budget_exceeded' for d in value.get('diagnostics',[])),
            'noTruncatedSuccess':outcome=='resource-failure' and value.get('published_success') is False}


def diagnostic_assertions(value):
    diagnostics=value.get('diagnostics',[])
    if not diagnostics: return {}
    if any(d.get('kind')=='internal_invariant' or d.get('bug') for d in diagnostics): raise Mismatch('BUG diagnostic')
    first=diagnostics[0]
    out={'reason':first['kind'],'errorKind':first['kind']}
    if 'code' in first:out['unsupportedReason']=first['code']
    if 'limit' in first:out['limit']=first['limit']
    if 'target_item' in first:out['targetItem']=first['target_item']
    return out


def declaration_assertions(original, compiled):
    originals=[(c,m) for c in original['classes'] for m in c['methods']]
    actual={(c['descriptor'],m['name'],m['proto']):(c,m) for c in compiled for m in c['methods']}
    original_fields=[(c['descriptor'],f) for c in original['classes'] for f in c['fields']]
    compiled_fields={(c['descriptor'],f['name'],f['proto']):f for c in compiled for f in c['fields']}
    preserved=all((c['descriptor'],m['name'],m['proto']) in actual for c,m in originals)
    bridges=[(c,m) for c,m in originals if m['flags'] & 0x1040 == 0x1040]
    bridge_ok=bool(bridges) and all(actual.get((c['descriptor'],m['name'],m['proto']),({},{}))[1].get('flags',0)&0x1040==0x1040 for c,m in bridges)
    finals=[(owner,f) for owner,f in original_fields if f['flags'] & 16]
    final_ok=bool(finals) and all(compiled_fields.get((owner,f['name'],f['type']),{}).get('flags',0)&16 for owner,f in finals)
    fields=list(compiled_fields.values())
    gets=[s for c in compiled for m in c['methods'] if 'code' in m for s in m['code']['sites'] if s['opcode']==0xb2]
    invoke_sites=[s for c in compiled for m in c['methods'] if m['name']=='f' and 'code' in m for s in m['code']['sites'] if s['opcode']==0xb8]
    abstract=[m for c,m in originals if m['code'] is None]
    return {'protoPreserved':preserved,'generatedBridgePreserved':bridge_ok,'finalFieldPreserved':bool(final_ok),
            'declarationsPreserved':sum(1 for c,m in originals if m['name']!='<init>' and (c['descriptor'],m['name'],m['proto']) in actual),
            'hasBody':not bool(abstract),'constantValueAbsent':bool(fields) and all('ConstantValue' not in f['attributes'] for f in fields),
            'fieldReadsPreserved':{s['reference']['name'] for s in gets}>={'N','S'},'orderedInvokeSites':len(invoke_sites)}


def rendered_assertions(value, text, original):
    text.decode('utf-8',errors='strict')
    spans=value['spans']; members={(c['descriptor'],m['name']+m['proto']) for c in original['classes'] for m in c['methods']}
    identities=[]; java=[]; raw=[]
    for s in spans:
        start,end=s['start'],s['end']
        if type(start) is not int or type(end) is not int or not 0<=start<end<=len(text):raise Mismatch('source span range')
        text[start:end].decode('utf-8',errors='strict')
        identity=(s['owner'],s['method']);identities.append(identity)
        if identity not in members:raise Mismatch('span has foreign original identity')
        (java if s['language']=='java' else raw if s['language']=='dex_debug' else []).append(s['method'])
        if s['language']=='dex_debug':
            try:snapshot=json.loads(text[start:end])
            except (ValueError,UnicodeError) as exc:raise Mismatch('debug span is not the actual readable snapshot') from exc
            if snapshot.get('schema')!='diana.dex-debug/1':raise Mismatch('debug span has no original-information schema')
    if len(set(identities))!=len(identities):raise Mismatch('duplicate original source span')
    excerpts=value.get('raw_excerpt_ranges',[])
    for span in excerpts:
        start,end=span['start'],span['end']
        if type(start) is not int or type(end) is not int or not 0<=start<end<=len(text):raise Mismatch('excerpt source range')
        try:excerpt=json.loads(text[start:end])
        except (ValueError,UnicodeError) as exc:raise Mismatch('excerpt does not expose original words') from exc
        name,proto=span['method'].split('(',1)
        methods=[m for c in original['classes'] if c['descriptor']==span['owner'] for m in c['methods'] if m['name']==name and m['proto']=='('+proto]
        if len(methods)!=1 or methods[0]['code'] is None:raise Mismatch('excerpt has no original code identity')
        code=methods[0]['code'];words=code.get('raw_words',code.get('units'))
        first=excerpt['code_unit_start'];actual=excerpt['words']
        if type(first) is not int or first<0 or not isinstance(actual,list) or not actual or any(type(w) is not int or not 0<=w<=65535 for w in actual) or first+len(actual)>len(words) or words[first:first+len(actual)]!=actual:
            raise Mismatch('excerpt differs from the checked original word range')
    return {'textAvailable':bool(text),'sourceSpansExact':True,'allOriginalMembersPresent':set(identities)==members,
            'eachOriginalMemberOnce':len(identities)==len(members),'mustIncludeJavaMethods':java,'mustIncludeDebugMethods':raw,
            'failedMethodHasNoJavaBody':all(s['language']!='java' for s in spans if s.get('failure')),
            'rawExcerpt':bool(excerpts)}


def query_assertions(value, original):
    rows=value['rows']; keys=[(r['owner'],r['name'],r['proto'],r.get('pc'),r.get('operand_slot')) for r in rows]
    if len(set(keys))!=len(keys):raise Mismatch('duplicate original query identity')
    defined=[c['descriptor']+'->'+m['name']+m['proto'] for c in original['classes'] for m in c['methods']]
    names=[r['owner']+'->'+r['name']+r['proto'] for r in rows]
    sites=[(c['descriptor'],m['name'],m['proto'],i['pc'],0) for c in original['classes'] for m in c['methods'] if m['code']
           for i in m['code']['instructions'] if i.get('reference',{}).get('owner')=='Lfixtures/Dep;' and i['reference'].get('name')=='f']
    return {'definedMethods':names,'siteCount':len(rows),'referencingMethod':'Case.target' if all(r['owner']=='Lfixtures/Case;' and r['name']=='target' for r in rows) and rows else None,
            'allCertifiedSites':Counter(keys)==Counter(sites),'mustNotInclude':names,'_definitions_complete':Counter(names)==Counter(defined)}


def compare_suite(root, spec, manifest, records, case_ids=None, reference=False):
    from e2e import load
    if not manifest['cases'] or manifest['spec']['revision']!=spec['specRevision']:raise Mismatch('empty/stale manifest')
    expected={(c['id'],i['id'],s):(c,i) for c,i,s in required_runs(spec,case_ids)}
    actual={}
    for record in records:
        key=(record['case_id'],record['invocation_id'],record['seam'])
        if key in actual:raise Mismatch('duplicate invocation/seam')
        actual[key]=record
    if set(expected)!=set(actual):raise Mismatch('missing/extra invocation or seam')
    assets={a['id']:a for a in manifest['assets']}
    bindings={(c['id'],i['id']):i for c in manifest['cases'] for i in c['invocations']}
    for key,(case,invocation) in expected.items():
        result=actual[key];cert=bindings[key[:2]]
        if cert['certificate']['result']!='inputs-certified':raise Mismatch('uncertified original input')
        for identity in cert['input_assets']:
            a=assets[identity];path=root/a['path']
            if not path.is_file() or path.stat().st_size!=a['size']:raise Mismatch('missing/changed original artifact')
        if result['spec_revision']!=spec['specRevision']:raise Mismatch('wrong spec revision')
        if result['execution']!='completed':raise Mismatch('execution '+result['execution'])
        if result['producer']!=('reference-falsification' if reference else 'diana-public-library'):raise Mismatch('ineligible result producer')
        original_asset=assets[cert['input_assets'][0]]
        projection_path=str(Path(original_asset['path']).with_suffix('.dexlib.json'))
        projection_asset=next((a for a in assets.values() if a['path']==projection_path),None)
        original=None
        if projection_asset is not None:
            p=root/projection_path
            if not p.is_file() or p.stat().st_size!=projection_asset['size']:raise Mismatch('missing/changed certified independent projection')
            original=load(p)
        layout_asset=next((a for a in assets.values() if a['path']==original_asset['path']+'.layout.json'),None)
        if layout_asset is not None and original_asset['path'].endswith('.dex'):
            path=root/layout_asset['path']
            if not path.is_file() or path.stat().st_size!=layout_asset['size']:raise Mismatch('changed original physical projection')
            logical=cert['bindings'][0]['origin']['logical_ordinal'] or 0
            raw_original=load(path)['members'][logical]
            original=original_debug_facts(original,raw_original) if original is not None else raw_original
        groups={}
        for group,artifact in result['artifacts'].items():
            p=root/artifact['path']
            if not p.resolve().is_relative_to(root.resolve()) or not p.is_file():raise Mismatch('missing/changed result artifact')
            groups[group]=p.read_bytes() if group=='text' else load(p)
        public=groups.get('outcome')
        if public is None:raise Mismatch('missing public classification')
        if not reference:
            from .materialize import observations
            groups=observations(root,case,invocation,cert,manifest,result,groups)
            public=groups['outcome']
        measured={'outcome':public_assertions(public),'diagnostic':diagnostic_assertions(public)}
        if 'driver' in groups:
            observed=groups['driver']
            if (observed['case'],observed['invocation'])!=key[:2]:raise Mismatch('wrong Driver identity')
            driver_assertions(spec,invocation['expected'],observed)
            measured['driver']={}
        if 'debug' in groups:
            pair=groups['debug']
            if original is None:raise Mismatch('debug lacks a certified original projection')
            selected=[b['selector'] for b in cert['bindings'] if b['selector']['kind']=='method']
            if not selected:
                selected=[{'descriptor':m['owner'],'name':m['name'],'proto':m['proto']} for m in public.get('members',[]) if m['language']=='dex_debug']
            if not selected:raise Mismatch('debug snapshot lacks selected original method identities')
            measured['debug']=debug_assertions(original,pair['actual'],selected)
            if selected:
                bodies=[m['code'] is not None for s in selected for c in pair['actual']['classes'] if c['descriptor']==s['descriptor'] for m in c['methods'] if m['name']==s['name'] and m['proto']==s['proto']]
                if len(bodies)!=len(selected):raise Mismatch('missing selected debug declaration')
                measured.setdefault('declaration',{}).update(hasBody=any(bodies),protoPreserved=True)
        if 'declaration' in groups:
            pair=groups['declaration']
            if original is None:raise Mismatch('declarations lack a certified original projection')
            measured['declaration']=declaration_assertions(original,pair['compiled'])
        if 'rendered' in groups:
            pair=groups['rendered']
            if original is None:raise Mismatch('spans lack certified original identities')
            measured['rendered']=rendered_assertions(pair,groups['text'],original)
        if 'query' in groups:
            pair=groups['query']
            if original is None:raise Mismatch('query lacks certified original identities')
            permitted_origins=[b['origin'] for b in cert['bindings']]
            for row in pair['rows']:
                if row.get('origin') not in permitted_origins:raise Mismatch('query row has the wrong original scope')
                methods=[m for c in original['classes'] if c['descriptor']==row['owner'] for m in c['methods'] if m['name']==row['name'] and m['proto']==row['proto']]
                if len(methods)!=1 or row.get('method_id_idx')!=methods[0]['method_id_idx']:raise Mismatch('query row does not resolve to an original method identity')
            measured['query']=query_assertions(pair,original)
        if 'filesystem' in groups:
            from .filesystem import assertions
            measured['filesystem']=assertions(root,groups['filesystem'])
            if 'otherErrorKind' in measured['filesystem']:
                measured['diagnostic']['otherErrorKind']=measured['filesystem']['otherErrorKind']
        for field,want in invocation['expected'].items():
            source=spec['assertionSources'][field]['source']
            if source=='driver':
                if 'driver' not in measured:raise Mismatch('missing real Driver observation')
                continue
            if source not in measured or field not in measured[source]:raise Mismatch('missing observation for '+field)
            got=measured[source][field]
            if field=='mustNotInclude':
                if any(item in got for item in want):raise Mismatch('referenced-only member reported as definition')
            elif field in {'definedMethods','mustIncludeJavaMethods','mustIncludeDebugMethods'}:
                if field=='definedMethods' and Counter(got)!=Counter(want) or field!='definedMethods' and not set(want)<=set(got):raise Mismatch('original member multiset mismatch')
            elif field=='observationsOnly':
                if not got or not set(got)<=set(want):raise Mismatch('forbidden/empty file observation sequence')
            else:exact(want,got,'$.'+field)
    return {'execution':'reference-verified' if reference else 'passed','cases':len({k[0] for k in expected}), 'invocations':len({k[:2] for k in expected}), 'seams':len(expected)}
