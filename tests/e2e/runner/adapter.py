"""Translate the public Engine development example into existing E2E artifacts.

The process is a development caller of the Rust library, not the product CLI.
Unimplemented request capabilities remain explicit records in the required set.
"""
from __future__ import annotations
import copy
import json
import os
import tempfile
from pathlib import Path


def required_runs(spec, case_ids=None):
    selected = None if case_ids is None else set(case_ids)
    runs=[]
    for case in spec['cases']:
        if case['required'] and (selected is None or case['id'] in selected):
            for invocation in case['invocations']:
                for seam in invocation['input'].get('through',[case['seam']]):
                    runs.append((case,invocation,seam))
    if not runs:
        raise ValueError('zero selected cases/invocations')
    if selected is not None and selected != {c['id'] for c,_,_ in runs}:
        raise ValueError('unknown/unrequired selected case')
    return runs


def run_public(root, cache, spec, manifest, case_ids=None,representation=None):
    from e2e import PreparationError, relative, run, save
    from .evaluate import compare_suite
    from .compare import Mismatch
    runs = required_runs(spec, case_ids)
    assets = {a['id']:a for a in manifest['assets']}
    certified = {(c['id'],i['id']):i for c in manifest['cases'] for i in c['invocations']}
    base = cache/'engine'
    base.mkdir(parents=True,exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix='run-',dir=base))
    run(['cargo','build','-p','diana-decomp','--release','--example','inspect_class','--locked'], root, output/'build.log', timeout=180)
    executable = root/'target/release/examples'/('inspect_class.exe' if os.name=='nt' else 'inspect_class')
    revision=run(['git','rev-parse','HEAD'],root,output/'revision.log').strip()
    dirty=bool(run(['git','status','--porcelain','--untracked-files=no','--','crates/diana-decomp','tests/e2e/runner/adapter.py','tools/e2e.py'],root,output/'working-tree.log').strip())
    records = []
    for case, invocation, seam in runs:
        record = {'schema_version':1,'spec_revision':spec['specRevision'],'case_id':case['id'],
                  'invocation_id':invocation['id'],'seam':seam,'producer':'diana-public-library',
                  'execution':'not_implemented','artifacts':{},'engine_revision':{'commit':revision,'working_tree_dirty':dirty}}
        cert = certified[(case['id'],invocation['id'])]
        work = output/case['id']/invocation['id']/seam
        try:
            if seam not in {'decompile-method','decompile-class','decompile-and-render','open-or-decompile','engine-original-view'}:
                record['reason'] = 'the public Engine adapter does not implement this seam'
            elif len(cert['bindings']) != 1:
                record['reason'] = 'multi-target public requests are not implemented'
            else:
                binding = cert['bindings'][0]
                selector, origin = binding['selector'], binding['origin']
                asset = assets[origin['asset_id']]
                if not asset['path'].endswith('.dex'):
                    record['reason'] = 'the application input boundary is not implemented'
                elif selector['kind'] not in {'class','method','request'}:
                    record['reason'] = 'the requested selector is not implemented'
                else:
                    path = root/asset['path']
                    if not path.is_file() or path.stat().st_size != asset['size']:
                        raise PreparationError('missing/changed certified input')
                    if selector['kind']=='request':
                        target=case['targets'][binding['role']]
                        if target['kind']!='method':raise PreparationError('symbolic caller selection requires a method target')
                        argv=[executable,path,'0',target['owner'],target['name'],target['proto']]
                    else:
                        argv = [executable,path,str(origin['header_offset']),str(selector['class_def_idx'])]
                        if selector['kind']=='method':argv.extend([selector['name'],selector['proto']])
                    control_path=work/'controls.json'
                    controls={key:invocation['input'][key] for key in ('limits','cancellation') if key in invocation['input']}
                    if representation is not None:controls['representation']=representation
                    save(control_path,controls)
                    argv.extend(['--control',control_path])
                    record['request_controls']={'path':relative(control_path)}
                    catalog_path=work/'method-signatures.json'
                    save(catalog_path,_catalog(root,assets,cert['catalog']['asset_id'],invocation['input'].get('javaConstantKind')))
                    argv.extend(['--catalog',catalog_path])
                    public_path=work/'public.json'
                    relative(public_path)
                    public_path.unlink(missing_ok=True)
                    argv.extend(['--output',public_path])
                    run(argv,root,work/'library.log')
                    from e2e import load
                    actual=load(public_path)
                    calls=invocation['input'].get('calls',[]) if case['validationScope']=='method-in-known-context' else []
                    requested=list(dict.fromkeys(call['method'] for call in calls))
                    groups = _artifacts(actual, cert,requested)
                    executable_sources = groups.pop('method_sources',None)
                    if executable_sources is not None:
                        if case['validationScope']!='method-in-known-context':raise ValueError('method fragment is not a complete JavaUnit')
                        source_path=work/'context'/Path(selector['descriptor'][1:-1]+'.java')
                        source_path.parent.mkdir(parents=True,exist_ok=True)
                        original_path=root/'tests/e2e/sources'/case['id']/Path(selector['descriptor'][1:-1]+'.java')
                        import re
                        support_sources=[]
                        if not original_path.is_file() and case['id']=='D065':
                            ordinal=origin['logical_ordinal']
                            original_path=root/'tests/e2e/sources/D065/fixtures/mirror'/('logical'+str(ordinal))/'Case.java'
                            original_source=original_path.read_text(encoding='utf-8')
                            original_source,count=re.subn(r'^package logical'+str(ordinal)+r';','package fixtures;',original_source,count=1)
                            if count!=1:raise ValueError('certified mirror context has a different namespace')
                        else:
                            if not original_path.is_file():
                                generated=cache/'prepared'/case['id']/Path(selector['descriptor'][1:-1]+'.java')
                                generated_asset=next((asset for asset in assets.values() if asset['path']==relative(generated)),None)
                                if generated_asset is not None:
                                    original_path=generated
                                    if not generated.is_file() or generated.stat().st_size!=generated_asset['size']:raise ValueError('missing/changed generated original context')
                                    for command in generated_asset['recipe']['argument_arrays']:
                                        if not command or Path(command[0]).name!='javac':continue
                                        for argument in command[1:]:
                                            if not argument.endswith('.java') or root/argument==original_path:continue
                                            path=relative(root/argument)
                                            dependency=next((asset for asset in assets.values() if asset['path']==path),None)
                                            if dependency is None:raise ValueError('generated context dependency is absent from the manifest')
                                            if not (root/path).is_file() or (root/path).stat().st_size!=dependency['size']:raise ValueError('missing/changed generated context dependency')
                                            if path not in support_sources:support_sources.append(path)
                                else:original_path=cache/'prepared'/case['id']/'reference'/binding['role']/Path(selector['descriptor'][1:-1]+'.java')
                            original_source=original_path.read_text(encoding='utf-8')
                        for fragment in executable_sources:
                            pattern=r'(?m)^([ \t]*(?:public|private|protected|static|final|strictfp|synchronized|\s)+(?:void|int|long|float|double|boolean|byte|short|char|(?:java\.lang\.)?String|[\w.$]+(?:\[\])*)\s+'+re.escape(fragment['name'])+r'\s*\([^)]*\)\s*)\{'
                            matches=list(re.finditer(pattern,original_source))
                            if len(matches)!=1:raise ValueError('known source context does not identify one selected method')
                            match=matches[0];position=match.end();depth=1
                            while position<len(original_source) and depth:
                                if original_source[position]=='{':depth+=1
                                elif original_source[position]=='}':depth-=1
                                position+=1
                            if depth:raise ValueError('unterminated known method context')
                            original_source=original_source[:match.start()]+fragment['source']+original_source[position:]
                        source_path.write_text(original_source,encoding='utf-8')
                        groups['java_input']={'sources':[{'path':relative(source_path)},*({'path':path} for path in support_sources)]}
                    context = groups.pop('declaration_source',None)
                    if context is not None:
                        if case['validationScope']!='method-in-known-context':raise ValueError('declaration fragment has no accepted compilation context')
                        source_path=work/'context'/Path(selector['descriptor'][1:-1]+'.java')
                        source_path.parent.mkdir(parents=True,exist_ok=True)
                        prefix=('package '+context['package']+';\n') if context['package'] else ''
                        source_path.write_text(prefix+'public '+('abstract ' if context['abstract'] else '')+'class '+context['name']+' {\n'+context['source']+'\n}\n',encoding='utf-8')
                        groups['java_input']={'sources':[{'path':relative(source_path)}]}
                    for group,value in groups.items():
                        path = work/('source.txt' if group=='text' else group+'.json')
                        if group=='text':path.write_bytes(value)
                        else:save(path,value)
                        record['artifacts'][group]={'path':relative(path)}
                    record['execution']='completed'
                    record['catalog']={'asset_id':cert['catalog']['asset_id'],'applied':'signatures and explicitly covered class directories'}
        except (PreparationError,OSError,ValueError,KeyError,TypeError,IndexError) as error:
            record['execution']='failed';record['reason']=str(error)
        records.append(record)
        save(work/'record.json',record)
    save(output/'records.json',records)
    results=[]
    for case,invocation,seam in runs:
        record=next(r for r in records if (r['case_id'],r['invocation_id'],r['seam'])==(case['id'],invocation['id'],seam))
        case_spec=copy.deepcopy(case);case_spec['seam']=seam
        case_spec['invocations']=[copy.deepcopy(invocation)]
        case_spec['invocations'][0]['input']['through']=[seam]
        local_spec={**spec,'cases':[case_spec]}
        verdict={'case_id':case['id'],'invocation_id':invocation['id'],'seam':seam,'execution':'failed'}
        try:
            compare_suite(root,local_spec,manifest,[record])
            verdict['execution']='passed'
        except (Mismatch,PreparationError,OSError,ValueError,KeyError,TypeError,IndexError) as error:
            verdict['reason']=str(error)
        results.append(verdict)
        print(case['id'],invocation['id'],seam,verdict['execution'],verdict.get('reason',''),flush=True)
    selected_cases={c['id'] for c,_,_ in runs}
    passed={cid for cid in selected_cases if all(r['execution']=='passed' for r in results if r['case_id']==cid)}
    all_runs=required_runs(spec)
    report={'execution':'passed' if len(passed)==len(selected_cases) else 'failed',
            'spec_revision':spec['specRevision'],'engine_revision':{'commit':revision,'working_tree_dirty':dirty},
            'profile':'native-public-library; certified declarations with per-class coverage; caller limits/cancellation; recovery disabled',
            'representation':representation or 'structured_then_dispatch',
            'full_required_set':case_ids is None,'selected_cases':len(selected_cases),
            'selected_invocations':len({(c['id'],i['id']) for c,i,_ in runs}),'selected_seams':len(runs),
            'passed_cases':len(passed),'passed_seams':sum(r['execution']=='passed' for r in results),
            'required_cases':len({c['id'] for c,_,_ in all_runs}),
            'required_invocations':len({(c['id'],i['id']) for c,i,_ in all_runs}),
            'required_seams':len(all_runs),'results':results}
    save(output/'report.json',report)
    save(base/'latest.json',{'report':relative(output/'report.json')})
    print('report',relative(output/'report.json'),flush=True)
    return report


