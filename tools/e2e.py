#!/usr/bin/env python3
"""Offline Diana fixture preparation. Only fetch-tools is allowed to use a network.

Expected behavior is read exclusively from docs/diana/e2e_cases.json. This tool
never executes or builds a reference decompiler and never refreshes expectations.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / 'tests/e2e'
SPEC = ROOT / 'docs/diana/e2e_cases.json'
sys.path.insert(0, str(SUITE))
if __name__ == '__main__':
    sys.modules['e2e'] = sys.modules[__name__]
JAVA_FLAGS = ['--release', '8', '-proc:none', '-g:none']


class PreparationError(Exception):
    pass


def load(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + '\n', encoding='utf-8')


def sha(path):
    """Informational provenance only; never use this value for admission or acceptance."""
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def relative(path):
    try:
        return Path(path).resolve().relative_to(ROOT).as_posix()
    except ValueError as exc:
        raise PreparationError('artifacts must be inside the checkout') from exc


def run(argv, cwd, log, timeout=90, output_cap=8 * 1024 * 1024):
    """Bound process lifetime and aggregate output, including a noisy child.

    A reader thread drains both streams through one pipe; no communicate() buffer
    can grow without bound. Process groups terminate javac/Java descendants too.
    """
    import signal
    import threading
    argv = [str(x) for x in argv]
    log.parent.mkdir(parents=True, exist_ok=True)
    save(log.with_suffix('.command.json'), {'argv':[a.replace(str(ROOT) + os.sep, '') if a != sys.executable else '<python3>' for a in argv], 'cwd':relative(cwd), 'timeout_seconds':timeout, 'output_cap_bytes':output_cap})
    output = bytearray()
    overflow = threading.Event()
    with subprocess.Popen(argv, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          start_new_session=(os.name != 'nt')) as process:
        def drain():
            while block := process.stdout.read(65536):
                remaining = output_cap - len(output)
                output.extend(block[:max(remaining, 0)])
                if len(block) > remaining:
                    overflow.set()
                    break
        reader = threading.Thread(target=drain, daemon=True)
        reader.start()
        deadline = time.monotonic() + timeout
        failure = None
        while process.poll() is None:
            if overflow.is_set() or time.monotonic() >= deadline:
                failure = 'output_limit' if overflow.is_set() else 'timeout'
                if os.name == 'nt':
                    process.kill()
                else:
                    os.killpg(process.pid, signal.SIGKILL)
                break
            time.sleep(0.01)
        process.wait(timeout=10)
        reader.join(timeout=10)
        if reader.is_alive():
            raise PreparationError('child output stream did not terminate')
        if overflow.is_set():
            failure = 'output_limit'
        log.write_bytes(output)
        if failure or process.returncode:
            raise PreparationError(f'{log.name}: {failure or "exit " + str(process.returncode)}; see {relative(log)}')
    return output.decode('utf-8', errors='strict')


class Tools:
    def __init__(self, cache):
        self.cache = cache
        self.lock = load(SUITE / 'toolchain.lock.json')
        machine = {'arm64':'aarch64', 'AMD64':'x86_64'}.get(platform.machine(), platform.machine())
        self.platform = platform.system().lower() + '-' + machine
        if self.platform not in self.lock['jdk']:
            raise PreparationError(f'unsupported/unexecuted platform: {self.platform}')
        self.jdk = self.lock['jdk'][self.platform]
        self.downloads = cache / 'tools/downloads'
        self.home = cache / 'tools/jdk' / self.jdk['home']
        suffix = '.exe' if os.name == 'nt' else ''
        self.java = self.home / ('bin/java' + suffix)
        self.javac = self.home / ('bin/javac' + suffix)
        self.javap = self.home / ('bin/javap' + suffix)
        self.jars = {a['id']: self.downloads / Path(a['path']).name for a in self.lock['artifacts']}
        self.cp = os.pathsep.join(str(p) for p in self.jars.values())

    def entries(self):
        for record in [self.jdk, *self.lock['artifacts']]:
            yield record, self.downloads / Path(record['path']).name
            if 'pom' in record:
                yield record['pom'], self.downloads / Path(record['pom']['path']).name

        for record in self.lock.get('pom_sources', []):
            yield record, self.downloads / Path(record['path']).name

    def verify(self):
        """Check locked paths, sizes and dependencies, without content fingerprints."""
        for record, path in self.entries():
            if not path.is_file() or path.stat().st_size != record['size']:
                raise PreparationError(f'missing/changed locked tool: {path.name}; run fetch-tools explicitly')
        ids = set(self.jars)
        for record in self.lock['artifacts']:
            if not set(record['dependencies']) <= ids:
                raise PreparationError(f'unlocked transitive dependency: {record["id"]}')
        if not self.java.is_file() or not self.javac.is_file():
            raise PreparationError('pinned JDK is not extracted; run fetch-tools')

    def fetch(self):
        self.downloads.mkdir(parents=True, exist_ok=True)
        def acquire(item):
            record, target = item
            if target.is_file() and target.stat().st_size == record['size']:
                return
            fd, name = tempfile.mkstemp(dir=self.downloads, suffix='.partial')
            try:
                with os.fdopen(fd, 'wb') as out, urlopen(Request(record['url'], headers={'User-Agent':'Diana-fixtures'}), timeout=120) as response:
                    total = 0
                    while block := response.read(65536):
                        total += len(block)
                        if total > record['size']:
                            raise PreparationError('download exceeds locked size: ' + target.name)
                        out.write(block)
                temp = Path(name)
                if temp.stat().st_size != record['size']:
                    raise PreparationError('download size mismatch: ' + target.name)
                temp.replace(target)
            finally:
                Path(name).unlink(missing_ok=True)
        with ThreadPoolExecutor(6) as pool:
            list(pool.map(acquire, self.entries()))
        destination = self.cache / 'tools/jdk'
        # JDK archive paths and links are checked before any extraction.
        if not self.java.exists():
            destination.mkdir(parents=True, exist_ok=True)
            with tarfile.open(self.downloads / Path(self.jdk['path']).name) as archive:
                for member in archive.getmembers():
                    path = destination / member.name
                    if not path.resolve().is_relative_to(destination.resolve()):
                        raise PreparationError('escaping JDK archive member')
                    if member.isdev() or member.isfifo() or member.islnk():
                        raise PreparationError('unsupported JDK archive member type')
                    if member.issym() and not (path.parent / member.linkname).resolve().is_relative_to(destination.resolve()):
                        raise PreparationError('escaping JDK archive link')
                archive.extractall(destination, filter='data')
        self.verify()
        print('locked tools ready:', self.platform)


def compile_java(tools, sources, output, logs, name, classpath=(), java_flags=None):
    output.mkdir(parents=True, exist_ok=True)
    argv = [tools.javac, *(JAVA_FLAGS if java_flags is None else java_flags), '-d', output]
    if classpath:
        argv += ['-cp', os.pathsep.join(str(p) for p in classpath)]
    argv += sources
    run(argv, ROOT, logs / (name + '.log'))
    classes = sorted(output.rglob('*.class'))
    if not classes:
        raise PreparationError(name + ': javac produced no classes')
    return classes


def java_group(tools, sources, drivers, driver_class, work, logs, name, invocations, dex=False, driver_args=None, java_flags=None):
    classes = work / name / 'classes'
    classfiles = compile_java(tools, sources, classes, logs, name + '-javac', java_flags=java_flags)
    if dex:
        dexdir = work / 'dex'
        dexdir.mkdir(parents=True, exist_ok=True)
        run([tools.java, '-cp', tools.cp, 'com.android.tools.r8.D8', '--debug', '--min-api', '23',
             '--lib', tools.home, '--output', dexdir, *classfiles], ROOT, logs / 'd8.log')
        if not (dexdir / 'classes.dex').is_file():
            raise PreparationError('D8 produced no DEX')
    observations = []
    if drivers:
        driver_out = work / name / 'driver-classes'
        compile_java(tools, drivers, driver_out, logs, name + '-driver-javac', [classes])
        for invocation in invocations:
            args = driver_args(invocation) if driver_args else [invocation['id']]
            path = work / name / (invocation['id'] + '.driver.json')
            raw = run([tools.java, '-cp', os.pathsep.join(map(str, [driver_out, classes])), driver_class, *args],
                      ROOT, logs / (name + '-' + invocation['id'] + '.log'))
            actual = json.loads(raw)
            if actual.get('case') is None or actual.get('invocation') != invocation['id']:
                raise PreparationError('Driver returned wrong identity')
            save(path, actual)
            observations.append({'invocation': invocation['id'], 'path': relative(path), 'kind':'source-jvm', 'classes':relative(classes)})
    return observations


def prepare_case(case, tools, cache):
    cid = case['id']
    source = SUITE / 'sources' / cid
    recipe = load(source / 'recipe.json')
    if recipe['targets'] != case['targets'] or [i['id'] for i in recipe['invocations']] != [i['id'] for i in case['invocations']]:
        raise PreparationError('recipe/spec target or invocation mismatch')
    work = cache / 'prepared' / cid
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    logs = work / 'logs'
    observations = []
    def local(paths):
        return [source / p for p in paths]
    if 'fixture' in recipe:
        sources = local(recipe['fixture']['sources'] + recipe.get('dependencies', {}).get('sources', []))
        driver = recipe.get('driver') or {}
        observations += java_group(tools, sources, local(driver.get('sources', [])), driver.get('class'),
                                   work, logs, 'reference', case['invocations'], dex=True,
                                   java_flags=recipe['javac']['args'])
    else:
        builder = recipe.get('builder') or recipe.get('dex', {}).get('builder')
        if builder:
            path = builder['path'] if isinstance(builder, dict) else builder
            if isinstance(builder, dict):
                output = builder.get('output')
                argument = work / Path(output).name if output else work
            else:
                argument = work
            run([sys.executable, source / path, argument], ROOT, logs / 'builder.log')
        if cid in {'D076','D077','D078'}:
            name = 'gen_reference.py' if cid == 'D076' else 'gen_sources.py'
            run([sys.executable, source / 'fixtures' / name, work], ROOT, logs / 'generate.log')
        # Execute checked-in Smali recipes exactly; no Java-to-Smali substitution.
        for build in recipe.get('builds', []):
            argv = build.get('argv', [])
            if 'org.jf.smali.Main' not in argv:
                continue
            output = work / Path(build['output']).name
            args = argv[argv.index('org.jf.smali.Main') + 1:]
            args[args.index('-o') + 1] = str(output)
            args = [str(source / a) if a.endswith('.smali') else a for a in args]
            run([tools.java, '-cp', tools.cp, 'org.jf.smali.Main', *args], ROOT, logs / (build['id'] + '.log'))
            if not output.is_file():
                raise PreparationError('assembler produced no output: ' + build['id'])
        if cid in {'D027','D046','D073'}:
            observations += java_group(tools, local(['fixtures/Case.java']), local(['fixtures/Driver.java']),
                                       'fixtures.Driver', work, logs, 'mirror', case['invocations'])
            for item in observations:
                item['kind'] = 'specification-mirror-jvm'
        elif cid == 'D065':
            paths = sorted((source / 'fixtures/mirror').rglob('*.java'))
            observations += java_group(tools, paths, [], None, work, logs, 'mirror', [])
            for invocation in case['invocations']:
                raw = run([tools.java, '-cp', work / 'mirror/classes', 'mirror.Driver', invocation['id']], ROOT,
                          logs / (invocation['id'] + '-mirror.log'))
                path = work / 'mirror' / (invocation['id'] + '.driver.json')
                save(path, json.loads(raw))
                observations.append({'invocation':invocation['id'],'path':relative(path),'kind':'specification-mirror-jvm','classes':relative(work/'mirror/classes')})
        elif cid == 'D075':
            for role in recipe['referenceJava']['roles']:
                inv = [i for i in case['invocations'] if i['input']['targetRole'] == role]
                drivers = list((source / 'driver' / role).glob('*.java'))
                observations += java_group(tools, local([f'reference/{role}/fixtures/Case.java']), drivers, 'driver.Driver',
                                           work, logs, role, inv)
        elif cid == 'D076':
            for role in recipe['referenceJava']['roles']:
                inv = [i for i in case['invocations'] if i['input']['targetRole'] == role]
                observations += java_group(tools, [work / f'reference/{role}/fixtures/Case.java'], local(['driver/Driver.java']),
                                           'driver.Driver', work, logs, role, inv,
                                           driver_args=lambda i, role=role: [role, i['id']])
        elif cid in {'D077','D078'}:
            drivers = list((source / 'driver').glob('*.java'))
            sources = [work / 'fixtures/Case.java', *[p for p in (source / 'fixtures').glob('*.java')]]
            observations += java_group(tools, sources, drivers, 'driver.Driver', work, logs, 'reference', case['invocations'], dex=(cid == 'D077'))
        elif cid == 'D086':
            inv = [i for i in case['invocations'] if i['id'] in recipe['driver']['entries']]
            observations += java_group(tools, local(['fixtures/Case.java']), local(['driver/Driver.java']), 'driver.Driver',
                                       work, logs, 'reference', inv)
        elif cid in {'D062','D063','D064'}:
            java_group(tools, local([p for p in recipe['sources'] if p.endswith('.java')]), [], None,
                       work, logs, 'reference', [], dex=True)
        elif cid == 'D080':
            compile_java(tools, local(['fixtures/External.java']), work / 'constant-proof', logs, 'constant-proof')
        if cid == 'D070':
            raw = run([sys.executable, source / 'fixtures/capability_report.py'], ROOT, logs / 'capabilities.log')
            save(work / 'capabilities.json', json.loads(raw))
    if recipe.get('catalogSources'):
        compile_java(tools, local(recipe['catalogSources']), work / 'catalog-proof', logs, 'catalog-proof')
    assets = sorted([*work.rglob('*.dex'), *work.rglob('*.zip')])
    if not assets:
        raise PreparationError(cid + ': no original input generated')
    return {'id':cid, 'execution':'prepared', 'assets':[relative(p) for p in assets], 'observations':observations,
            'recipe_sha256':sha(source / 'recipe.json'), 'source_sha256':{relative(p):sha(p) for p in sorted(source.rglob('*')) if p.is_file()}}


def prepare(tools, cache, jobs):
    tools.verify()
    spec = load(SPEC)
    cases = [c for c in spec['cases'] if c['required']]
    if not cases or not sum(len(c['invocations']) for c in cases):
        raise PreparationError('empty required execution set')
    def one(case):
        try:
            item = prepare_case(case, tools, cache)
        except (PreparationError, OSError, ValueError, KeyError) as exc:
            item = {'id':case['id'], 'execution':'blocked', 'reason':str(exc)}
        print(item['id'], item['execution'], item.get('reason',''), flush=True)
        return item
    with ThreadPoolExecutor(jobs) as pool:
        results = list(pool.map(one, cases))
    axml_source=SUITE/'sources/H03'
    axml_work=cache/'prepared/H03'
    axml_work.mkdir(parents=True,exist_ok=True)
    run([sys.executable,axml_source/'build_axml.py',axml_work],ROOT,axml_work/'build.log')
    supplemental={'source_sha256':{relative(p):sha(p) for p in sorted(axml_source.iterdir()) if p.is_file()},
                  'assets':[relative(axml_work/'AndroidManifest.axml'),relative(axml_work/'manifest.zip')]}
    report = {'schema_version':1, 'spec_revision':spec['specRevision'], 'spec_sha256':sha(SPEC), 'toolchain_sha256':sha(SUITE/'toolchain.lock.json'),
              'platform':tools.platform, 'case_denominator':len(cases),
              'invocation_denominator':sum(len(c['invocations']) for c in cases), 'cases':results,
              'art':'not-run: no operator-authorized device/image', 'engine':'not_implemented','supplemental_inputs':supplemental}
    save(cache / 'preparation-report.json', report)
    if any(r['execution'] != 'prepared' for r in results):
        raise PreparationError('fixture build failures; see preparation-report.json')


def main():
    if sys.version_info < (3, 11):
        raise PreparationError('Python 3.11 or newer is required')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['fetch-tools','prepare','verify-inputs','verify-harness','run'])
    parser.add_argument('--cache', type=Path, default=ROOT / '.cache/e2e')
    parser.add_argument('--jobs', type=int, default=4)
    parser.add_argument('--case', action='append', dest='case_ids', help='run every invocation of the selected fixed case; repeatable')
    parser.add_argument('--representation',choices=('structured_then_dispatch','dispatch_only'),help='native Java representation adapter for core execution')
    args = parser.parse_args()
    cache = args.cache.resolve()
    relative(cache)
    if args.jobs < 1:
        raise PreparationError('jobs must be positive')
    tools = Tools(cache)
    if args.command == 'fetch-tools':
        tools.fetch()
    elif args.command == 'prepare':
        prepare(tools, cache, args.jobs)
    elif args.command == 'run':
        tools.verify()
        from runner.adapter import run_public
        report = run_public(ROOT,cache,load(SPEC),load(SUITE/'manifest.json'),args.case_ids,args.representation)
        return 0 if report['execution']=='passed' else 1
    else:
        tools.verify()
        from runner.certify import verify_inputs
        if args.command == 'verify-inputs':
            verify_inputs(ROOT, cache, tools)
        else:
            from runner.falsify import verify_harness
            verify_harness(ROOT, cache, tools)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (PreparationError, OSError, ValueError) as exc:
        print('preparation failed:', exc, file=sys.stderr)
        sys.exit(1)
