# T15 materialized array access

Goal: recover D024's index-call prefix and D026's runtime reference-array check.
Keep original array/index/value definitions materialized and ordered. Typed unused
reads and length operations remain statements; native verification checks original
operands and definitions before Java output. Existing fixed JVM Drivers supply
bounds/null/component-type acceptance without new tooling tests.

The straight-line initialized parameter profile distinguishes int[], Object[],
Object, String and int values. P3 preserves exact aget/aput component opcodes and
array/index/value definitions, and emits every original load, store and length
read, including unused reads. Zero constants have a per-use null view for reference
stores rather than changing their global int kind. Zero-argument int/void calls
retain complete P4 binding, original move-result adjacency and effect order;
unhandled checked throws or type shapes remain unsupported.

P8 independently replays original words, parameter descriptors, aliases, call
identities/results, value IDs, null views, element kinds and all array operands.
It constructs the owned typed statement stream only after that correspondence and
source-scope checks. P9 materializes calls before array checks; stores do not inline
an effectful RHS across a null/bounds failure. Reference stores use the actual
Object[] operand and preserve its runtime ArrayStoreException check, without a new
cast or helper. Public calls() evidence also retains these original static calls.
Known wrong element opcode or index type is Malformed/InvalidArrayAccessType.

```sh
python3 tools/e2e.py run --case D024 --case D026
python3 tools/e2e.py run --case D024 --case D026 --case D027 --case D073 --case D074
python3 tools/e2e.py run
```

On macOS arm64, `run-jeazaclx` passes D024/D026's five fixed invocations. Final
related `run-95sdemwa` passes all five selected cases / 13 invocations, including
the existing fresh/zero/unproven fill behavior. Real javac/Drivers check index
effects before failure and dynamic String[] component checks.

Supplemental authorized sources in `.cache/e2e/t15-array-access/` test an index
call followed by a value call, int store/read, unused primitive/reference loads,
unused length and a contextual null reference store. All actual methods compile
and 14 input/result/event/mutation/exception observations match independent
expectations plus the reference JVM. Both I and V occur before store null/bounds
failure; failed accesses leave the original array unchanged. Unused reads/length
still throw, and a null store is legal on String[]. Three separate valid-format
Smali counterexamples retain Malformed for wrong primitive/reference opcodes and
an Object index, without needing a catalog. Separate original/actual files,
locked compiler logs and observations.json retain this supplemental core evidence;
it does not add fixed passes or test auxiliary tools.

The complete final `run-h1k20ghz` retains 86 mandatory cases / 177 invocations /
181 seams: 49 whole cases / 114 seams pass. Only D024's two and D026's three seams
are newly passing relative to run-4wuua3k5; no pass is lost. The full command exits
1 for remaining genuine obligations. Fmt, strict all-target/all-feature Clippy,
locked release, all 118 cargo-nextest tests with zero skips, diff and sensitive
checks pass. Existing core tests were reused. TASKS.md, inputs and independent
expectations are unchanged.

T15 remains pending. Next action: bind the original instance field for D028 and
preserve field read/write/volatile and class-initialization ordering.
