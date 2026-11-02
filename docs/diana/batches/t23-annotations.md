# T23 runtime annotation uses and original metadata

Baseline: 3b3402d. Contracts: DESIGN §4/§4.1, ALGORITHMS §12.1/§12.3 and
TASKS T23. Preparation reuses accepted macOS arm64 B01–B04 and unchanged draft-7
D051 sources, independent projections and Driver. The operator approved the
owned annotation sidecar, P4 declaration/use/default obligations and P8 annotation
unit/source extensions before implementation.

## Implementation notes

These notes describe the current implementation slice, not new algorithm
requirements or reduced T23 acceptance criteria. DESIGN and ALGORITHMS remain
the governing contracts.

P1 `annotation_facts` consumes the existing checked directory/association reader
and iterative EncodedValues stream. It retains class, field, method and parameter
locations; original type/name/reference IDs, visibility, tags/arguments, UTF-16,
numeric bits, byte ranges and original bytes remain explicit. The owned stream is
flat, not a recursive object tree. A parse failure retains the checked prefix and
pointer evidence with its classified issue. Report and nested reports distinguish
complete, partial and unavailable metadata without suppressing independent code.

Engine schedules P4 annotation binding before unit assembly. P4 checks the native
annotation interface, complete element declarations and standard retention/target
contracts. This source-use profile accepts same-package runtime TYPE annotations
whose elements are String or int-array. An omitted element needs a checked native
default; a missing required value remains Mixed. P4 binds AnnotationDefault's
nested annotation type and element names to the actual declaring interface and
methods. Original retention is RUNTIME and Target is TYPE. P4 proves a simple
type spelling in the current Object-based scope, avoiding an ambiguous fully
qualified package prefix. Missing declarations, retention or element/type evidence
cannot be replaced by a default.

P8 verifies source names, UTF-16 literal escaping and actual classfile UTF-8/array
limits. Its owned clause/provenance product enters the existing unit binding;
ordinary methods retain the shared P2–P8 chain. P9 prints the sealed header and
original facts without interpreting retention or repairing values. Source-opaque
field annotations reject a complete ordinary/nested unit, even when all its
methods are Java. Original metadata remains available through the public library
and inspect JSON/text. System metadata is not rendered as a Java annotation.

For annotation definitions, Engine passes the ready P4 product into the existing
P8 declaration/unit entries. P8 admits the original no-code element declarations,
checks the exact member set and builds `@interface` syntax and class-owned default
clauses. Those clauses have sealed MethodId/source mappings; P9 only projects
them into member spans. Individual declaration fragments remain distinct from
complete owner source. Default clause production checks the configured element
output limit as well as the literal format limits. Known same-package `java`
aliases reject the qualified standard-annotation spelling rather than importing,
renaming or omitting metadata.

Malformed class metadata retains its error and checked original method snapshot;
it is not relabeled successful Java or empty annotations. Cancellation/resource
failures abort rather than publishing a partial successful unit. Existing opt-in
work/allocation/output controls apply; no default numeric cap is added.

## Independent evidence and preparation update

`tests/e2e/sources/D051/fixtures/{Case,Mark}.java` and the tracked Driver establish
runtime Mark(value="x",nums={1,2}). Independent original projections give Case
ClassDefId 0, Mark ClassDefId 1 and Case constructor MethodId 0. Case's annotation
bytes are 813..826; Retention and Target items start at 826 and 832. Original
retention/target field references and element return types are checked natively.

The original fixture JDK catalog omitted Annotation, Retention and Target. The
preparer now extracts those three actual dependencies from locked Java8 ct.sym.
The real D051 run also exposed stale catalog size evidence: manifest enumeration
registered the old derived catalog before rewriting it. Excluding that one
producer-owned path from the earlier generic scan registers it after production.
No auxiliary-tool tests were added. `python3 tools/e2e.py verify-inputs` rechecks
86 cases, 177 invocations and 144 scalar/JVM cross-checks. B01 expectations and
B04 comparator behavior are unchanged; digest records remain informational.

Public coverage is `crates/diana-decomp/tests/pipeline/annotations.rs` with
`tests/fixtures/annotations/` inputs:

- `d051.dex` is the unchanged certified D051 input. Its narrowed catalog retains
  required original fixture/JDK declarations, not a full platform catalog.
