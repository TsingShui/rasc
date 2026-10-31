"""Materialize actual Java/raw observations from public adapter artifacts.

The adapter cannot supply a successful compile flag, a canned Driver result or a
rawExact boolean. Compilation, execution and independent reassembly happen here.
"""
from __future__ import annotations
from collections import Counter
import json
import os
from pathlib import Path
import tempfile
from . import classfile
from .compare import Mismatch


def observations(root,case,invocation,certificate,manifest,result,groups):
    from e2e import Tools, compile_java, load, run, sha
    assets={a['id']:a for a in manifest['assets']}
    original=assets[certificate['input_assets'][0]]
    cache=root/original['path'].split('/prepared/',1)[0]
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
            if path.suffix!=suffix or not path.resolve().is_relative_to(root.resolve()) or not path.is_file() or sha(path)!=artifact['sha256']:
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
                if sha(path)!=entry['sha256']:raise Mismatch('unchanged dependency was edited')
            classes=work/'classes'
            classfiles=compile_java(tools,sources+dependencies,classes,logs,'candidate')
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
                compile_java(tools,[recipe_root/p for p in driver['sources']],driver_classes,logs,'driver',[classes])
                text=run([tools.java,'-cp',os.pathsep.join(map(str,[driver_classes,classes])),driver['class'],*args],root,logs/'driver-run.log')
                groups['driver']=json.loads(text)
        elif groups['outcome']['outcome'] in {'java','declaration-only'}:
            raise Mismatch('Java result has no compilable submitted source')
        elif 'driver' in groups:
            raise Mismatch('Driver observation is not from compiled recovered source')
        if 'raw_input' in groups:
            sources=checked_sources(groups['raw_input'],'.smali')
            dex=work/'raw.dex'
            run([tools.java,'-cp',tools.cp,'org.jf.smali.Main','assemble','--api','28' if original['path'].endswith('D085/case.dex') else '23','-o',dex,*sources],root,logs/'assemble.log')
            if not dex.is_file():raise Mismatch('assembler produced no complete artifact')
            text=run([tools.java,'-cp',os.pathsep.join([str(cache/'projector/classes'),tools.cp]),'ProjectDex',dex],root,logs/'decode.log',output_cap=64*1024*1024)
            groups['raw']={'actual':json.loads(text)}
            for span in groups.get('rendered',{}).get('spans',[]):
                if span['language']=='smali':
                    binding=span['raw_slice'];path=root/binding['path']
                    if path not in sources:raise Mismatch('raw span is not the assembled source')
                    source=path.read_bytes()[binding['start']:binding['end']]
                    if source!=groups['text'][span['start']:span['end']]:raise Mismatch('rendered raw slice differs from independently assembled code')
        elif 'raw' in groups:
            raise Mismatch('raw observation is not from independently assembled output')
    return groups
