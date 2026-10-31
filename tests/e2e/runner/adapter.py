"""Tests-only public-library adapter boundary. T05 supplies the real binding.

No fabricated Engine, CLI subprocess or private-phase inspection is used when the
library entrypoints are absent. Every selected invocation/seam is still reported.
"""
from __future__ import annotations


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


def run_unimplemented(spec, spec_hash):
    return [{'schema_version':1,'spec_sha256':spec_hash,'case_id':c['id'],'invocation_id':i['id'],
             'seam':seam,'producer':'diana-public-library','execution':'not_implemented',
             'reason':'T05 public-library integration is not implemented; no engine result exists',
             'artifacts':{}} for c,i,seam in required_runs(spec)]
