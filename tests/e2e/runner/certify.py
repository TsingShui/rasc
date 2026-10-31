"""B03 independent original-input projection and source-reference certification."""
from __future__ import annotations
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path

from . import layout
from .compare import driver_assertions


def verify_inputs(root, cache, tools):
    from e2e import JAVA_FLAGS, PreparationError, load, relative, run, save, sha
    spec_path = root / 'docs/diana/e2e_cases.json'
    spec = load(spec_path)
    report = load(cache / 'preparation-report.json')
    if report['spec_sha256'] != sha(spec_path) or report['toolchain_sha256'] != sha(root / 'tests/e2e/toolchain.lock.json'):
        raise PreparationError('stale preparation specification/toolchain')
    by_id = {c['id']:c for c in spec['cases'] if c['required']}
    if len(report['cases']) != len(by_id) or {c['id'] for c in report['cases']} != set(by_id):
        raise PreparationError('missing/duplicate original case')
    projector = cache / 'projector/classes'
    projector.mkdir(parents=True, exist_ok=True)
    run([tools.javac, *JAVA_FLAGS, '-cp', tools.cp, '-d', projector, root / 'tests/e2e/runner/ProjectDex.java'],
        root, cache / 'projector/compile.log')
    def one(built):
        cid = built['id']
        try:
            if built['execution'] != 'prepared':
                raise PreparationError(built.get('reason','input not prepared'))
            for path, digest in built['source_sha256'].items():
                if sha(root / path) != digest:
                    raise PreparationError('changed source after preparation: ' + path)
            case = by_id[cid]
            invocations = {i['id']:i for i in case['invocations']}
            source_evidence = []
            for item in built['observations']:
                observation = load(root / item['path'])
                if observation['case'] != cid:
                    raise PreparationError('wrong Driver case identity')
                fields = driver_assertions(spec, invocations[item['invocation']]['expected'], observation)
                source_evidence.append({**item, 'sha256':sha(root / item['path']), 'asserted_fields':fields})
            assets = []
            for path in built['assets']:
                asset = root / path
                record = {'path':path, 'sha256':sha(asset), 'size':asset.stat().st_size}
                out = asset.with_suffix(asset.suffix + '.layout.json')
                if asset.suffix == '.dex':
                    try:
                        projection = layout.project(asset)
                        save(out, projection)
                        record['layout'] = relative(out)
                    except layout.FormatError as exc:
                        intended = {('D055','insns-count.dex'):'code_insns_out_of_range',
                                    ('D055','handler-count.dex'):'typed_handler_count_out_of_range'}.get((cid,asset.name))
                        if intended != exc.kind:
                            raise
                        record['invalid'] = {'kind':exc.kind, 'offset':exc.offset}
                    if 'invalid' not in record and projection['members'][0]['version'] != '041':
                        dest = asset.with_suffix('.dexlib.json')
                        log = asset.with_suffix('.dexlib.log')
                        try:
                            text = run([tools.java, '-cp', os.pathsep.join([str(projector), tools.cp]), 'ProjectDex', asset],
                                       root, log, timeout=90, output_cap=64 * 1024 * 1024)
                            decoded = json.loads(text)
                            save(dest, decoded)
                            record['dexlib2'] = relative(dest)
                            # The independent decoders must agree on original declaration identity.
                            raw = projection['members'][0]
                            if [[c['descriptor'],[(m['method_id_idx'],m['name'],m['proto'],m['flags']) for m in c['methods']]] for c in raw['classes']] != [[c['descriptor'],[(m['method_id_idx'],m['name'],m['proto'],m['flags']) for m in c['methods']]] for c in decoded['classes']]:
                                raise PreparationError('independent declaration projections disagree')
                            if [s['units'] for s in raw['strings']] != decoded['strings']:
                                raise PreparationError('independent UTF-16 projections disagree')
                        except PreparationError:
                            if (cid,asset.name) != ('D059','mutated.dex') or 'unknown opcode at 0' not in log.read_text():
                                raise
                            data = asset.read_bytes()
                            method = next(m for c in projection['members'][0]['classes'] for m in c['methods'] if m['name'] == 'f')
                            if method['code']['units'] != [0x3e] or method['proto'] != '()V':
                                raise PreparationError('unknown-width input lost its exact opcode/range')
                            record['unknown_width'] = {'opcode':0x3e,'pc':0,'offset':method['code']['insns_offset']}
                elif asset.suffix == '.zip':
                    from .zip_projection import project_zip
                    save(out, project_zip(asset))
                    record['zip'] = relative(out)
                assets.append(record)
            evidence = {'id':cid,'state':'projected','assets':assets,'references':source_evidence,
                        'validation_scope':case['validationScope'],'requirements':'not-yet-certified',
                        'art':'not-run'}
            save(cache / 'prepared' / cid / 'certificate.json', evidence)
            print(cid, 'projected', flush=True)
            return evidence
        except (ValueError, OSError, PreparationError, KeyError, IndexError) as exc:
            print(cid, 'blocked:', str(exc)[:300], flush=True)
            return {'id':cid,'state':'blocked','reason':str(exc)}
    with ThreadPoolExecutor(6) as pool:
        results = list(pool.map(one, report['cases']))
    save(cache / 'input-projections.json', {'spec_sha256':sha(spec_path),'cases':results,
                                          'case_denominator':len(by_id), 'invocation_denominator':sum(len(c['invocations']) for c in by_id.values())})
    if any(r['state'] != 'projected' for r in results):
        raise PreparationError('independent projection failures; see input-projections.json')
    from .axml_projection import certify as certify_axml
    certify_axml(root,cache,report)
    # Projection success alone is not inputRequirements acceptance.
    from .requirements import certify_requirements
    certify_requirements(root, cache, tools, spec, results)
