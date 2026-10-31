"""Bind certified bytes, original selectors, reference artifacts and spec pointers."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import zipfile

from . import classfile
from .compare import driver_values, exact
from .requirements import need, method
from .scalar import execute

# Only declared JDK dependencies of these fixtures, not an Android/platform
# catalog. Java8 signatures come from the locked JDK's --release 8 ct.sym.
JDK_FIXTURE_TYPES = [
    'java/lang/Object','java/lang/String','java/lang/StringBuilder','java/lang/Class',
    'java/lang/Enum','java/lang/Exception','java/lang/RuntimeException',
    'java/lang/Throwable','java/lang/Thread','java/lang/annotation/ElementType',
    'java/lang/annotation/RetentionPolicy','java/util/ArrayList','java/util/List',
    'java/lang/invoke/CallSite','java/lang/invoke/MethodHandles$Lookup','java/lang/invoke/MethodType',
]


def jdk_declarations(root, cache, tools):
    from e2e import relative, save
    result = {}
    with zipfile.ZipFile(tools.home / 'lib/ct.sym') as archive:
        names = archive.namelist()
        pending = list(JDK_FIXTURE_TYPES)
        while pending:
            name = pending.pop()
            descriptor = 'L'+name+';'
            if descriptor in result:
                continue
            suffix = '/java.base/' + name + '.sig'
            matches = [p for p in names if '8' in p.split('/')[0] and p.endswith(suffix)]
            need(len(matches)==1, 'missing/ambiguous Java8 JDK declaration: ' + name)
            path = cache / 'catalog/jdk8' / (name + '.class')
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes(archive.read(matches[0]))
            value = classfile.project(path)
            result[descriptor] = {'path':relative(path),'ct_sym_entry':matches[0],'declaration':value}
            for parent in ([value['parent']] if value['parent'] else []) + ['L'+i+';' for i in value['interfaces']]:
                if parent not in result:
                    pending.append(parent[1:-1])
    save(cache/'catalog/jdk8.json',result)
    return result


def input_path(case, invocation, certificate):
    cid = case['id']; paths = certificate['assets']; role = invocation['input'].get('targetRole','case')
    desired = None
    if cid == 'D055': desired = invocation['id']+'.dex'
    elif cid in {'D056','D059'}: desired = 'mutated.dex'
    elif cid in {'D074','D075','D076'}: desired = role+'.dex'
    elif cid == 'D069': desired = {'bad-crc':'bad-crc.zip','inflate-budget':'over-budget.zip','unsupported-compression':'unsupported.zip','encrypted-entry':'encrypted.zip'}[invocation['id']]
    if desired:
        matches = [a for a in paths if Path(a['path']).name==desired]
    else:
        matches = paths
    need(len(matches)==1, 'ambiguous original input asset: '+cid+'/'+invocation['id'])
    return matches[0]


def selected_roles(case, invocation):
    roles=[]
    def walk(value):
        if isinstance(value,dict):
            for key,item in value.items():
                if key=='targetRole': roles.append(item)
                elif key=='targetRoles': roles.extend(item)
                else: walk(item)
        elif isinstance(value,list):
            for item in value: walk(item)
    walk(invocation['input'])
    if not roles:
        roles = list(case['targets'])
    return list(dict.fromkeys(roles))


def scalar_crosscheck(case, certificate, root):
    """Cross-check independently decoded pure DEX against real source JVM runs."""
    from e2e import load
    supported = {'D001','D002','D003','D004','D005','D007','D008','D009','D010','D011','D012','D013','D014','D072','D083'}
    if case['id'] not in supported:
        return []
    evidence=[]
    asset=certificate['assets'][0]
    decoded=load(root/asset['dexlib2']); raw=load(root/asset['layout'])['members'][0]
    references={r['invocation']:r for r in certificate['references']}
    for invocation in case['invocations']:
        inp=invocation['input']; selector=inp.get('method')
        if not selector or selector.startswith('n(') or invocation['expected'].get('exception'):
            continue
        name,proto=selector.split('(',1);proto='('+proto
        m=method(decoded,name,proto)
        r=next(m for c in raw['classes'] for m in c['methods'] if m['name']==name and m['proto']==proto)
        args=[int(a) if type(a) is str else a for a in inp.get('args',[])]
        actual=execute(m,r['code']['ins_size'],args)
        observed=driver_values(load(root/references[invocation['id']]['path']))
        key='returnLong' if actual['kind']=='long' else 'return'
        value=str(actual['value']) if key=='returnLong' else actual['value']
        exact(observed[key],value,'scalar/JVM '+case['id']+'/'+invocation['id'])
        evidence.append({'invocation':invocation['id'],'method':selector,'steps':actual['steps'],'reference':references[invocation['id']]['path']})
    need(bool(evidence),'empty scalar cross-check: '+case['id'])
    return evidence


def crosscheck_probe(root,cache,tools):
    from e2e import compile_java, run, load, save, sha
    import os
    from .layout import project
    work=cache/'scalar-probe';logs=work/'logs';classes=work/'classes';drivers=work/'driver'
    reference=root/'tests/e2e/runner/ScalarReference.java'
    driver=root/'tests/e2e/runner/ScalarReferenceDriver.java'
    classfiles=compile_java(tools,[reference],classes,logs,'probe')
    compile_java(tools,[driver],drivers,logs,'driver',[classes])
    dex=work/'dex';dex.mkdir(parents=True,exist_ok=True)
    run([tools.java,'-cp',tools.cp,'com.android.tools.r8.D8','--debug','--min-api','23','--lib',tools.home,'--output',dex,*classfiles],root,logs/'d8.log')
    text=run([tools.java,'-cp',os.pathsep.join(map(str,[classes,drivers])),'ScalarReferenceDriver'],root,logs/'jvm.log')
    observed=json.loads(text)
    decoded=json.loads(run([tools.java,'-cp',os.pathsep.join([str(cache/'projector/classes'),tools.cp]),'ProjectDex',dex/'classes.dex'],root,logs/'decode.log'))
    raw=project(dex/'classes.dex')['members'][0]
    for item in observed:
        name,proto=item['method'].split('(',1);proto='('+proto
        m=method(decoded,name,proto,'LScalarReference;')
        raw_method=next(m for c in raw['classes'] for m in c['methods'] if m['name']==name and m['proto']==proto)
        result=execute(m,raw_method['code']['ins_size'],[int(a) for a in item['args']])
        exact(item['value'],str(result['value']),'pure oracle/JVM '+item['method'])
    need(bool(observed),'empty pure-operation oracle cross-check')
    save(work/'crosscheck.json',{'source_sha256':sha(reference),'driver_sha256':sha(driver),'dex_sha256':sha(dex/'classes.dex'),'observations':observed})
    return len(observed)


def build_manifest(root,cache,tools,spec,certificates):
    from e2e import load, relative, save, sha
    assets={}; ids={}
    def asset(path, sources, recipe, license_='Apache-2.0'):
        path=Path(path); name=relative(path)
        if name in assets:
            return assets[name]['id']
        identity='asset-'+hashlib.sha256(name.encode()).hexdigest()[:24]
        need(identity not in ids,'asset ID collision')
        ids[identity]=name
        if recipe is not None:
            recipe={**recipe,'input_asset_ids':[assets[p]['id'] for p in sources if p in assets],
                    'output_asset_ids':[identity]}
            recipe.setdefault('argument_arrays',[])
        assets[name]={'id':identity,'path':name,'sha256':sha(path),'size':path.stat().st_size,
                      'source_paths':sources,'license':license_,'recipe':recipe}
        return identity
    for helper in [root/'tools/e2e.py', *sorted((root/'tests/e2e/runner').glob('*.py')), *sorted((root/'tests/e2e/runner').glob('*.java'))]:
        asset(helper,[relative(helper)],None)
    probe_comparisons=crosscheck_probe(root,cache,tools)
    jdk=jdk_declarations(root,cache,tools)
    jdk_path=cache/'catalog/jdk8.json'
    jdk_id=asset(jdk_path,[],{'tool_ids':['temurin-21.0.8+9'],'profile':'jdk-java8-signatures','description':'Declared fixture JDK types and their superclass/interface closure extracted from locked lib/ct.sym, not the runtime Java21 API.'})
    for value in jdk.values():
        asset(root/value['path'],[],{'tool_ids':['temurin-21.0.8+9'],'profile':'jdk-java8-signatures','archive_entry':value['ct_sym_entry']},'GPL-2.0-only WITH Classpath-exception-2.0')
    supplemental_sources=[relative(p) for p in sorted((root/'tests/e2e/sources/H03').iterdir()) if p.is_file()]
    for path in supplemental_sources:asset(root/path,[path],None)
    for path in sorted((cache/'prepared/H03').iterdir()):
        if path.is_file() and path.suffix in {'.axml','.zip','.json'} and not path.name.endswith('.command.json'):
            asset(path,supplemental_sources,{'tool_ids':['python-standard-library'],'profile':'H03-self-made-AXML-source-and-independent-projection',
                  'argument_arrays':[['<python3>','tests/e2e/sources/H03/build_axml.py',relative(cache/'prepared/H03')]]})
    cases=[]; scalar_evidence=[]
    for case,cert in zip([c for c in spec['cases'] if c['required']],certificates):
        cid=case['id'];source=root/'tests/e2e/sources'/cid;work=cache/'prepared'/cid
        source_recipe=load(source/'recipe.json')
        sources=[relative(p) for p in sorted(source.rglob('*')) if p.is_file()]
        for p in sources: asset(root/p,[p],None)
        commands=[load(p) for p in sorted(work.rglob('*.command.json'))]
        recipe={'tool_ids':['python-standard-library'],'profile':'independent-in-process-projection',
                'argument_arrays':[],'description':'Independent layout/dexlib2/classfile/requirement aggregation; the corresponding source helper and original input are content-locked.',
                'recipe_path':relative(source/'recipe.json')}
        for p in sorted(work.rglob('*')):
            if p.is_file() and p.suffix in {'.class','.dex','.zip','.json','.java'} and 'logs' not in p.parts and not p.name.endswith('.command.json'):
                producing=[]
                name=relative(p)
                for command in commands:
                    argv=command['argv']
                    if name in argv or '-d' in argv and name.startswith(argv[argv.index('-d')+1]+'/'):
                        producing.append(argv)
                    elif p.suffix=='.dex' and '--output' in argv and name.startswith(argv[argv.index('--output')+1]+'/'):
                        producing.append(argv)
                    elif p.name.endswith('.driver.json') and argv[-1]==p.name.removesuffix('.driver.json') and '-cp' in argv:
                        producing.append(argv)
                producer=recipe if not producing else {'tool_ids':['temurin-21.0.8+9','com.android.tools:r8:8.3.37','org.smali:smali:2.5.2'],
                    'profile':'self-made-fixture','argument_arrays':producing,'recipe_path':relative(source/'recipe.json')}
                asset(p,sources,producer)
        scalar_evidence+= [{'case':cid,**r} for r in scalar_crosscheck(case,cert,root)]
        classfiles=load(root/cert['classfiles'])
        catalog={'source_classfiles':classfiles,'jdk_declarations':jdk_id,
                 'explicit_facts':[relative(p) for p in sorted((source/'fixtures').glob('catalog*.json'))],
                 'completeness':'complete-except-declared-gap' if cid=='D061' else 'explicit-constantness-variant' if cid=='D080' else 'complete'}
        catalog_path=work/'catalog.json';save(catalog_path,catalog);catalog_id=asset(catalog_path,sources,recipe)
        invocations=[]
        for index,inv in enumerate(case['invocations']):
            original=input_path(case,inv,cert); original_id=assets[original['path']]['id']
            projection=load(root/original['layout']) if 'layout' in original else None
            bindings=[]
            for role in selected_roles(case,inv):
                target=case['targets'][role]
                logical=int(role[-1]) if cid=='D065' else 0
                origin={'asset_id':original_id,'entry_ordinal':0,'logical_ordinal':None,'header_offset':None}
                selector={'kind':'request','spec_input_ref':f"/cases/{spec['cases'].index(case)}/invocations/{index}/input"}
                if projection:
                    member=projection['members'][logical]
                    origin.update(logical_ordinal=logical,header_offset=member['header_offset'])
                    if target['kind'] in {'class','method'}:
                        descriptor=target.get('descriptor',target.get('owner'))
                        cls=next((c for c in member['classes'] if c['descriptor']==descriptor),None)
                        need(cls is not None,'missing original class role: '+cid+'/'+role)
                        selector={'kind':target['kind'],'descriptor':descriptor,'class_def_idx':cls['class_def_idx']}
                        selected_method=inv['input'].get('method') or source_recipe.get('requestMethods',{}).get(inv['id'])
                        if target['kind']=='method' or selected_method and case['seam'] in {'decompile-method','open-or-decompile','decompile-and-render'}:
                            method_name=target['name'] if target['kind']=='method' else selected_method.split('(',1)[0]
                            method_proto=target['proto'] if target['kind']=='method' else '('+selected_method.split('(',1)[1]
                            matches=[m for m in cls['methods'] if m['name']==method_name and m['proto']==method_proto]
                            need(len(matches)==1,'ambiguous original method role')
                            selector.update(kind='method',method_id_idx=matches[0]['method_id_idx'],name=method_name,proto=method_proto)
                elif 'zip' in original:
                    entry=inv['input'].get('entry')
                    if isinstance(entry,dict) and 'ordinal' in entry:
                        selector={'kind':'entry','name':entry['name'],'ordinal':entry['ordinal']}
                    else:
                        selector={'kind':'request','spec_input_ref':f"/cases/{spec['cases'].index(case)}/invocations/{index}/input"}
                bindings.append({'role':role,'selector':selector,'origin':origin})
            method_bindings=[]
            if projection and case['seam']=='decompile-method' and 'calls' in inv['input']:
                for call in inv['input']['calls']:
                    role=call.get('targetRole','case');target=case['targets'][role]
                    owner=target.get('descriptor',target.get('owner'));name,proto=call['method'].split('(',1);proto='('+proto
                    member=projection['members'][0];cls=next(c for c in member['classes'] if c['descriptor']==owner)
                    matches=[m for m in cls['methods'] if m['name']==name and m['proto']==proto]
                    need(len(matches)==1,'missing/non-unique grouped original method')
                    method_bindings.append({'role':role,'class_def_idx':cls['class_def_idx'],'method_id_idx':matches[0]['method_id_idx'],'name':name,'proto':proto})
            refs=[r for r in cert['references'] if r['invocation']==inv['id']]
            required_driver={k for k in inv['expected'] if spec['assertionSources'][k]['source']=='driver'}
            need(not required_driver or len(refs)==1,'missing/duplicate reference Driver: '+cid+'/'+inv['id'])
            assertions=[];case_index=spec['cases'].index(case)
            for key in inv['expected']:
                mapping=spec['assertionSources'][key]
                assertions.append({'expected_ref':f'/cases/{case_index}/invocations/{index}/expected/{key}',**mapping})
            invocations.append({'id':inv['id'],'input_assets':[original_id],
                'expected_ref':f'/cases/{case_index}/invocations/{index}/expected','bindings':bindings,
                'request':{'seam':case['seam'],'input_ref':f'/cases/{case_index}/invocations/{index}/input','through':inv['input'].get('through',[case['seam']]),'method_bindings':method_bindings},
                'catalog':{'asset_id':catalog_id,'completeness':catalog['completeness']},
                'driver':None if not refs else {'reference_assets':[assets[r['path']]['id'] for r in refs], 'recipe_path':relative(source/'recipe.json'),'isolation':'fresh-jvm-per-invocation','kind':refs[0]['kind']},
                'checks':cert['checks'],'assertions':assertions,
                'certificate':{'result':'inputs-certified','validationScope':case['validationScope'],'platform':tools.platform,
                               'toolchain_sha256':sha(root/'tests/e2e/toolchain.lock.json'),'evidence_assets':[assets[relative(work/'certificate.json')]['id']], 'art':'not-run'}})
        cases.append({'id':cid,'invocations':invocations})
    need(bool(scalar_evidence),'no scalar/JVM cross-checks')
    save(cache/'scalar-crosscheck.json',{'producer':'pure-register-oracle-crosschecked-with-real-JVM','observations':scalar_evidence,'pure_operation_probe_comparisons':probe_comparisons})
    result={'schema_version':1,'spec':{'revision':spec['specRevision'],'path':'docs/diana/e2e_cases.json','sha256':sha(root/'docs/diana/e2e_cases.json')},
            'toolchain_lock':{'path':'tests/e2e/toolchain.lock.json','sha256':sha(root/'tests/e2e/toolchain.lock.json')},
            'assets':list(assets.values()),'cases':cases}
    locked_path=root/'tests/e2e/manifest.json'
    if locked_path.exists():
        previous=load(locked_path)
        if previous['spec']['sha256']==result['spec']['sha256'] and previous['toolchain_lock']['sha256']==result['toolchain_lock']['sha256']:
            def input_hashes(value):
                assets_by_id={a['id']:a for a in value['assets']}
                return {(c['id'],i['id']):[assets_by_id[a]['sha256'] for a in i['input_assets']] for c in value['cases'] for i in c['invocations']}
            need(input_hashes(previous)==input_hashes(result),'original fixture hash drift: review recipe/tool difference before replacing the input lock')
    save(cache/'manifest.json',result)
    # Only checked certificates may materialize the source/input lock. This never
    # writes the independent case inputs or expected values back to the spec.
    save(locked_path,result)
    print('input certification:',len(cases),'cases;',sum(len(c['invocations']) for c in cases),'invocations;',len(scalar_evidence)+probe_comparisons,'scalar/JVM cross-checks')
