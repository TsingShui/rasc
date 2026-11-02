"""Materialize actual Java/debug observations from public adapter artifacts.

Java is compiled and executed here. Non-Java information comes from the actual
public member source, parsed as a debug snapshot; no output assembler is used.
"""
from __future__ import annotations
from collections import Counter
import json
import os
from pathlib import Path
import tempfile
import time
from . import classfile
from .compare import Mismatch
from .evaluate import normalized_debug


def observations(root,case,invocation,certificate,manifest,result,groups,timing=None):
    from e2e import Tools, compile_java, load, run
    assets={a['id']:a for a in manifest['assets']}
    original=assets[certificate['input_assets'][0]]
    cache=root/original['path'].split('/prepared/',1)[0]
    tools=None
    if 'java_input' in groups:
        tools=Tools(cache);tools.verify()
    recipe_root=root/'tests/e2e/sources'/case['id'];recipe=load(recipe_root/'recipe.json')
    public=groups['outcome']
    if Counter(json.dumps(b,sort_keys=True) for b in public.get('selected',[]))!=Counter(json.dumps(b,sort_keys=True) for b in certificate['bindings']):
        raise Mismatch('public result selection/original scope does not match the certified request')
    if public['outcome']=='java' and case['validationScope'] in {'full-unit','full-unit-set'} and public.get('status')!='java_unit':
        raise Mismatch('a fragment is not a complete required JavaUnit')
    def checked_sources(record, suffix):
        sources=[]
        for artifact in record['sources']:
            path=root/artifact['path']
            if path.suffix!=suffix or not path.resolve().is_relative_to(root.resolve()) or not path.is_file():
                raise Mismatch('missing/changed submitted source')
            sources.append(path)
        if not sources:raise Mismatch('empty submitted source set')
        return sources
    # Controlled subprocesses are bounded by e2e.run; no source output is silently
    # reclassified as Unsupported after a compiler/Driver error.
    with tempfile.TemporaryDirectory(dir=cache,prefix='result-') as directory:
        work=Path(directory);logs=work/'logs'
        if 'java_input' in groups:
            sources=checked_sources(groups['java_input'],'.java')
            dependencies=[recipe_root/p for p in recipe.get('dependencies',{}).get('sources',[])]
            locked_paths={a['path']:a for a in manifest['assets']}
            for path in dependencies:
                entry=locked_paths[path.relative_to(root).as_posix()]
                if not path.is_file() or path.stat().st_size!=entry['size']:raise Mismatch('missing/changed dependency')
            classes=work/'classes'
            started = time.monotonic()
            classfiles=compile_java(tools,sources+dependencies,classes,logs,'candidate')
            if timing is not None:timing['candidate_compile_seconds'] = time.monotonic() - started
            compiled=[classfile.project(p) for p in classfiles]
            required_owners={b['selector']['descriptor'] for b in certificate['bindings'] if b['selector']['kind'] in {'class','method'}}
            if not required_owners <= {c['descriptor'] for c in compiled}:raise Mismatch('compiled output lacks selected original owner')
            if case['validationScope'] in {'full-unit','full-unit-set'}:
                original_path=root/str(Path(original['path']).with_suffix('.dexlib.json'))
                original_doc=load(original_path)
                compiled_map={c['descriptor']:c for c in compiled}
                for c in original_doc['classes']:
                    if c['descriptor'] not in required_owners:continue
                    actual_class=compiled_map[c['descriptor']]
                    if {(m['name'],m['proto']) for m in c['methods']}!={(m['name'],m['proto']) for m in actual_class['methods']}:
                        raise Mismatch('full unit lost/added an original method or generated member')
                    if {(f['name'],f['type']) for f in c['fields']}!={(f['name'],f['proto']) for f in actual_class['fields']}:
                        raise Mismatch('full unit lost/added an original field')
            groups['declaration']={'compiled':compiled}
            if certificate['driver'] is not None:
                driver=recipe.get('driver') or {}
                args=[invocation['id']]
                if case['id'] in {'D027','D046','D073'}:
                    driver={'class':'fixtures.Driver','sources':['fixtures/Driver.java']}
                elif case['id']=='D065':
                    driver={'class':'driver.Driver','sources':['driver/Driver.java']}
                elif driver.get('perRole'):
                    role=invocation['input']['targetRole'];entry=next(c for c in driver['classes'] if c['role']==role)
                    driver={'class':entry['class'],'sources':[entry['path']]}
                elif case['id']=='D076':
                    driver={**driver,'sources':['driver/Driver.java']};args=[invocation['input']['targetRole'],invocation['id']]
                elif 'sources' not in driver:
                    driver={**driver,'sources':['driver/Driver.java']}
                driver_classes=work/'driver'
                started = time.monotonic()
                compile_java(tools,[recipe_root/p for p in driver['sources']],driver_classes,logs,'driver',[classes])
                if timing is not None:timing['driver_compile_seconds'] = time.monotonic() - started
                started = time.monotonic()
                text=run([tools.java,'-cp',os.pathsep.join(map(str,[driver_classes,classes])),driver['class'],*args],root,logs/'driver-run.log')
                if timing is not None:timing['driver_run_seconds'] = time.monotonic() - started
                groups['driver']=json.loads(text)
        elif groups['outcome']['outcome'] in {'java','declaration-only'}:
            raise Mismatch('Java result has no compilable submitted source')
        elif 'driver' in groups:
            raise Mismatch('Driver observation is not from compiled recovered source')
        snapshots=[];member_sources={}
        for member in public.get('members',[]):
            if member['language']!='dex_debug' or member['status']=='unavailable':continue
            source=member['source']
            try:snapshot=json.loads(source)
            except (ValueError,TypeError) as exc:raise Mismatch('public debug source is not a readable snapshot') from exc
            if snapshot.get('schema')!='diana.dex-debug/1' or type(snapshot.get('complete')) is not bool:
                raise Mismatch('public debug source lacks its schema/coverage')
            identities={(c['descriptor'],m['name'],m['proto']) for c in snapshot['classes'] for m in c['methods']}
            identity=(member['owner'],member['name'],member['proto'])
            if identities!={identity} or identity in member_sources:raise Mismatch('debug source does not identify exactly its original member')
            member_sources[identity]=source.encode('utf-8');snapshots.append(snapshot)
        if snapshots:
            merged={};undecoded=[]
            for snapshot in snapshots:
                undecoded.extend(snapshot.get('undecoded_ranges',[]))
                for c in snapshot['classes']:
                    owner=c['descriptor'];header={**c,'methods':[]}
                    if owner in merged and normalized_debug({'classes':[header]})!=normalized_debug({'classes':[{**merged[owner],'methods':[]}]}):
                        raise Mismatch('inconsistent original debug declaration context')
                    if owner not in merged:merged[owner]=header
                    merged[owner]['methods'].extend(c['methods'])
            groups['debug']={'actual':{'schema':'diana.dex-debug/1','complete':all(s['complete'] for s in snapshots),'classes':list(merged.values()),'undecoded_ranges':undecoded}}
            for span in groups.get('rendered',{}).get('spans',[]):
                if span['language']=='dex_debug':
                    name,proto=span['method'].split('(',1);identity=(span['owner'],name,'('+proto)
                    if member_sources.get(identity)!=groups['text'][span['start']:span['end']]:raise Mismatch('rendered debug slice differs from the actual public member source')
        elif 'debug' in groups:
            raise Mismatch('debug facts did not come from actual public member source')
    return groups