def _catalog(root, assets, identity,field_kind=None):
    from e2e import PreparationError, load
    by_path={a['path']:a for a in assets.values()}
    def checked(asset):
        path=root/asset['path']
        if not path.resolve().is_relative_to(root.resolve()) or not path.is_file() or path.stat().st_size!=asset['size']:
            raise PreparationError('missing/changed declaration evidence')
        return load(path)
    catalog=checked(assets[identity])
    api_declarations=[value['declaration'] for value in checked(assets[catalog['jdk_declarations']]).values()]
    declarations=list(api_declarations)
    source_declarations=list(catalog['source_classfiles'].values())
    declarations.extend(source_declarations)
    explicit_entries=[]
    for path in catalog['explicit_facts']:
        for entry in checked(by_path[path])['entries']:
            explicit_entries.append(entry)
            if entry.get('declared') is True:declarations.append(entry)
    methods=[]
    def units(text):
        data=text.encode('utf-16-le',errors='surrogatepass')
        return [int.from_bytes(data[i:i+2],'little') for i in range(0,len(data),2)]
    for declaration in declarations:
        for method in declaration['methods']:
            methods.append({'owner':units(declaration['descriptor']),'name':units(method['name']),'prototype':units(method['proto'])})
    classes=[]
    for declaration,coverage in [(declaration,'exported_api') for declaration in api_declarations]+[(declaration,'all_declarations') for declaration in source_declarations]:
        attributes=declaration['attributes']
        classes.append({'descriptor':units(declaration['descriptor']),'access_flags':declaration['flags'],'parent':None if declaration['parent'] is None else units(declaration['parent']),
                        'interfaces':[units(name) for name in declaration['interfaces']],
                        'methods':[{'name':units(method['name']),'prototype':units(method['proto']),'access_flags':method['flags'],'throws':None if method.get('throws') is None else [units(name) for name in method['throws']],'generic':'Signature' in method['attributes']} for method in declaration['methods']],
                        'field_names':[units(field['name']) for field in declaration['fields']],
                        'coverage':coverage,
                        'generic':'Signature' in attributes,'enclosing_scope':any(attribute in attributes for attribute in ('InnerClasses','EnclosingMethod','NestHost','NestMembers'))})
    fields={}
    for declaration,coverage in [(declaration,'exported_api') for declaration in api_declarations]+[(declaration,'all_declarations') for declaration in source_declarations]:
        for field in declaration['fields']:
            kind='constant' if 'ConstantValue' in field['attributes'] and field.get('constant_value') is not None else 'non_constant' if coverage=='all_declarations' else 'unknown'
            key=(declaration['descriptor'],field['name'],field['proto'])
            fields[key]={'owner':units(key[0]),'name':units(key[1]),'field_type':units(key[2]),'access_flags':field['flags'],'constant_kind':kind}
    matched_variant=False
    flag_bits={'public':1,'private':2,'protected':4,'static':8,'final':16,'volatile':64,'transient':128,'synthetic':4096,'enum':16384}
    for declaration in explicit_entries:
        for field in declaration.get('fields',[]):
            if field_kind is None or field.get('javaConstantKind')!=field_kind:continue
            key=(declaration['descriptor'],field['name'],field['type'])
            flags=0
            for flag in field['accessFlags']:
                if flag not in flag_bits:raise PreparationError('unknown explicit field access flag')
                flags|=flag_bits[flag]
            kind={'Constant':'constant','NonConstant':'non_constant','Unknown':'unknown'}[field_kind]
            fields[key]={'owner':units(key[0]),'name':units(key[1]),'field_type':units(key[2]),'access_flags':flags,'constant_kind':kind}
            matched_variant=True
    if field_kind is not None and not matched_variant:raise PreparationError('requested field evidence variant is not certified')
    return {'environment_id':0,'methods':methods,'classes':classes,'fields':list(fields.values())}


