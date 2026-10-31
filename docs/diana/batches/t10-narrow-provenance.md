# T10 narrow producer bounds

Goal: recover D071's narrow returns and all D072 observations using producer
bounds through moves/Phis, without truncating arithmetic or changing descriptors.
Contracts: ALGORITHMS §4.9 and TASKS T10. The D071 call still requires T11's exact
binding producer. Existing numeric, boolean/null, CFG and resource evidence remain
regressions.

The CFG producer retains the descriptor type separately from the int carrier.
It seeds ranges from byte/short/char/boolean declarations or original literal
bits. Moves copy bounds; a Phi takes the closed hull, with None as Bottom.
Arithmetic establishes full I32 again. Endpoint changes are monotone, finite
and charged; no invocation values or desired formal types supply evidence.

P8 retains explicit ProvenNarrow return nodes only when the finalized range is
non-empty and contained in the target range. Known01 can refine the closed range
without restarting its propagation. P6 edge copies keep their exact int storage
types; P9 casts only at the verified ordinary return. Header parameter and return
descriptors stay byte/short/char. The independent dispatcher checker validates
the declared narrow target and the actual source range.

On macOS arm64, `.cache/e2e/engine/run-1jhy2xf7/report.json` records 28 selected
cases and 69 invocations: 27 whole cases and 68 seams passed. All six D071 return
calls and all four D072 calls passed actual source compilation/JVM comparison.
D071 call-byte remains Debug, so the whole case failed and the command exited 1.

```sh
python3 tools/e2e.py run --case D001 --case D002 --case D003 --case D004 --case D005 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D014 --case D016 --case D017 --case D055 --case D056 --case D057 --case D058 --case D059 --case D060 --case D061 --case D062 --case D063 --case D064 --case D071 --case D072 --case D083
```

Supplemental core operations in `.cache/e2e/t10-narrow-provenance/` exercise
missing proof negatives. In the certified D071 bytes, change b's final B formal
to S or I, updating the corresponding shared type-list and shorties. The Phi
then contains byte+short or byte+int and cannot return byte losslessly. Both
`b(ZBS)B` and `b(ZBI)B` retained complete Debug with Unsupported. A third copy
replaces the original if's first two words at byte 448 with
`[0x0190,0x0201]` (add-int v1,v1,v2), retaining the legal goto to the original
return. It likewise refused to reuse the byte parameter bounds after addition.
These are public Engine observations, not additional fixed-case passes.

```sh
target/release/examples/inspect_class .cache/e2e/t10-narrow-provenance/short.dex 0 0 b '(ZBS)B' --output .cache/e2e/t10-narrow-provenance/short.json
target/release/examples/inspect_class .cache/e2e/t10-narrow-provenance/int.dex 0 0 b '(ZBI)B' --output .cache/e2e/t10-narrow-provenance/int.json
target/release/examples/inspect_class .cache/e2e/t10-narrow-provenance/arithmetic.dex 0 0 b '(ZBB)B' --output .cache/e2e/t10-narrow-provenance/arithmetic.json
```

Only format-defined DEX header fields were populated after the local mutations;
no fingerprint gate was introduced. Workspace fmt, all-target/all-feature Clippy
with warnings denied, locked release build and all 116 cargo-nextest core tests
with zero skips passed. No new tests, fixed input/expected changes or TASKS.md
edits were introduced. Diff and sensitive-input checks passed before commit.
T10's remaining cmp and literal/target obligations are still pending.
