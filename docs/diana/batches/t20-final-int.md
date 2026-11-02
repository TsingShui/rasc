# Own final int assignment

Goal: recover D045 through the ordinary chain with a checked own-instance final
int assignment on every normal non-delegating constructor path. Baseline:
ca830b4 plus the verified boolean preparation slice. Contracts: DESIGN §4/§4.1;
ALGORITHMS §12.2; TASKS T20. Preparation reuses accepted B01–B04, unchanged
draft-7 and the certified D045 source and DEX projections on macOS arm64.

The operator approved retaining final initialization obligations in P4 field
facts, a sealed P8 method assignment certificate consumed by P8 unit admission,
and an explicit P8 this receiver consumed by P9. No stage input or construction
authority expands. P4 validates original own-field identity and flags; P5's
existing WriteThis validates receiver provenance. P8 consumes shared P6 storage,
P3 operations and normal edges, checks finite path write sets and rejects missing,
multiple or cyclic evidence. P7's existing diamond product retains exact edges
and CopyPlans while admitting constructor frames, observable statements and join
statements. P8 independently replays final source control before publication.

The source profile requires a super-chain constructor, no protection and a
natural acyclic diamond. Delegation write sets remain pending. P8 unit admission
requires a certificate for each original constructor and final field; a missing
or damaged member leaves Mixed without withdrawing other available methods.
Ordinary and external final writes retain rejection. TextBuffer/vector growth
uses existing fallible ledgers; every traversal checks cancellation. No raw
retry, register tracker or name-based recovery route is added.

Independent evidence: tests/e2e/sources/D045/fixtures/Case.java, its Driver and
recipe; the public initialization table and existing P7/P8 replay/resource cases.
The D045 projection has a branch-selected value and a single iput at the join.
Extend the public initialization coverage with fixed missing/multiple-write and
receiver counterexamples while preserving original identity and classified exits.

## Execution

P4 field_facts::collect_own_write records OwnFinalInitialization after checking
original own-field identity/flags and rejecting static or final-volatile writes.
P5's existing ConstructorThis/WriteThis carries the receiver provenance. P8
final_assignment::verify consumes StorageBody, checks the super-chain and
unprotected natural-diamond profile, and traverses checked normal edges in
acyclic order. Its bit set represents {Zero, One, Many}; joins union incoming
sets and writes increment them. Normal returns require exactly One. Explicit
reads of the affected field, loops, switches, throw terminals and unproved
shapes retain Unsupported in this slice.

The private Certificate stores original FieldIds. P8 class_unit checks every
available original constructor against each final field and the existing full
member-set admission rejects missing/damaged members. No normal assignment is
invented. P8 field_ops::verify checks the explicit This receiver against the
original register and P5 WriteThis. P9 prints this without doing provenance
analysis. The certificate is attached only after dispatcher-operation verification
and final natural source-control replay.

P7 plan::diamond retains its four-block/four-edge exact topology and exclusive
arm/join checks, but accepts constructor frames, either condition operand form,
observable arm/join operations and return-void joins. The same P8 operations and
CopyPlans are emitted and independently replayed. No additional method builder,
SSA tracker or scheduler entry was introduced.

The new public initialization test uses a copy of the already-certified D045
DEX and catalog in crates/diana-decomp/tests/fixtures/final-assignment/. Its
independent projection fixes instructions at byte 264 and iput at PC 8. Fixed
mutations remove the write, insert an extra arm write or replace the receiver.
The missing assignment keeps the constructor Java but the class Mixed; extra
or unproved receiver writes keep original Debug/Unsupported. The wrong-receiver
row is missing evidence under the current typed profile, not a Malformed claim.
Resource checks in p8_java/source_control_tests.rs cross the existing ordinary
pipeline and reuse this input, verifying zero work/allocation and cancellation
at the new proof consumer. Every new scan and owned vector uses the existing
ledger. An initial cancellation check fired while constructing the budget; the
final test arms it after that construction to exercise the actual consumer.

Observed on macOS arm64 using unchanged draft-7 and locked Java8-target tools:

- cargo fmt --all -- --check: passed.
- cargo clippy --workspace --all-targets --all-features --locked -- -D warnings:
  passed.
- cargo build --workspace --release --locked: passed.
- cargo nextest run --workspace --all-features --locked: 161 passed, zero skips.
- python3 tools/e2e.py run --case D045: both original invocations pass, including
  complete recovered-unit javac compilation, field/member checks and independent
  source-JVM Driver observations.
- python3 tools/e2e.py run: 71/86 cases and 145/181 seams pass, with all 177
  invocations executed; exit 1 for outstanding obligations. Compared with the
  preceding boolean-preparation fixed-input run, only D045's two verdicts improve.
  Review covers all 2,379 original member observations. D045 constructors become
  Java and x retains final. D071 call-byte keeps identity, Java availability and
  diagnostics while its source changes from dispatcher to natural diamond; its
  independent Driver passes. All other complete public reports are unchanged.
- python3 tools/e2e.py run --representation dispatch_only --case D001 --case D002 --case D003 --case D004 --case D005 --case D006 --case D007 --case D008 --case D009 --case D010 --case D011 --case D012 --case D013 --case D041 --case D042 --case D044 --case D046 --case D047 --case D071 --case D082:
  20/20 selected cases and 48/48 invocations/seams pass.
- git diff --check and python3 tools/audit/desensitize_check.py --range HEAD:
  passed.

D045's fixed final-int profile is verified. Final delegation/write sets,
non-diamond initialization profiles, static final preparation and D079 joint
recovery remain T20 obligations. ART and other host platforms are not run.
