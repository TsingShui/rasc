# Protected Java and catch ownership

Goal: implement T18 through P7/P8 using the existing P2/P3 exception graph,
P4/P5 declaration/type evidence and T17 storage. Contracts: DESIGN §4/§3.5;
ALGORITHMS §5/§6/§9.5/§10. Preparation reuses accepted B01–B04 on macOS arm64,
draft-7 and the certified D029–D035 inputs, sources and Drivers.

The producer is P7's read-only reduction of normal regions and ordered handler
groups into protection ownership. P8 consumes the same checked operations,
normal CopyPlans and T17 placement obligations as ordinary methods. The invariant
is exact original operation/edge coverage, pre-throw snapshots, catch-parameter
binding before ingress and unchanged per-site catch selection. The original
source-JVM Drivers and independent DEX projections supply existing evidence.

Baseline: `run-dvwip56y` retains protected targets as Debug. T17's storage and
placement tests pass; actual try/catch construction is absent. D033's prepared
input contains no try table and its existing failure needs separate capability
triage rather than an altered oracle.

## Products and failure checks

P7 `protection_build` constructs disjoint normal Sequence/Diamond trees, using
reverse DAG dominance for normal follows. It rejects repeated block ownership
before expanding another subtree. Handler-to-outer-handler dependencies select
consecutive catch prefixes and their ambient suffixes. Bounded bottom-up
reduction attaches handler bodies once and accepts only whole subtrees or
contiguous runs of nodes with exclusive entry and a common normal follow.

`protection::{build,verify}` independently replays normal targets, block coverage
and each original throwing site's ordered lexical catch stack. A handler body
receives only its enclosing catches. A source-graph shape outside the acyclic,
one-nested-layer profile stops admission as Unsupported; changing a published
certificate fails as InternalInvariant. Every scan, candidate, vector growth,
subtype query and verification step charges existing work/allocation policies and
checks cancellation. Resource failures do not become a handler-free result.

P8 produces one explicit placement-obligation list and passes it to the shared
operation lowering. That consumer uses its before-throw positions and handler
ingress records to create Snapshot and ExceptionBind statements. The independent
verifier checks those statements against P6 storage rather than the lowering list. They check every snapshot against its
original site before the operation, and every ingress copy immediately after the
original exception definition. Normal copies still execute only on the selected
original edge. Protected methods use these same operations, storage, bindings,
types and CopyPlans; they do not enter an exception dispatcher or another method
analysis chain. P9 renders the checked tree without changing catch selection.

P4's existing static, receiver and constructor binding checks now accept original
invocation protection facts for declared-throws obligations. P8 validates checked
catch reachability and escaping bounds against the fixed lexical structure.
Pure moves/Phis of one ExceptionAt retain that catch identity; P8 records a
Rethrow transfer and P9 prints the effectively-final catch parameter rather than
a Throwable local. Missing hierarchy or rethrow evidence stops admission rather
than widening the method throws list. Original catch types/ranks remain intact.

The extended public Engine regression preserves the definite Undefined versus
unproved exceptional availability distinction and sibling members. With complete
catch declarations, supported original throw-state methods become Java. Its
`zzThrow` graph protects only the handler throw with a catch-all, not the original
call; it remains Debug because wrapping both in an outer catch would add a target.
D035 supplies the independently certified lexical nesting positive instead.

## Independent fixtures and execution

`tests/fixtures/protected-java/` under the engine package contains self-made Java,
a small Driver, a shared-tail Smali rejection input and their generated DEX/catalog
artifacts. They use the engine's existing license. Temurin 21.0.8+9 with
`--release 8` supplies source classfiles and the reference observations; R8 8.3.37
D8 with min-api 21 supplies the positive DEX; smali/dexlib2 2.5.2 supplies and
independently projects the rejection input. The catalog combines those source
classfiles with the already-certified required Java 8 ct.sym directories.

The reference Driver covers checked catch, catch-all return, precise checked
rethrow, cleanup failure replacing the rethrow and cleanup outside protection.
All ten fixed observations match the recovered Java compiled with the same
helpers and Driver. The recovered classpath contains no original target
classfile. This is method-context Java compilation and source-JVM behavior, not
an Engine JavaUnit or ART execution claim. Independent DEX projections and the
public Debug snapshot of the shared-tail rejection retain all ten instructions
and both ordered handler clauses.

