# Ordered throw sites and old-state SSA

Goal: implement TASKS T16 through the ordinary method chain. Contracts:
DESIGN §4/§5.2–§5.3; ALGORITHMS §1.6–§1.7/§2.1–§2.2/§3. Preparation reuses the
accepted macOS arm64 profile, draft-7 and locked fixture tools. T15's verified
products are recorded in `t15-derived-array-types.md`.

## Baseline and independent inputs

The recorded baseline rejects nonempty try tables before constructing FlowBody.
The fixed D029 report retains const 7 in v0, invoke at PC 1, move-result into v1
at PC 4, a later move into v0 at PC 5, and handler entry at PC 7. Its try covers
PCs [1,5). That input alone does not falsify committing a call result into the
same physical register before taking an exceptional snapshot.

The self-made `throw-state/Case.smali` supplements, rather than replaces, D029.
`overwrite`, `callInside` and `callOutside` share instructions but protect
respectively [1,5), [1,4) and [4,5). All use move-result v0. Additional members
cover non-type-ID-sorted typed catches followed by catch-all, shared handler
entries, a handler throw protected by a separate outer range, and an int result
that overwrites one word of an old wide value only on normal completion.

Temurin 21.0.8+9 compiles the independent Java reference with `javac --release 8`;
smali 2.5.2 assembles `classes.dex`. Original classfile/layout projections record
headers, instructions and try tables. The catalog retains original Smali caller
declarations and Java helper metadata. The original Driver records fourteen
result/exception/heap-trace observations: successful calls return 11, protected
failures retain 7, and the call outside protection propagates ThrowError; each
helper call records C. These are original-JVM observations, not execution of
recovered protected Java or ART certification. Fixtures are self-made and use
the engine's existing license.

Baseline evidence:
`.cache/e2e/engine/run-2s0a_s5e/D029/v1/decompile-method/{public.json,library.command.json}`
and `.cache/e2e/t16-throw-state/baseline.json`.

## Products and transfer checks

P1 publishes immutable checked protection ranges and ordered handler chains.
P2 retains original PCs/ranges, projects only proven pure adjacent result splits,
splits each protected may-throw site and publishes distinct normal and exceptional
edges. Its verifier checks original opcode effects, handler entry definitions,
normal ingress, boundaries, catch ranks/types and exact target PCs. No pseudo
uncaught handler is created.

P3 collects handler live demands before the throwing write, binds uses once,
saves only demanded old slots and wide-owner facts, and connects fixed Phis by
EntrySeed/Normal/Exceptional identities. Handler move-exception creates a fresh
ExceptionAt definition. The independent verifier replays each block's latest
register definitions and checks exceptional inputs before normal commit. It does
not substitute normal dominance or a builder boolean for availability. Temporary
live sets and construction snapshots are released after validation.

| Transfer | Checked behavior |
|---|---|
| Call/filled result | Receiver/arguments are read first; the result commits only normally. Protection belongs to the producer PC, including boundaries immediately before move-result. |
| Cast/read/allocation/division | The exceptional frame keeps the target's old definition; a successful cast refinement or other result is a separate normal definition. |
| Heap write/call effect | Register snapshots do not imply heap rollback. Original operation effects and order remain in the common products. |
| Throw terminator | Operand reads, handler live demands and pre-throw snapshots are retained; there is no normal successor. A handler throw uses only its original outer protection. |
| Wide overlap | Saved low/high ownership precedes normal invalidation; the handler does not combine words from different definitions. |

Graph expansion, metadata, sparse snapshots and verification charge existing work
and allocation policies and check cancellation. An unsupported opcode prevents
publication of a checked body; resource failure is not a handler-free success.
Protected methods stop after checked SSA, before ordinary Java typing. Unresolved
reference/catch types are not represented as a verified TypedBody.

## Failure classification and operator decision

The operator approved `RequestError::DefinitionAvailabilityUnproved { register,
pc }` and its public DiagnosticCode. Both are now implemented as Unsupported.
Normal definite bad reads, rootless values and invalid wide reads retain their
Malformed classification. Missing evidence transported only through conservative
exception edges is tracked separately and never becomes successful availability.

The supplemental `conservative` and `zUndefined` members are rejection inputs,
not valid-program or Java-recovery claims. The former has a conservative division
edge before the register definition and a later call edge after it; the latter
never defines the handler-read register. Original raw facts are independently
projected; the classification expectation follows ALGORITHMS §3.3. The public
Engine regression checks Unsupported/DefinitionAvailabilityUnproved versus
Malformed/UndefinedRegister and preserves every other original member.

## Execution and acceptance

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
cargo nextest run -p diana-decomp --all-features --locked --test contracts t16_
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

On macOS arm64, Cargo gates pass with 129 tests and zero skips. The targeted T16
selection runs four tests; the task-card contracts selection runs one nonempty
public test. Core module regressions inspect actual old-state Phi inputs, normal
result inputs, handler values, ordered/shared ingress and wide ownership. Changing
a catch rank, replacing an exceptional input with the successful result, or
changing a handler exception definition is rejected by the actual verifiers.
Protection and SSA construction independently reject configured work/allocation
limits and cancellation. Existing public T03 cases retain instruction-middle
try/handler rejection and illegal normal entry into move-exception.

Full `run-a3_g9k2k` runs the unchanged 86 cases/177 invocations/181 seams and exits
1 with 51 cases/117 seams passing. The twenty T15 heap rows still pass. Against
T15's `run-2s0a_s5e`, all passing seams and all 2379 original member observations
(status, language, failure kind/code/cause) are unchanged. Required protected Java
rows are not counted as passed; T18 owns that capability.

Evidence: `.cache/e2e/t16-throw-state/` contains original projections, reference
classfiles, `reference.txt`, compiler log, `nextest.log`, `full-e2e.log` and
`member-comparison.json`. Diff and sensitive-input checks pass. T16's exception
graph and SSA contract is verified; Java protected rendering remains deferred.
