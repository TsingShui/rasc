# T22 named ownership and capture

Baseline: 71958ea. Contracts: DESIGN §4/§4.1, ALGORITHMS §12.1/§12.3 and
TASKS T22. The operator approved ownership products, capture consumers, closure
scheduling and sealed result views. Preparation reuses accepted macOS arm64
B01–B04 and unchanged draft-7 D049 inputs.

## Products and invariants

- P1 reads reciprocal EnclosingClass/InnerClass/MemberClasses metadata and checks
  duplicates, cycles and missing declarations. Original ClassDefIds, names and
  modifiers remain explicit; `$` alone establishes no ownership.
- P4 consumes original declarations and complete catalog coverage. Capture
  evidence identifies the unique synthetic final outer field, hidden parameter
  and Object constructor. Lexical package calls require the same checked nest
  and original access flags. Private fields remain accessible only from their
  original declaring class.
- P8 certifies the original capture prefix. Engine schedules constructor
  certificates before admitting capture-dependent source. A rejected certificate
  retains its diagnostic and does not admit an `Outer.this` reader. Hard resource,
  cancellation and invariant errors terminate the request.
- Ordinary methods still use P2 operations/effects → P3 SSA → P4 declarations →
  P5 types → P6 storage → P7 regions → P8 verification. P5 checks the original
  receiver/outer parameter origins; P6 and P8 independently replay the omitted
  compiler-owned assignment. General pre-super writes remain rejected.
- P8 verifies nested member sets, constructor ownership, source names and bindings.
  GeneratedCaptureField retains its original FieldKey and constructor MethodKey.
  An original static synthetic accessor keeps its name and ordinary verified
  body as Explicit; the implementation does not invent a private-access helper.
- The closure is a flat preorder with checked depths. P9 reads sealed headers,
  preparation and generated-field identities, copies verified member source and
  relocates UTF-8 class/method spans. Its stack closes lexical owners without
  recursive reports or semantic recovery. Allocation, traversal and output use
  request control with no default numeric cap.
- Missing source evidence makes the owner Mixed while retaining other class and
  method results. A single inner fragment remains Mixed; independent verified
  members remain available. No fallback repeats method analysis.

## Independent evidence

`crates/diana-decomp/tests/pipeline/nested.rs` crosses the public Engine and
renderer with the fixtures under `crates/diana-decomp/tests/fixtures/nested/`:

| Input | Observation |
|---|---|
| `d049.dex`, `catalog.json` | Certified D049 capture, original class/method/field identities and two-outer behavior |
| `Shapes.java`, `shapes.dex`, `shapes-catalog.json`, `ShapesDriver.java` | Static nested BMP/supplementary Unicode names, UTF-8 spans; original and recovered JVM outputs both `9`, `10` |
| `Tree.java`, `tree.dex`, `tree-catalog.json`, `TreeDriver.java` | Flat multilevel closure, separate `A.Same`/`B.Same` identities, private field, contained class/member spans; JVM outputs both `7`, `1`, `2` |
| `Private.java`, `private.dex`, `private-catalog.json`, `PrivateDriver.java` | Nested call through the original package accessor to a private field; JVM outputs both `7`; changed access/coverage evidence retains raw source |
| `Cycle.smali`, `Missing.smali`, `Ambiguous.smali` and their DEX inputs | Cycle/contradictory owners classify Malformed; missing member class classifies Unsupported; original results remain retrievable |
| Changed D049 prefix and `RepeatedCapture.smali`/`repeated.dex` | Wrong receiver, outer argument, super receiver, missing initialization or repeated write cannot acquire capture/source admission |

The Java fixtures use Temurin 21.0.8+9 with `javac --release 8 -proc:none -g:none`,
then D8 8.3.37 with `--min-api 26`. Their catalog declarations come from the
original classfiles through the existing classfile projector, retaining the
accepted Object catalog. Original IDs were checked with dexlib2 2.5.2 ProjectDex,
not recovered source. The Smali fixtures use smali 2.5.2. Split combined files at
`.class` boundaries before assembly. The fixtures use the engine's existing
license.

Resource regressions retain configured work/allocation failures and cancellation.
The deep-closure render also fails with BudgetExceeded when its complete output
exceeds the configured report limit; it does not publish a truncated unit. Native
checks compare each relocated member slice with its owned verified source,
validate Unicode boundaries and check original parent/child span containment.

Reproduce native acceptance:

```sh
cargo nextest run -p diana-decomp --all-features --locked t22_
python3 tools/e2e.py run --case D049
```

For each supplemental Java fixture, run the `inspect_class` example with its DEX,
original descriptor and catalog, compile the returned complete unit alongside the
tracked Driver using the pinned javac flags, and execute the Driver in a separate
JVM. Compile and execute the original source and Driver independently. No shared
compilation cache or generated expected value supplies the comparison.

## Integration results

- Native T22 selection: five nonempty public regressions pass.
- Workspace nextest: 175 passed, zero skips.
- Formatting, strict all-target/all-feature Clippy, locked release build, diff and
  desensitization gates pass.
- Full draft-7 E2E: 77/86 cases and 154/181 seams pass. D049 contributes both new
  passing seams. The complete suite still fails on existing pending task cases.
- Member regression review against the T21 evidence found no missing original
  identity, lost Java or changed failure classification outside D049. D049's
  outer constructor changes from Debug/BindingEvidenceMissing to Java; the
  closure additionally retains both original inner methods and their spans.

T22's specified F22/U07 source, ownership and origin obligations pass. The task
record reports this acceptance without changing draft-7 expectations or the
full-suite denominator.