```sh
# Java executables below resolve under the locked JDK's Contents/Home/bin.
javac --release 8 -d .cache/e2e/t18-protected-java/reference crates/diana-decomp/tests/fixtures/protected-java/fixtures/*.java crates/diana-decomp/tests/fixtures/protected-java/Driver.java
java -cp .cache/e2e/t18-protected-java/reference Driver
java -cp .cache/e2e/tools/downloads/r8-8.3.37.jar com.android.tools.r8.D8 --min-api 21 --lib "$JHOME" --classpath .cache/e2e/t18-protected-java/reference --output .cache/e2e/t18-protected-java/dex .cache/e2e/t18-protected-java/reference/fixtures/Protected.class
java -cp '.cache/e2e/tools/downloads/*' org.jf.smali.Main assemble crates/diana-decomp/tests/fixtures/protected-java/shared.smali -o crates/diana-decomp/tests/fixtures/protected-java/shared.dex
java -cp '.cache/e2e/projector/classes:.cache/e2e/tools/downloads/*' ProjectDex crates/diana-decomp/tests/fixtures/protected-java/classes.dex
java -cp '.cache/e2e/projector/classes:.cache/e2e/tools/downloads/*' ProjectDex crates/diana-decomp/tests/fixtures/protected-java/shared.dex
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
cargo nextest run -p diana-decomp --all-features --locked t18_
python3 tools/e2e.py run --case D029 --case D030 --case D031 --case D032 --case D035
python3 tools/e2e.py run
python3 tools/audit/desensitize_check.py --range HEAD
```

`JHOME` is `.cache/e2e/tools/jdk/jdk-21.0.8+9/Contents/Home` under the checkout.
The existing `inspect_class` public-library adapter retrieves the supplemental
positive and rejection methods. The recovered target is compiled directly with
`javac --release 8` into `recovered-classes`, together with the original helper
sources and Driver; `java -cp recovered-classes Driver` is compared with the
original observations. These operations exercise core recovery, not the adapter
or fixture tools themselves.

On macOS arm64, Cargo gates pass: 141 tests, zero skips. The targeted selection
runs five tests. Certificate mutations reject reordered catches, missing blocks,
wrong branch targets and expanded protection. The actual operation lowering and
snapshot verifier reject a snapshot moved after its call. Producer and independent
replay reject configured work/allocation limits and cancellation. Public checks
reject a checked catch made unreachable by removing the callee's declared checked
throws while retaining
a legal catch-all, and preserve original Debug for unassigned shared tails.

Full `run-l1endj5g` executes the unchanged draft-7 denominator: 86 required cases,
177 invocations and 181 seams. It exits 1 with 56 cases and 125 seams passing.
Against T17's `run-dvwip56y`, all previously passing seams remain passed. Review of
2379 original member observations finds exactly eight changes, all the required
D029/D030/D031/D032/D035 target observations changing Debug to Java. All other
observations retain their original identities, availability, classified failures
and Debug completeness.

Evidence: `.cache/e2e/t18-protected-java/` contains original classfile/DEX
projections, compiler logs, reference/recovered observations and classfiles,
`public.json`, `shared-public.json`, `nextest.log`, `targeted-nextest.log`,
`full-e2e.log` and `member-comparison.json`. Format, Clippy, release build, diff and
sensitive-input checks pass. The positive fixture's checked rethrow compiles
without an added `throws Throwable` or source repair.

## Acceptance follow-up

At this batch's recorded baseline, D033's prepared DEX has no try table and
reports Unsupported/BindingEvidenceMissing for
`java.lang.StringBuilder.<init>()V`, MethodId(7) at PC 2. The operator approved
repairing that T12/T11 implementation gap. The
[inherited-binding repair](t12-inherited-constructor-binding.md) records its
resolution and the passing D029–D035 source-JVM group without changing required
expected values. ART and other platforms are not run; EngineCoreRC remains
unaccepted.
