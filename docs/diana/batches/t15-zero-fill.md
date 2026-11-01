# T15 zero-count primitive fill

Goal: discharge D073 with an original-site array length read, preserving NPE and
performing no writes. ALGORITHMS §2.4 owns component/width and FreshArray rules.
Use declared primitive array types rather than infer a component from payload width;
keep non-empty shapes raw until their own certificate is available.

P5 records the declared primitive array component and exact width for all eight
primitive kinds. The zero-fill producer tracks initialized parameter aliases and
retains each original fill register, site and payload target. P8 independently
replays the parameter descriptor, aliases, payload count/width and return site.
P9 emits one unused int length read at each original zero-fill site, with no store.
Known reference-array or component/width misuse is Malformed/
InvalidArrayPayloadType; undefined alias reads retain UndefinedRegister.

The first related run passed D073 but exposed D074's existing wrong raw reasons:
JavaAnalysisUnavailable/BindingEvidenceMissing rather than required ArrayFillShape.
A bounded read-only shape check now classifies unproven non-empty fills before
unrelated missing call binding. It declines borrowed, too-short, unknown-length,
escaped and non-linear/protected shapes. Its straight-line facts are invalidated
by barriers and it tracks initialized words while that prefix remains known.
Branch/protection detection precedes type conclusions, so a physical scan cannot
claim type facts across a join. Passing this precheck is not a FreshArray product
and does not authorize Java; the eventual non-empty lowering still needs its own
sealed certificate and independent replay. Known original P1 failures remain hard.
The cast printer also rejects contradictory access flags before emitting source.

```sh
python3 tools/e2e.py run --case D073 --case D074 --case D025 --case D080
python3 tools/e2e.py run
```

On macOS arm64, final `run-7b72vlsk` passes all four related fixed cases / 11
invocations. D073's array/null calls are Java and match the independent JVM;
D074's four shapes preserve complete original raw facts with ArrayFillShape.
Expected raw is not counted as a substitute for required Java. D025/D080 retain
their previous semantics and classifications.

Authorized supplemental Smali/Java under `.cache/e2e/t15-zero-fill/` covers every
primitive component through an original alias. All eight actual methods compile;
16 array/null observations match independently authored values plus the reference
JVM, including unchanged true bits, signed extremes and negative FP zero. Three
additional valid-format inputs independently produce Malformed for wrong width,
reference-array payload and an undefined alias with no catalog. Original/actual
sources, locked compiler commands/logs and observations.json remain separate.
These are core observations, not auxiliary-tool tests or extra fixed passes.

The complete final `run-a6jolxi7` retains 86 mandatory cases / 177 invocations /
181 seams: 46 whole cases / 107 seams pass. Only D073's two and D074's four seams
are newly passing relative to run-c53evc4a; no pass is lost. The full command exits
1 for real remaining obligations. Fmt, strict all-target/all-feature Clippy,
locked workspace/example release builds, all 118 cargo-nextest tests with zero
skips, diff and sensitive-input checks pass. Existing core tests were reused;
TASKS, independent expectations and required denominators are unchanged.

T15 remains pending. Next action: construct the real FreshArray certificate and
ordered exact stores needed by required-Java D027.
