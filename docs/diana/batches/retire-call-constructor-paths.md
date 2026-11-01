# Retire call and constructor method paths

This slice removes the migrated receiver, call-chain and constructor whole-method
builders, replay trackers and renderers. Contracts: [DESIGN](../DESIGN.md),
[ALGORITHMS](../ALGORITHMS.md), [shared receiver calls](shared-receiver-calls.md),
[object allocation](shared-object-allocation.md) and
[constructor entry](shared-constructor-entry.md).

The engine no longer attempts those three builders. Their method AST variants,
legacy call-identity adapters, prefix renderer, adjacent-allocation tracker and
replay/storage adapters are deleted. Shared P2/P3 operations, phase-owned
initialization products and P6 storage remain the producers. The P4 constructor
resolver and this-chain proof remain active shared consumers.

P8 retains receiver/constructor declared-throws evidence and checks it against
its existing SourceThrows product. The same coverage predicate serves the P4
binding compatibility check; this does not add a recovery or retry path. Own
field evidence consumes the original identity returned by its existing checker.

## Evidence

Fmt, strict workspace/all-target/all-feature Clippy, release and cargo-nextest
pass: 122 tests, zero skips. Detached staged-patch gates pass without the frozen
static-field changes, reusing the certified D065 fixture. No tests, inputs,
Drivers, expectations or auxiliary validation framework were added.

Full `run-4a8u0jw8` retains 86 cases, 177 invocations and 181 seams. It exits 1
with 51 cases/117 seams passing. There are no lost or gained passing seams
against `run-i1u5zztp`. Partial member availability was also checked by original
method ID for D020/D021/D041/D042: each retains the same Java/Debug states;
D042's previously unsupported member remains Debug. These observations do not
count Mixed classes as accepted JavaUnit results.

Artifacts are under `.cache/e2e/retire-call-constructor-paths/`. Diff and
sensitive-input checks pass before submission. The frozen static-field draft is
preserved separately; only baseline metadata export cleanup is staged from its
shared module. TASKS and fixed independent expectations remain unchanged.

The heap method route, engine preflight/raw consolidation and final architecture
reacceptance remain in the current goal.
