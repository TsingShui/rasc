# T11 generated original source context

Goal: run D077's existing Java method through its certified generated source
context. The manifest already records prepared/D077/fixtures/Case.java and its
original javac recipe; the thin adapter currently looks only under checked-in
sources and reference/<role>. Context remains original input, not engine output.

The adapter now selects that exact manifest-recorded generated source and its
original javac source dependencies. It checks their manifest presence and sizes;
it does not search arbitrary cache directories, regenerate input or use engine
output as the original context. Only the selected method body is replaced with
the actual public Engine fragment.

The first actual D077 operation reached compilation but lacked Dep.java in the
candidate sources. Resolving dependencies from the same recorded javac arguments
fixed that context omission. The following real core operations passed:

```sh
python3 tools/e2e.py run --case D077 --case D018 --case D019 --case D023
python3 tools/e2e.py run
```

`.cache/e2e/engine/run-5xhhjxav/report.json` passes all four selected cases and
five invocations. The complete final
`.cache/e2e/engine/run-qil4wi_v/report.json` retains 86 mandatory cases, 177
invocations and 181 seams: 41 whole cases and 95 seams passed. D077 passes actual
Java compilation/classfile checks and the independent Driver's 10000-tick
observation. Other failures remain recorded; the full command exited 1.

Diff and sensitive-input checks passed. The engine is unchanged from 108ab77;
its fmt/strict Clippy/locked release/117-nextest evidence is reused from
[t11-effectful-arguments.md](t11-effectful-arguments.md). No auxiliary-tool tests,
new fingerprints, fixed expectations or TASKS.md changes were introduced.
