# P0 — Class facts

Code: `crates/diana-decomp/src/p0/`, with the application adapter in
the engine development shell `crates/diana-decomp/examples/dbg_shell.rs`.

## Input and ownership

`p0::collect` consumes a selected `ProgramIndex` and its original `ClassKey`. P0 reuses the shared checked declaration and metadata
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
encoded values retain checked bytes, format tags and flat value events. Reference
values retain an owned declaration pool, not only numeric IDs; nested annotation
descriptors are delivered too. Field and interface declarations preserve original
TypeIds alongside descriptor-read states.

The collector retains class, field, method and parameter annotations with their
original associations, visibility, descriptor, physical ranges and value events.
Nested metadata uses the shared reader's flat container-event representation. System annotations
retain raw facts; P0 does not infer nested ownership or generic source declarations.
MethodMetadata shares these owned immutable records with the method chain. Its
read-only iterators expose only the selected method/parameter records and their
referenced declaration leaves. Class
traversal extent/issue remains explicit: an incomplete traversal does not establish
absence of later method annotations.

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
An unlocatable member-record suffix terminates the shared walk without discarding
located members. A damaged composite preparation retains its decoded prefix;
the already checked root count still establishes independently omitted defaults.
Unlocated encoded entries remain NotRead, not zero/null or implicit defaults.
Allocation failures and internal invariants stop collection or rendering.
One member's declaration or preparation failure does not discard independently
readable siblings. Each direct interface keeps its own fact state, so an
unreadable interface does not discard the other original references.
A damaged class identity or container that prevents selection produces an error.

Copies and JSON growth check capacity, then reserve fallibly. No numeric quota is
introduced by P0. All offsets address the original physical DEX, including DEX 041
shared data; the logical header offset remains part of identity.

## Same-DEX superclass relationships

The current declaration remains an owned `OwnerFacts` snapshot. Method preparation
adds a private `OwnerContext` view that shares the selected logical DEX's restricted
original declaration reader. This view travels with the current declaration through
P1/P2/P3; it exposes no ProgramIndex, code reader or P0 construction entry to P4.
The existing current-class declaration is reused rather than materialized again.

`SuperclassRelation` distinguishes Same, Ancestor with depth, Unrelated and
Unknown with a `SuperclassGap`. The query compares original TypeIds only inside
this logical DEX. A matching superclass reference establishes the relationship
without requiring that target's definition. Continuing beyond that reference
requires an unambiguous local class definition. A missing definition, damaged
record, duplicate-definition ambiguity or cycle does not prove Unrelated. Concrete
input errors retain their classification in the gap; the current declaration's
original failure remains in OwnerFacts.

Current-class and direct-parent queries use the existing declaration without
building a class-location index. A deeper query initializes the shared local
class-location index on demand and reads only required ancestor headers through
the same checked reader as ordinary P0 declarations. It does not enumerate members,
open method bodies, build an all-ancestor closure or search another logical DEX.
The actual local class-def count bounds traversal of a cyclic chain; this is not
a numeric resource quota. Resource/invariant lookup failures are propagated, not
cached as absent definitions.

## Development inspection

The engine development shell is the single P0–P8 observation entry:

```sh
cargo run -p diana-decomp --example dbg_shell --locked -- \
  --dex classes.dex --exec '{"cmd":"p0","class":0}'
```

It reads a bare DEX, emits one JSON response per command and does not re-read the
input for presentation. P0 no longer has a separate debug renderer or example.
