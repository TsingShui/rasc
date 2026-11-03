# 01 — Input and original facts

Code: `apps/diana/src/analysis/{input,archive,apk,request,images}.rs` and
`crates/diana-decomp/src/p1_input/`.

## Ownership

`RequestScope` owns an input lease for the request and supplies borrowed root DEX
views. `DexContainer` borrows the original physical bytes; each logical member is
identified by its original header offset. The engine does not open files or ZIP
entries. Dropping the request scope does not modify the input.

## DEX admission

`DexContainer::open` recognizes little-endian DEX versions 035, 037, 038, 039,
040 and 041. Unknown versions and reverse-endian input are classified unsupported;
malformed headers and ranges retain their input error classification. A checked
physical header chain alone does not validate all logical-member maps, code,
references or Java representability.

For 035–040 the single header begins at byte zero. For 041, logical headers form
a checked chain within the original physical container. Member offsets refer to
physical bytes; shared data is not copied into fabricated independent DEX files.
Original identities include the physical input and logical header as well as table
indices. Byte offsets, code-unit offsets and UTF-16 coordinates remain distinct.

## Reads and original metadata

Readers MUST check arithmetic, lengths, offsets and counts before slicing or
allocating. Reads and loops consume the request's resource controls and check
cancellation. No default numeric cap is implied; an explicitly configured limit
MUST NOT become silent successful truncation.

`ProgramIndex` exposes original class and member facts. `MethodInput` separates a
checked method body from a declaration-only method. UTF-16 units and available
annotations, debug information, references and raw instruction coverage remain
available without pretending an incomplete decode is complete. A local damaged
member MUST NOT erase independently available members.

## Detailed format rules

During migration the detailed header, map, instruction-width and metadata rules
remain in [ALGORITHMS §1](../ALGORITHMS.md#1-reader-format-tables-and-raw-instructions-t02t04).
The application file and archive contracts remain in
[DESIGN §6–7](../DESIGN.md#6-cli-contract). These references do not authorize a
second DEX decoder in the application.
