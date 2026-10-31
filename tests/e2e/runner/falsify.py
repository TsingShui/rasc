"""Operator-authorized B02/B04 bounded fault falsification, not engine acceptance."""
from __future__ import annotations
import copy
import json
from pathlib import Path
import sys

from .adapter import required_runs, run_unimplemented
from .compare import Mismatch, exact
from .evaluate import compare_suite, debug_view, original_debug_facts
from .filesystem import assertions as file_assertions

REFERENCE_CASES=['D009','D018','D016','D052','D061','D069']


def verify_harness(root,cache,tools):
    from e2e import PreparationError, Tools, load, relative, run, save, sha
    spec=load(root/'docs/diana/e2e_cases.json');manifest=load(root/'tests/e2e/manifest.json')
    if manifest['spec']['revision']!=spec['specRevision']:
        raise PreparationError('stale suite revision')
    # Admit every original/dependency/certificate by path and size.
    for asset in manifest['assets']:
        path=root/asset['path']
        if not path.is_file() or path.stat().st_size!=asset['size']:
            raise PreparationError('missing/changed manifest artifact: '+asset['path'])
    directory=cache/'harness'
    directory.mkdir(parents=True,exist_ok=True)
    def artifact(name,value):
        path=directory/(name+'.json');save(path,value)
        return {'path':relative(path),'sha256':sha(path)}
    cases={c['id']:c for c in spec['cases']}
    locked={(c['id'],i['id']):i for c in manifest['cases'] for i in c['invocations']}
    assets={a['id']:a for a in manifest['assets']}
    records=[]
    # The positive control is an original-fact snapshot, not a mock Engine and
    # not output reconstructed through an assembler.
    binding=locked[('D061','v1')];original=assets[binding['input_assets'][0]]
    decoded=load(root/str(Path(original['path']).with_suffix('.dexlib.json')))
    physical=load(root/(original['path']+'.layout.json'))['members'][0]
    selected=[b['selector'] for b in binding['bindings'] if b['selector']['kind']=='method']
    debug_control=debug_view(original_debug_facts(decoded,physical),selected)
    for case,invocation,seam in required_runs(spec,REFERENCE_CASES):
        cid,iid=case['id'],invocation['id'];binding=locked[(cid,iid)]
        groups={};public={'outcome':'java','status':'java','diagnostics':[]}
        if cid=='D061':
            public={'outcome':'debug','status':'debug','diagnostics':[{'kind':'unsupported','code':'BindingEvidenceMissing'}]}
            groups['debug']=artifact(cid+'-'+iid+'-debug',{'actual':debug_control})
        elif cid=='D069':
            original=assets[binding['input_assets'][0]];zip_doc=load(root/(original['path']+'.layout.json'));entry=zip_doc['entries'][0]
            kind='crc_mismatch' if not entry['crc_valid'] else 'encrypted_entry' if entry['flags']&1 else 'unsupported_compression' if entry['method'] not in [0,8] else 'budget_exceeded'
            public={'outcome':'error','status':'request_error','diagnostics':[{'kind':kind}]}
            frame=directory/(cid+'-'+iid);frame.mkdir(exist_ok=True)
            target=frame/'target.bin';target.write_bytes(b'KEEP');temporary=frame/'temps';temporary.mkdir(exist_ok=True)
            groups['filesystem']=artifact(cid+'-'+iid+'-filesystem',{'target_path':relative(target),'temporary_directory':relative(temporary)})
        else:
            driver=binding['driver'];need_driver=driver is not None
            if not need_driver:raise PreparationError('missing real positive-control Driver')
            ref=assets[driver['reference_assets'][0]]
            groups['driver']={'path':ref['path']}
        groups['outcome']=artifact(cid+'-'+iid+'-public',public)
        records.append({'schema_version':1,'spec_revision':spec['specRevision'],'case_id':cid,'invocation_id':iid,'seam':seam,
                        'producer':'reference-falsification','execution':'completed','artifacts':groups})
    positive=compare_suite(root,spec,manifest,records,REFERENCE_CASES,reference=True)
    failures=[]
    def reject(name, operation, expected=(Mismatch,)):
        try:operation()
        except expected as exc:
            failures.append({'fault':name,'result':'rejected','reason':str(exc)})
        else:raise PreparationError('harness accepted injected fault: '+name)
    def changed(cid,group,mutate,name,iid=None):
        candidate=copy.deepcopy(records)
        record=next(r for r in candidate if r['case_id']==cid and (iid is None or r['invocation_id']==iid))
        value=load(root/record['artifacts'][group]['path']);mutate(value)
        record['artifacts'][group]=artifact('fault-'+name,value)
        return candidate
    def compare(value):return compare_suite(root,spec,manifest,value,REFERENCE_CASES,reference=True)
    reject('wrong-return',lambda:compare(changed('D009','driver',lambda v:v['result'].__setitem__('value',v['result']['value']^1),'return')))
    reject('missing-event',lambda:compare(changed('D018','driver',lambda v:v['events'].pop(),'event')))
    def catch(value):
        m=next(m for c in value['actual']['classes'] for m in c['methods'] if m['name']=='f')
        handlers=m['code']['tries'][0]['handlers'];handlers[0],handlers[1]=handlers[1],handlers[0]
    reject('swapped-catch',lambda:compare(changed('D061','debug',catch,'catch')))
    reject('changed-float-zero',lambda:compare(changed('D016','driver',lambda v:v['result'].__setitem__('bits','00000000'),'zero','v1')))
    def surrogate(value):
        units=value['result']['units'];units[units.index(55296)]=65533
    reject('lossy-surrogate',lambda:compare(changed('D052','driver',surrogate,'surrogate')))
    def truncate(value):
        next(m for c in value['actual']['classes'] for m in c['methods'] if m['name']=='f')['code']['instructions'].pop()
    reject('truncated-debug',lambda:compare(changed('D061','debug',truncate,'debug')))
    for iid in ['bad-crc','inflate-budget']:
        reject('wrong-error-kind-'+iid,lambda iid=iid:compare(changed('D069','outcome',lambda v:v['diagnostics'][0].__setitem__('kind','target_exists'),'error-'+iid,iid)))
    reject('missing-invocation',lambda:compare(records[:-1]))
    missing=copy.deepcopy(records);missing[0]['artifacts']['driver']['path']=relative(directory/'does-not-exist.json')
    reject('missing-artifact',lambda:compare(missing))
    timeout=copy.deepcopy(records);timeout[0]['execution']='timeout'
    reject('timeout-result',lambda:compare(timeout))
    reject('zero-cases',lambda:compare_suite(root,spec,manifest,[],[],reference=True),(ValueError,))
    reject('subprocess-timeout',lambda:run([sys.executable,'-c','while True: pass'],root,directory/'timeout.log',timeout=0.05),(PreparationError,))
    # D070's empty-success counterexample is an actual empty file, not a missing
    # flag. This checks only the verifier; no application publisher is simulated.
    frame=directory/'empty-output';frame.mkdir(exist_ok=True);target=frame/'out.bin';target.write_bytes(b'');temp=frame/'temps';temp.mkdir(exist_ok=True)
    observed=file_assertions(root,{'target_path':relative(target),'temporary_directory':relative(temp)})
    wanted=next(i for i in cases['D070']['invocations'] if i['id']=='traversal-name-safe-target')['expected']['fileBytes']
    reject('D070-empty-output',lambda:exact(wanted,observed.get('fileBytes')))
    instance=Tools(cache);instance.lock=copy.deepcopy(instance.lock)
    instance.lock['artifacts'][0]['size']+=1
    reject('tool-size',instance.verify,(PreparationError,))
    instance=Tools(cache);instance.downloads=directory/'missing-tools'
    reject('missing-tool',instance.verify,(PreparationError,))
    absent=run_unimplemented(spec)
    if any(r['execution']!='not_implemented' for r in absent):raise PreparationError('fake engine result')
    reject('unimplemented-engine',lambda:compare_suite(root,spec,manifest,absent))
    report={'schema_version':1,'positive_reference':positive,'faults':failures,
            'authorization':'operator request to complete B01-B04; listed preparer/comparator faults only',
            'engine':{'execution':'not_implemented','case_denominator':len({r['case_id'] for r in absent}),
                      'invocation_denominator':len({(r['case_id'],r['invocation_id']) for r in absent}),
                      'seam_denominator':len(absent),'passed':0},'art':'not-run'}
    save(directory/'verification-report.json',report)
    save(cache/'engine-not-implemented.json',absent)
    print('harness reference accepted;',len(failures),'injected faults rejected; engine 0/'+str(report['engine']['case_denominator'])+' (not_implemented)')