- `Text.java`/`text.dex` and `TextDriver.java` cover NUL, unpaired surrogate units,
  literal backslash-u and signed int-array values. Original and recovered isolated
  JVMs both print UTF-16 `[0,55296,65,56320,92,117,48,48,48,97]` and
  ints `[-1,0,2147483647]`.
- `Field.java`/`field.dex` retain a runtime Deprecated field annotation. Both methods
  stay Java, but the complete unit remains Mixed with the original field metadata.
- Missing Marker/retention API/retention field facts, removed native retention,
  a void element, a mismatched value kind, a missing required value and a known
  `java` type alias remain Mixed with available Java.
- A malformed value retains annotation bytes 813..825 as a prefix and the
  constructor remains Java. A malformed annotation header is Unavailable metadata
  with its checked pointer and error; the hard member result retains all original
  constructor words `[4208,3,0,14]` in its snapshot.
- `Defaults.java`, `Defaulted.java`, `Override.java` and `defaults.dex` cover
  annotation definition/default recovery and both omitted and explicit empty-array
  values. Independent published-format projection fixes Defaults ClassDefId 1,
  element MethodIds 1/2 and AnnotationDefault bytes 985..1007; its nested type is
  TypeId 2. Wrong default value tags, wrong nested owners and unknown element
  names prevent unit admission. Duplicate default names retain Malformed and
  original members; failures in the annotation definition do not discard the
  annotated classes' constructors.
- The 50-byte element output limit admits the no-default D051 definition but
  terminates default source production with MaxMethodOutputBytes. This exercises
  the unit-owned clause path, not just the shorter declaration fragments.

The unchanged D051 Driver also passes when both Case and Mark are recovered and
compiled together, without the original Mark dependency: valueUnits `[120]`,
nums `[1,2]`. `DefaultsDriver.java` compiles once with original classes and once
with all three recovered classes in isolated outputs. Both print the same string
units and integer arrays above, `-1` after modifying a returned annotation array,
then the reflected element defaults, `z:0`, `RUNTIME` and `[TYPE]`.

The existing public T04 association regression also checks the owned sidecar's
class/field/method/parameter identities and unpaired string units while another
method's code range is damaged. This reuses the independent fixture and does not
read that unrelated code to recover metadata.

Supplemental inputs use Temurin 21.0.8+9, `javac --release 8 -proc:none -g:none`
and D8 8.3.37; original classfile projection supplies declaration facts. Source
Marker and Drivers are compiled independently of recovered output. Fixtures use
the engine's existing license.

## Execution and next batch

- Seven public source/metadata regressions and the extended T04 association case
  pass; workspace nextest passes 187 tests, zero skips.
- Formatting, strict all-target/all-feature Clippy and locked release build pass.
- The fixed D051 javac/JVM Driver passes. Full draft-7 E2E passes 79/86 cases and
  156/181 seams; D051 is the only changed seam against the enum baseline.
- Original member identity, Java availability and classified member failure review
  found no regressions. Diff and tracked-diff desensitization checks pass.

T23's restricted enum/annotation profile now has canonical enum evidence in the
preceding batch and annotation declaration/use/default evidence here. The remaining
full-suite failures are existing application/input/query/output seams; their failed
acceptance is not counted as an engine CoreRC pass. ALGORITHMS is unchanged by
this batch.

Reproduce the workspace and fixed-case evidence from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo build --workspace --release --locked
cargo nextest run --workspace --all-features --locked
python3 tools/e2e.py verify-inputs
python3 tools/e2e.py run
```

With JAVA_HOME set to the locked JDK and CLASSES set to a fresh output directory,
reproduce the supplemental source observation:

```sh
"$JAVA_HOME/bin/javac" --release 8 -proc:none -g:none -d "$CLASSES" \
  crates/diana-decomp/tests/fixtures/annotations/{Defaults,Defaulted,Override,DefaultsDriver}.java
"$JAVA_HOME/bin/java" -cp "$CLASSES" fixtures.DefaultsDriver
```

To recover each definition/use, run the public `inspect_class` example on
`crates/diana-decomp/tests/fixtures/annotations/defaults.dex`, selecting
`Lfixtures/Defaults;`, `Lfixtures/Defaulted;` and `Lfixtures/Override;`, with the
neighboring `defaults-catalog.json`. Compile their returned Java text with the
same unchanged Driver into another fresh output directory and run the same entry.
Generate the supplemental DEX with locked D8 8.3.37 from the three original
classfiles, excluding the Driver; the tool pin is `tests/e2e/toolchain.lock.json`.
