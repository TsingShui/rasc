# T14 integer switch acceptance

Goal: verify the existing sealed integer-switch producer against TASKS T14 and
ALGORITHMS §6 rather than rebuild the accepted T09 path. Fixed D007/D083 and
public malformed-payload coverage exist. The remaining behavioral observation is
a materialized effectful selector across shared/fallthrough/default cases and
switch versus loop transfers, with independent results/events.

Review found a strategy gap despite passing values: the printer emitted Java
switch for every key span. P7 now selects Cases or Comparisons with the checked
i64 span/density rule from ALGORITHMS §6.2. The choice is sealed into the storage
transfer and checked by P8. Sparse printing uses the materialized selector, exact
keys, original edge copies and the separate default; it allocates no span-sized
table. No keys are omitted and no new input quota is introduced.

The existing source correspondence remains authoritative: P8 compares selector,
every key, each EdgeId/target/copy set and the explicit default against original
SSA. Shared source targets have one dispatcher block; selecting keys preserve
their distinct incoming copies. Original case jumps preserve fallthrough and
loop/switch break distinctions without duplicating effectful tails. Payloads
remain non-executable P1 facts; no string/enum sugar is inferred.

Before adding coverage, review found that the existing public switch test covered
unordered keys and an operand target, but not duplicate keys or a distant target.
The same test was extended table-wise for duplicate keys, payload entry and a
far out-of-range relative target. These counterexamples retain classified
Malformed/UnorderedSwitchKeys or InvalidBranchTarget; the valid packed baseline
and wrong-payload-kind counterexample remain. No test function or tooling test
was added. The existing core dispatcher interpreter ignores the new style field
while still checking the execution relation.

```sh
python3 tools/e2e.py run --case D007 --case D061 --case D083
python3 tools/e2e.py run --representation dispatch_only --case D007 --case D061 --case D083
```

On macOS arm64, final `run-3ii0ldht` and `run-m32_74em` each pass all three
selected fixed cases / eight invocations. D007 independently checks shared keys,
fallthrough and default. D083 independently checks signed min/max/default and
bounded execution without asserting source print form. D061 preserves required
original branch/payload/handler Debug information; that expected Debug result is
not substituted for either required Java case.

Authorized supplemental sources in `.cache/e2e/t14-integer-switch/` compile with
the locked javac/D8. Both adapters' actual methods compile in the unchanged helper
context and match independently authored values/events plus the reference JVM for
11 inputs: every shared/fallthrough/default key, signed min/max/default, switch
break, loop continue, named outer break and ordinary loop completion. Each
selector logs exactly one Q per reached switch; each body logs its actual E
sequence. Separate source/actual files, compiler logs and observations.json retain
the comparisons. They are supplemental core evidence, not extra fixed passes or
runner/comparator tests.

Fmt, strict all-target/all-feature Clippy, locked workspace/example release builds,
all 118 cargo-nextest tests with zero skips, diff and sensitive-input checks pass.
This batch ran the full T14-related fixed set in both modes; the last complete
mandatory report remains run-nscdi4n9, before the later rendering changes. Required
denominators and independent expectations are unchanged; TASKS.md is untouched.

T14 is accepted for the executed native/macOS-arm64 JVM profile. Actual existing
switch/copy/payload producers are reused rather than rebuilt; the missing density
choice and M12 counterexamples are now implemented and verified. Next action:
advance T15 from the ready field/access artifacts while retaining T11/T12 shared
class/body obligations.
