# P0 — Class facts

Code: `crates/diana-decomp/src/p0_class/`, with the application adapter in
`apps/diana/src/analysis/p0.rs` and the internal `p0_debug` Cargo example.

## Input and ownership

`p0::collect` consumes a selected `ProgramIndex`, its original `ClassKey` and the
caller's `RunContext`. P0 reuses the shared checked declaration and metadata
readers; those readers own range checks, class-data delta decoding, declaration
checks and encoded-value grammar. The input snapshot and APK selection remain
application-owned. P0 performs one class-data traversal for both the field and
method directories.

The collector reads the selected class's descriptor, flags, superclass, direct
interfaces and source-file name. It enumerates actual field and method definitions,
not all references in the DEX ID tables. The four original groups and their ordinal
positions come from class-data, never from flags. All methods occupy one list, including
constructors, class initializers, abstract and native methods. Code offsets are
original class-data integers, not evidence that a code item exists or is valid.
The collector MUST NOT open code items or invoke method analysis.

Method declarations retain exact name/prototype text, shorty, parameter type
identities and word layout, return type identity and parameter count, so the method
reader does not resolve the declaration record again. Fields retain storage group,
static ordinal, original declaration and encoded static preparation bytes. An omitted static entry is `implicit_default`, distinct
from an explicitly encoded default. Instance fields have no encoded static initial
value. Scalar strings and floating-point bits retain their original values. Other
encoded values retain their complete checked bytes and format tag.

The collector retains class, field, method and parameter annotations with their
original associations, visibility, descriptor, physical ranges and value events.
Nested metadata uses the P1 flat container-event representation. System annotations
retain raw facts; P0 does not infer nested ownership or generic source declarations.

## Snapshot and failure behavior

`ClassFacts` owns copied facts and metadata. Class and method requests pass it to
the method reader, which obtains the bound method entry without re-walking
class-data. Its `complete`/`partial` status describes only attempted declaration,
preparation and metadata reads. Method contents remain `not_read` when the
class-data code offset is nonzero and `absent` when it is zero.

A fact uses `present`, `absent`, `not_read` or `unavailable`. A failed local read
MUST NOT become an absent fact. Declaration failures and annotation prefixes retain
the independently available members and metadata. Section diagnostics explain
incomplete traversal; a delivered prefix MUST NOT claim complete coverage.
Cancellation, allocation failures and internal invariants stop collection or rendering.
One member's declaration or preparation failure does not discard independently
readable siblings. Each direct interface keeps its own fact state, so an
unreadable interface does not discard the other original references.
A damaged class identity or container that prevents selection produces an error.

Copies and JSON growth check capacity, then reserve fallibly. No numeric quota is
introduced by P0. All offsets address the original physical DEX, including DEX 041
shared data; the logical header offset remains part of identity.

## Debug presentation

`p0::render_json` renders the existing snapshot through `serde_json` as pretty JSON
with `schema_version: 1` and `stage: p0`. It does not re-read the input. Original
text is displayed as readable `text`, with isolated surrogates escaped; internal
UTF-16 units are not duplicated in the debug output. Original access flags include
the raw word, per-kind names and unknown bits.

The top-level sections are `identity`, `status`, `class`, `fields`, `methods`,
`annotations` and `diagnostics`. Annotation locations use original member IDs and
prototype parameter positions.

The internal debug entry is a Cargo example, not a Diana CLI command:

```sh
cargo run --release --locked -p diana --example p0_debug -- \
  sample.apk 'com.example.App' -o p0-class.json
```

The example accepts the existing exact class selector. `-o` uses the application
atomic publisher and refuses an existing target or an input alias. Without `-o`,
pretty JSON goes to stdout. A complete declaration snapshot exits zero; a partial
snapshot is published with its fact states and diagnostics and exits non-zero.
Errors that prevent collection or rendering produce no new output file.