def _units(value):
    return b''.join(int(unit).to_bytes(2,'little') for unit in value).decode('utf-16-le')


def _failure_outcome(diagnostics):
    if any(d['kind'] in {'malformed','cancelled','internal_invariant'} for d in diagnostics):return 'error'
    return 'resource-failure' if any(d['kind']=='budget_exceeded' and d.get('limit') in {'max_method_output_bytes','fallback_output_bytes','max_report_output_bytes'} for d in diagnostics) else 'error'


def _artifacts(actual, cert,requested=()):
    binding=cert['bindings'][0];selector=binding['selector'];origin=binding['origin']
    if actual['schema']!='diana.inspect/1':raise ValueError('unknown public result schema')
    if actual['status']=='request_error':
        return {'outcome':{'outcome':_failure_outcome(actual['diagnostics']),'status':'request_error','selected':cert['bindings'],'members':[],
                           'diagnostics':actual['diagnostics'],'report':actual['report'],'published_success':actual['published_success']},
                'text':b'','rendered':{'spans':[],'raw_excerpt_ranges':[]}}
    if selector['kind']!='request' and (actual['header_offset']!=origin['header_offset'] or actual['class_def_idx']!=selector['class_def_idx']):
        raise ValueError('public result has a different logical/class identity')
    owner=_units(actual['descriptor_units'])
    if selector['kind']!='request' and owner!=selector['descriptor']:raise ValueError('public result has a different owner')
    members=[];identities={}
    for member in actual['members']:
        source=member['source'];language=member['language']
        if not isinstance(source,str):raise ValueError('public member has no source slice')
        if member['name_units'] is not None:
            name,proto=_units(member['name_units']),_units(member['proto_units'])
        elif language=='diagnostic':
            name,proto=None,None
        else:
            decoded=json.loads(source)
            if language=='original_excerpt':name,proto=decoded['name'],decoded['proto']
            elif language=='dex_debug':
                declaration=decoded['classes'][0]['methods'][0];name,proto=declaration['name'],declaration['proto']
            else:raise ValueError('unavailable member has no original declaration identity')
        mid=member['method_id_idx']
        if mid in identities:raise ValueError('duplicate original method')
        identities[mid]=(name,proto)
        members.append({'owner':owner,'name':name,'proto':proto,'method_id_idx':mid,
                        'language':language,'source':source,'status':member['status']})
    if selector['kind'] in {'method','request'}:
        if selector['kind']=='method' and (set(identities)!={selector['method_id_idx']} or identities[selector['method_id_idx']] not in {(None,None),(selector['name'],selector['proto'])}):
            raise ValueError('public result differs from the certified method selection')
        if len(actual['members'])!=1:raise ValueError('method request did not return exactly one original key')
        selected=actual['members'][0]
        status=selected['status'];outcome={'java':'java','declaration_only':'declaration-only','debug':'debug','unavailable':'unavailable'}[status]
        reason=selected['reason']
        diagnostics=[] if reason is None else [actual['diagnostics'][reason]]
    else:
        status=actual['status'];outcome=status;diagnostics=actual['diagnostics']
    scoped=None
    if requested:
        scoped=[member for method in requested for member in members if member['name']+member['proto']==method]
        if len(scoped)!=len(requested):raise ValueError('method group differs from requested original identities')
        chosen={member['method_id_idx'] for member in scoped}
        diagnostics=[diagnostic for diagnostic in actual['diagnostics'] if diagnostic['method_id_idx'] in chosen or diagnostic['kind'] in {'malformed','cancelled','internal_invariant'}]
        if all(member['status']=='java' and member['language']=='java' for member in scoped):status,outcome='java','java'
    text=actual['text'].encode('utf-8')
    spans=[];excerpts=[]
    for span in actual['spans']:
        name,proto=identities[span['method_id_idx']]
        if name is None and selector['kind']=='method' and span['method_id_idx']==selector['method_id_idx']:
            name,proto=selector['name'],selector['proto']
        value={**span,'owner':owner,'method':None if name is None else name+proto,
               'failure':next(m for m in actual['members'] if m['method_id_idx']==span['method_id_idx'])['reason'] is not None}
        spans.append(value)
        if span['language']=='original_excerpt':excerpts.append(value)
        submitted=next(m for m in members if m['method_id_idx']==span['method_id_idx'])['source'].encode('utf-8')
        if text[span['start']:span['end']]!=submitted:raise ValueError('public member source differs from its rendered slice')
    if any(d['kind'] in {'malformed','cancelled','internal_invariant'} for d in diagnostics):
        outcome,status='error','request_error'
    if outcome=='unavailable' and _failure_outcome(diagnostics)=='resource-failure':outcome='resource-failure'
    public={'outcome':outcome,'status':status,'selected':cert['bindings'],'members':members,'diagnostics':diagnostics,
            'report':{'available':True},'published_success':False}
    groups={'outcome':public,'text':text}
    if outcome!='error':groups['rendered']={'spans':spans,'raw_excerpt_ranges':excerpts}
    if requested:public['context_report']={'status':actual['status'],'diagnostics':actual['diagnostics'],'members':members};public['requested_methods']=requested;public['members']=scoped
    if outcome=='java':groups['method_sources']=[{'name':member['name'],'proto':member['proto'],'source':member['source']} for member in (scoped if scoped is not None else members)]
    if outcome=='declaration-only':
        name=owner[1:-1].split('/')[-1];package=owner[1:-1].rsplit('/',1)[0].replace('/','.') if '/' in owner else ''
        if any(not part.isidentifier() for part in (package.split('.') if package else [])+[name]):raise ValueError('unsupported known Java context name')
        groups['declaration_source']={'name':name,'package':package,'source':actual['members'][0]['source'],'abstract':bool(actual['access_flags']&0x400)}
    return groups


def run_unimplemented(spec):
    return [{'schema_version':1,'spec_revision':spec['specRevision'],'case_id':c['id'],'invocation_id':i['id'],
             'seam':seam,'producer':'diana-public-library','execution':'not_implemented',
             'reason':'T05 public-library integration is not implemented; no engine result exists',
             'artifacts':{}} for c,i,seam in required_runs(spec)]
