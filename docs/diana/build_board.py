#!/usr/bin/env python3
"""Refresh the self-contained architecture board from the Diana Markdown plan.

Run from anywhere: python3 docs/diana/build_board.py
Only index.html's embedded plan data is updated; no engine code or network use.
"""
from pathlib import Path
import hashlib
import json
import re

ROOT = Path(__file__).resolve().parent
START = '<script id="plan-data" type="application/json">'
END = '</script><!-- /plan-data -->'


def load(name):
    return (ROOT / name).read_text(encoding='utf-8')


def clean(value):
    return value.replace('`', '').strip()


def phases_in(value):
    result = set()
    for lo, hi in re.findall(r'P(\d)\s*[–—-]\s*P(\d)', value):
        result.update(f'P{i}' for i in range(int(lo), int(hi) + 1))
    result.update(re.findall(r'P[0-9]', value))
    if '全部' in value:
        result.update(f'P{i}' for i in range(10))
    return sorted(result)


def build():
    design = load('DESIGN.md')
    plan = load('IMPLEMENTATION_PLAN.md')
    tasks_text = load('TASKS.md')
    extension_text = load('RECOVERY_EXTENSIONS.md')
    preparation_text = load('TEST_PREPARATION.md')
    prep_deps = {
        key: [] if value == '-' else value.split()
        for key, value in re.findall(r'^prep-deps (B\d+): (.+)$', preparation_text, re.M)
    }
    prep_tasks = []
    prep_headings = list(re.finditer(r'^## (B\d+) — (.+)$', preparation_text, re.M))
    for i, heading in enumerate(prep_headings):
        stop = prep_headings[i + 1].start() if i + 1 < len(prep_headings) else preparation_text.index('## 3. 测试冻结')
        body = preparation_text[heading.end():stop].strip()
        positions = list(re.finditer(r'^\*\*([^*]+)\*\*：', body, re.M))
        sections = []
        for j, match in enumerate(positions):
            section_stop = positions[j + 1].start() if j + 1 < len(positions) else len(body)
            sections.append({'name': match[1], 'text': body[match.end():section_stop].strip()})
        prep_tasks.append({'id': heading[1], 'title': heading[2], 'deps': prep_deps[heading[1]], 'sections': sections})
    case_design = json.loads(load('e2e_cases.json'))
    assert case_design['caseCount'] == len(case_design['cases']) == 70
    cli_text = load('CLI.md')
    commands = []
    for line in cli_text.splitlines():
        if not re.match(r'\| (get-class|entries|extract|strings|xref|manifest|classes|get-method|fields|methods|help) \|', line):
            continue
        cells = [c.strip() for c in line.strip('|').split('|')]
        commands.append({'id': cells[0], 'summary': cells[1], 'columns': cells[2], 'usage': cells[3]})
    assert len(commands) == 11
    host_text = load('PROJECT_TASKS.md')
    host_deps = {
        key: [] if value == '-' else value.split()
        for key, value in re.findall(r'^host-deps (H\d+): (.+)$', host_text, re.M)
    }
    host_tasks = []
    host_headings = list(re.finditer(r'^## (H\d+) — (.+)$', host_text, re.M))
    for i, heading in enumerate(host_headings):
        stop = host_headings[i + 1].start() if i + 1 < len(host_headings) else host_text.index('## 2. 产品验收矩阵')
        body = host_text[heading.end():stop].strip()
        positions = list(re.finditer(r'^\*\*([^*]+)\*\*：', body, re.M))
        sections = []
        for j, match in enumerate(positions):
            section_stop = positions[j + 1].start() if j + 1 < len(positions) else len(body)
            sections.append({'name': match[1], 'text': body[match.end():section_stop].strip()})
        host_tasks.append({'id': heading[1], 'title': heading[2],
                           'deps': host_deps[heading[1]], 'sections': sections})
    deps = {
        key: [] if value == '-' else value.split()
        for key, value in re.findall(r'^deps (T\d+): (.+)$', plan, re.M)
    }
    tasks = []
    headings = list(re.finditer(r'^## (T\d+) — (.+)$', tasks_text, re.M))
    for i, heading in enumerate(headings):
        stop = headings[i + 1].start() if i + 1 < len(headings) else tasks_text.index('## 26.')
        body = tasks_text[heading.end():stop].strip()
        phase = re.search(r'\*\*Phase\*\*：(.*?)。', body)
        sections = []
        positions = list(re.finditer(r'^\*\*([^*]+)\*\*：', body, re.M))
        for j, match in enumerate(positions):
            section_stop = positions[j + 1].start() if j + 1 < len(positions) else len(body)
            sections.append({'name': match[1], 'text': body[match.end():section_stop].strip()})
        tasks.append({
            'id': heading[1], 'title': heading[2], 'deps': deps[heading[1]],
            'phases': phases_in(phase[1] if phase else ''),
            'phaseText': phase[1] if phase else '', 'sections': sections,
            'optional': heading[1] == 'T24', 'first': int(heading[1][1:]) <= 6,
        })
    stage_names = {
        'P0': ('知识与配置', '正交只读面'), 'P1': ('安全输入', '校验字节与原始覆盖'),
        'P2': ('控制流', '解码、边与副作用'), 'P3': ('SSA 定义', '固定身份与 Phi'),
        'P4': ('调用契约', '声明证据与绑定义务'), 'P5': ('类型事实', '有限约束与转换证据'),
        'P6': ('源变量', '独立 locals 与边拷贝'), 'P7': ('区域归属', '保护域、循环与锁'),
        'P8': ('Java 合法化', '结构、声明与最终验证'), 'P9': ('文本输出', '只读打印与原始降级'),
    }
    phases = []
    for line in design.splitlines():
        if not re.match(r'\| P\d `', line):
            continue
        cells = [c.strip() for c in line.strip('|').split('|')]
        stage = re.match(r'(P\d) `([^`]+)`', cells[0])
        pid, folder = stage.groups()
        title, subtitle = stage_names[pid]
        phases.append({
            'id': pid, 'title': title, 'subtitle': subtitle, 'folder': folder,
            'artifact': clean(cells[1]), 'responsibility': cells[2], 'excluded': cells[3],
            'tasks': [t['id'] for t in tasks if pid in t['phases']],
        })
    increments = []
    for line in plan.splitlines():
        if not re.match(r'\| I\d ', line):
            continue
        cells = [c.strip() for c in line.strip('|').split('|')]
        name = re.match(r'(I\d) (.+)', cells[0])
        ids = set()
        for lo, hi in re.findall(r'T(\d+)\s*[–—-]\s*T(\d+)', cells[1]):
            ids.update(f'T{i:02d}' for i in range(int(lo), int(hi) + 1))
        ids.update(re.findall(r'T\d{2}', cells[1]))
        increments.append({'id': name[1], 'title': name[2], 'tasks': sorted(ids),
                           'outcome': cells[2], 'gate': cells[3]})
    extensions = []
    for line in extension_text.splitlines():
        if not re.match(r'\| E\d\d ', line):
            continue
        cells = [c.strip() for c in line.strip('|').split('|')]
        name = re.match(r'(E\d\d) (.+)', cells[0])
        extensions.append({'id': name[1], 'title': name[2], 'change': cells[1],
                           'evidence': cells[2], 'reject': cells[3], 'depsText': cells[4],
                           'deps': sorted(set(re.findall(r'T\d{2}', cells[4])))})
    assert len(phases) == 10 and len(tasks) == 25 and len(increments) == 8 and len(extensions) == 8
    assert len(host_tasks) == 10
    keys = {t['id'] for t in tasks}
    assert set(deps) == keys
    assert set(host_deps) == {t['id'] for t in host_tasks}
    assert len(prep_tasks) == 4
    all_deps = {**deps, **host_deps, **prep_deps}
    all_keys = set(all_deps)
    seen = set()
    active = set()

    def visit(key):
        if key in active:
            raise ValueError(f'Task cycle: {key}')
        if key in seen:
            return
        active.add(key)
        for dep in all_deps[key]:
            assert dep in all_keys
            visit(dep)
        active.remove(key)
        seen.add(key)

    for key in all_deps:
        visit(key)
    grouped = [key for group in increments for key in group['tasks']]
    assert len(grouped) == len(keys) and set(grouped) == keys
    names = ['DESIGN.md', 'TASKS.md', 'IMPLEMENTATION_PLAN.md', 'RECOVERY_EXTENSIONS.md', 'PROJECT.md', 'PROJECT_TASKS.md', 'CLI.md', 'TEST_PREPARATION.md', 'REPRODUCIBILITY.md', 'e2e_cases.json']
    fingerprint = hashlib.sha256(''.join(load(n) for n in names).encode()).hexdigest()[:12]
    return {'schema': 1, 'fingerprint': fingerprint, 'sources': names,
            'phases': phases, 'tasks': tasks, 'increments': increments, 'extensions': extensions,
            'hostTasks': host_tasks, 'prepTasks': prep_tasks, 'e2eDesign': case_design,
            'commands': commands, 'product': {'name': 'Diana', 'rootPackage': 'diana',
             'enginePackage': 'diana-decomp', 'scope': 'CLI-first', 'mcp': 'deferred'}}


if __name__ == '__main__':
    page = ROOT / 'index.html'
    original = page.read_text(encoding='utf-8')
    assert original.count(START) == original.count(END) == 1
    data = json.dumps(build(), ensure_ascii=False, separators=(',', ':'))
    data = data.replace('<', '\\u003c').replace('\u2028', '\\u2028').replace('\u2029', '\\u2029')
    a = original.index(START) + len(START)
    b = original.index(END, a)
    page.write_text(original[:a] + '\n' + data + '\n' + original[b:], encoding='utf-8')
    print('Diana board refreshed: 70 designed E2E cases / 4 prerequisite preparation tasks / 10 product + 25 engine tasks')
