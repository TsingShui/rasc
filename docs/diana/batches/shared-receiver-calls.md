# Receiver calls in shared flow

## Slice

This slice migrates existing initialized virtual/interface calls in ordinary
static callers. Contracts: [DESIGN](../DESIGN.md), [ALGORITHMS](../ALGORITHMS.md)
and [the architecture baseline](architecture-baseline.md).

P2 retains invoke kind, original MethodId, ordered receiver/argument registers
and result adjacency. P3 independently checks the original opcode kind and uses.
P4 collects original owner/name/prototype/formal constraints without selecting a
receiver expression. P5 separates the nominal receiver use from converted
argument uses. P8 invokes the existing receiver access/subtype/overload/throws
validator after typing, then consumes each binding into the owned AST. The
independent P8 checker compares original constraints, receiver identity, argument
views and call sites. Both representations use the same P6 storage and statement
producer. The public owned call iterator retains original receiver and invoke
kind. P9 only writes the verified receiver expression and arguments.

Nominal naming now accepts plain public interface reference types in the shared
P8 naming consumer. Class-only constraints used by existing allocation/cast
producers retain their former checks. Receiver formals/results retain the
existing int profile. Direct/super, implicit-this callers and initialization
paths remain temporary routes; the calls/fields task remains open.

## Core evidence

The existing shared-fields fixture adds Reader.java and Case.invoke, reusing the
field/call/array operation composition. Reader.value delegates to the existing
Dep callback. The existing public Engine regression adds this method to its
complete/missing/contradicted-native-volatile declaration table, without adding
a separate test function or harness.

The recipe is the [instance-field recipe](shared-instance-fields.md) plus
Reader.class. The existing Driver retains its 78 observations and adds eight
fixed receiver boundaries: base/override dispatch, null receiver, null field
receiver, null/empty array after the call, callback failure, and a different
argument/index. These replace an initial expanded receiver Cartesian product
at the user's direction. Both representations match the original reference
program on all 86 retained observations. Captured return, exception, call trace,
field and array state are independent of engine output. These remain supplemental
method-context observations, not fixed cases or JavaUnit acceptance.

The fixed D023 interface methods now emit shared storage and receiver statements;
both fixed invocations retain their passes. Existing prepared expectations and
inputs were reused.

## Executed gates

- Format, strict workspace/all-target/all-feature Clippy, release build and
  cargo-nextest passed; 122 core tests, zero skips.
- Related default E2E `run-_nlf7geu`: overall failed; 8/10 cases and 17/19
  invocation seams passed. D020/D021 retain their earlier failures; no lost pass.
- Full required E2E `run-qmrpwcl1`: overall failed; 51 cases/117 seams passed
  out of 86 cases/177 invocations/181 seams. No lost pass relative to
  `run-m7z0ts4o`.
- The production/test patch was checked independently of the frozen static-field
  changes in a detached worktree: format, Clippy, release and all 122 core tests.
- Diff and sensitive-input checks passed before submission.

Artifacts are under `.cache/e2e/shared-receiver-calls/`. The frozen static-field
changes remain uncommitted. The unified failure exit and deletion of legacy
method routes remain pending migration parity.
