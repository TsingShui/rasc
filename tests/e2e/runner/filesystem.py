"""Compare actual file/handle captures, never publisher success booleans."""
from __future__ import annotations
from pathlib import Path
from .compare import Mismatch


def assertions(root, record):
    def path(name):
        p=root/name
        if not p.resolve().is_relative_to(root.resolve()):raise Mismatch('escaping observation path')
        return p
    def content(name):
        p=path(name)
        if not p.is_file():raise Mismatch('missing file observation artifact')
        return p.read_bytes()
    target=path(record['target_path'])
    target_bytes=target.read_bytes() if target.is_file() else None
    temporary=path(record['temporary_directory'])
    if not temporary.is_dir():raise Mismatch('missing observed temporary directory')
    temp_count=sum(1 for p in temporary.iterdir())
    out={'targetAbsent':not target.exists(),'tempFilesRemaining':temp_count}
    if target_bytes is not None and len(target_bytes)<=4096:
        out['targetBytes']=list(target_bytes);out['fileBytes']=list(target_bytes)
    if 'originals' in record:
        unchanged=bool(record['originals'])
        for original in record['originals']:
            before=content(original['before_bytes'])
            held=content(original['held_final_bytes'])
            current=path(original['path'])
            identity=tuple(original['identity_before'])
            info=current.stat()
            unchanged &= current.is_file() and current.read_bytes()==before and held==before and (info.st_dev,info.st_ino)==identity and tuple(original['held_identity'])==identity
        out['originalFilesUnchanged']=bool(unchanged)
    if 'safe_output_path' in record:
        safe=path(record['safe_output_path'])
        created=[path(p) for p in record['created_paths']]
        out['outputOnlyExplicitPath']=safe==target and created==[safe] and target_bytes is not None
        out['escapePathAbsent']=not path(record['escape_path']).exists()
    if 'publishers' in record:
        successful=[p for p in record['publishers'] if p['outcome']=='success']
        failed=[p for p in record['publishers'] if p['outcome']!='success']
        out['exactlyOnePublish']=len(successful)==1 and len(failed)==1
        out['otherErrorKind']=failed[0]['error_kind'] if len(failed)==1 else None
        winning=content(successful[0]['payload_path']) if len(successful)==1 else None
        out['targetEqualsSuccessfulPublisher']=winning is not None and target_bytes==winning
    else:
        winning=None
    selected=content(record['selected_payload_path']) if 'selected_payload_path' in record else None
    if selected is not None:out['targetEqualsSelectedPayload']=target_bytes==selected
    if 'observations' in record:
        observed=[];complete=False
        for observation in record['observations']:
            if observation is None:
                observed.append('absent');continue
            data=content(observation['bytes_path'])
            if winning is not None and data==winning:label='complete-winning-payload';complete=True
            elif selected is not None and data==selected:label='complete-new-payload';complete=True
            elif selected is not None and data==b'KEEP':label='old-KEEP'
            else:label='invalid-empty-prefix-or-mixed'
            observed.append(label)
        out['observationsOnly']=observed
        out['completePayloadObserved']=complete and target_bytes in [p for p in [winning,selected] if p is not None]
    return out
