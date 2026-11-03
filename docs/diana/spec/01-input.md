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

The application file and archive contracts remain in 10-cli. These contracts do not authorize a second DEX decoder in the application.

## Reader, format tables and raw instructions

### Address space and read functions

`Cursor` holds `bytes: &[u8], pos: usize, end: usize`; every read returns a Result, and the
implementation MUST NOT index first and check afterwards. `range(off,n,stride)` verifies through
`n.checked_mul(stride)` and `off.checked_add(size)` and only then takes the slice.
Fixed records keep a borrowed view; a variable-length record that needs decoding first verifies
the remaining bytes and the minimum record size, charges the budget, and only then calls
try_reserve. Offsets use `FileByte` and `CodeUnit` separately; a call site MUST NOT multiply by
2 inline.

- Accept the defined 035/037/038/039/040/041 containers; 036 and unknown versions are
  UnsupportedContainer. Version recognition does not mean that every Java opcode of that version
  is supported. reverse-endian is Unsupported.
- 035–040 require header_offset=0, header_size=0x70, and file_size equal to the physical DEX
  length of this read.
- 041 requires header_size=0x78; the 0x70/0x74 fields are container_size/header_offset
  respectively. Advance from 0 by each header's file_size with checked arithmetic and end exactly
  at container_size; every header has a container_size equal to the physical length and a
  header_offset equal to its actual position, and the step is at least 0x78.
- The section/data/code/debug offsets of 041 are absolute offsets from the **physical container
  start**. They MAY reference data of a later logical member and MUST NOT reference data before
  the current header. The implementation MUST NOT truncate shared data by logical file_size and
  MUST NOT add header_offset twice. data_size/data_off are not used as a data-region boundary in
  041.
- Every input view is the original container plus header_offset. The implementation MUST NOT
  overlay the header and MUST NOT recompute “original bytes”. It verifies the physical header
  chain once and builds the catalog of a logical member only after selecting that member; the same
  physical slice MUST NOT count twice toward input bytes.
- Keep header checksum and signature as original fields only; they are not a memory-safety proof
  and do not claim complete ART file authentication. The fixture content lock and the ZIP
  extraction CRC are independent contracts and are not cancelled by this.

#### Header and 041 section checks

All offsets in the following table are bytes relative to the current header;
stored section offsets address the physical container, not this table.

| Header byte offset | Field / interpretation |
|---|---|
| `0x00 / 0x08 / 0x0c` | magic[8] / checksum u32 / signature[20] |
| `0x20 / 0x24 / 0x28` | file_size / header_size / endian_tag, each u32 |
| `0x2c / 0x30 / 0x34` | link_size in bytes / link_off / map_off |
| `0x38 / 0x40 / 0x48` | string_ids / type_ids / proto_ids: u32 count followed by u32 offset |
| `0x50 / 0x58 / 0x60` | field_ids / method_ids / class_defs: the same count/offset pair |
| `0x68 / 0x6c` | data_size / data_off; unused as 041 data-region bounds |
| `0x70 / 0x74` | 041-only container_size / header_offset |

For 041, each non-final header step is four-byte aligned. Every counted section
has both size/count and offset zero, or both non-zero. A non-empty section lies
after the header that defines it and within the physical container. References
reached transitively through shared tables obey that same lower bound for every
logical member using them. The reader MUST check the complete item range, not
only its first byte; a forward start with an end beyond container_size is invalid.

The reader MUST apply the DEX format/constraints alignment per item type. Header,
ID tables, class_defs, map_list, type_list, annotation sets/directories and
code_item use four-byte alignment; byte-encoded strings, debug, annotations,
class_data and encoded arrays do not acquire an invented four-byte restriction.
Inside one member's map, types occur once, entries follow physical offset order,
and actual item extents do not overlap. Variable item extents come from checked
parsing, not the count times a guessed stride. The map's header entry describes
this header with its version-specific size; ID entries agree with this member's
header. A later header is not data that an earlier code/string item may swallow.

Different logical members may share the same permitted data range; a global
"all member ranges disjoint" check would reject legal sharing. Sharing bytes
never shares an index namespace: refs inside a method are resolved only against
its selected member's tables. The reader MUST NOT merge declarations by equal
numeric index or descriptor. Unknown container versions are UnsupportedContainer;
a recognized 041 with a broken chain, illegal range, alignment or conflicting
map is Malformed. These checks follow the published DEX format and constraints,
not the assumptions of a 035 parser with an overlaid header.

Fixed table strides: string_id/type_id=4, proto_id=12, field_id/method_id=8, class_def=32.
The reader first verifies map_list's `4+12*n`, type uniqueness, the offset order required by the
specification, and consistency of the fixed tables with the header; the reader MAY read
references only after that. The type and proto count limit is 65535; other tables are constrained
by the actual
range, index width and resource quotas, and the reader MUST NOT cast a u32 count to u16 first.
After the four class_data counts it accumulates the field and method ULEB deltas separately;
each group starts from 0, uses checked_add, and publishes declarations only after validation of
the index, the owner it belongs to and duplicate definitions.

ULEB32 spans at most 5 bytes; the 5th byte MUST NOT continue the encoding and its payload is
<=0x0f. The 5th SLEB32 byte payload MUST be 0x00..0x07 or 0x78..0x7f. Accumulate in u64
first, then sign-extend and range-check; do not take abs on MIN. The reader MUST NOT add a
“shortest encoding required” restriction that the specification does not require.

`code_item`: 16-byte header → insns_size*2 → 2-byte padding when insns is odd and tries>0 →
8*tries → encoded_catch_handler_list. handler_off is relative to the **handler list start**;
branch and payload are relative to the original instruction PC; handler size is handled as i64
for negative values. Try ranges are ordered and non-overlapping, start and end on valid
instruction boundaries, and their targets are not payloads. code_off=0 for abstract/native
produces a declaration only and does not run the method pipeline.

MUTF-8 produces `Vec<u16>`: 00 terminates, C0 80 encodes NUL; one/two/three-byte units are
verified per the specification, surrogate units are kept, and the ordinary UTF-8 four-byte path
MUST NOT be accepted in place of this encoding. The declared utf16 length MUST match exactly.
String-pool literals may hold isolated surrogates; descriptors and names are additionally
validated against DEX declaration syntax, and the implementation MUST NOT look them up as
members after a lossy conversion. Display, Java literals and original identity are three
different purposes.

### Format bit fields and the complete opcode registry

Let `w0..w4` be u16 code units; `a8=w0>>8`, `a4=(w0>>8)&15`, `b4=w0>>12`.
`s8/s16/s32` denote interpretation of that width as signed; 32/64-bit words are always
concatenated little-endian.

| Format | code units | Decoded fields |
|---|---:|---|
| 10x / 12x / 11n / 11x / 10t | 1 | no operand and high byte 0 / A=a4,B=b4 / A=a4,lit=s4(b4) / A=a8 / delta=s8(a8) |
| 20t / 22x | 2 | high byte 0,delta=s16(w1) / A=a8,B=w1 |
| 21t / 21s / 21h / 21c | 2 | A=a8, and respectively delta=s16(w1), lit=s16(w1), high-bits=w1, index=w1 |
| 23x / 22b | 2 | A=a8,B=w1&255,C=w1>>8 / lit=s8(w1>>8) |
| 22t / 22s / 22c | 2 | A=a4,B=b4, and respectively delta=s16(w1), lit=s16(w1), index=w1 |
| 30t / 32x | 3 | high byte 0,delta=s32(w1|w2<<16) / high byte 0,A=w1,B=w2 |
| 31i / 31t / 31c | 3 | A=a8, and respectively bits32, signed payload delta, unsigned jumbo index, assembled from w1/w2 |
| 35c | 3 | count=b4<=5,G=a4,index=w1; C/D/E/F are the four nibbles of w2 from low to high, and the actual registers are the first count items of [C,D,E,F,G] |
| 3rc | 3 | word_count=a8,index=w1,first=w2; when count>0, check first+count<=registers_size with checked arithmetic |
| 45cc / 4rcc | 4 | follow the 35c/3rc layout respectively, with w3 additionally holding the proto index; 038+ |
| 51l | 5 | A=a8,bits64=w1|w2<<16|w3<<32|w4<<48 |

The opcode registry expands over the following closed ranges; one `OpcodeSpec` holds the format,
version, mnemonic, ref kind, operand shape, read/write slots, effect and Java lowering kind,
and the decoder and raw printer read only this table:

| opcode | format / meaning |
|---|---|
| 00 | 10x nop; 0100/0200/0300 complete words are additionally recognized as payload signatures |
| 01..09 | move / move-wide / move-object, three each, in order 12x, 22x, 32x |
| 0a..0d / 0e / 0f..11 | 11x result/result-wide/result-object/exception; 10x return-void; 11x return/return-wide/return-object |
| 12..15 | 11n const/4, 21s const/16, 31i const, 21h const/high16 |
| 16..19 | 21s const-wide/16, 31i const-wide/32, 51l const-wide, 21h const-wide/high16 |
| 1a..1c | 21c const-string, 31c const-string/jumbo, 21c const-class |
| 1d..1f / 20 / 21 | 11x monitor-enter/exit; 21c check-cast; 22c instance-of; 12x array-length |
| 22..27 | 21c new-instance, 22c new-array, 35c/3rc filled-new-array[/range], 31t fill-array-data, 11x throw |
| 28..2c | 10t/20t/30t goto[/16,/32]; 31t packed/sparse-switch |
| 2d..31 | 23x cmpl-float/cmpg-float/cmpl-double/cmpg-double/cmp-long |
| 32..37 / 38..3d | 22t if-eq/ne/lt/ge/gt/le; 21t the corresponding if-*z |
| 44..51 | 23x the seven aget forms / the corresponding seven aput forms: ordinary, wide, object, boolean, byte, char, short |
| 52..5f / 60..6d | 22c iget/iput for the same seven kinds; 21c sget/sput for the same seven kinds |
| 6e..72 / 74..78 | 35c virtual/super/direct/static/interface; 3rc the corresponding range forms |
| 7b..8f | 12x neg/not and primitive conversions, registered in the order of the Dalvik bytecode name table |
| 90..af / b0..cf | 23x binary and 12x /2addr; 11 kinds each for int/long, 5 kinds each for float/double |
| d0..d7 / d8..e2 | 22s /lit16: add,rsub,mul,div,rem,and,or,xor; 22b /lit8 additionally shl,shr,ushr |
| fa/fb/fc/fd | 038+: 45cc/4rcc invoke-polymorphic[/range], 35c/3rc invoke-custom[/range] |
| fe/ff | 039+: 21c const-method-handle, const-method-type |

The unary 7b..8f order is neg-int,not-int,neg-long,not-long,neg-float,neg-double,
int-to-long/float/double,long-to-int/float/double,float-to-int/long/double,
double-to-int/long/float,int-to-byte/char/short. The binary int/long order is
add,sub,mul,div,rem,and,or,xor,shl,shr,ushr; FP has only add,sub,mul,div,rem.
long/double operands and results are Two; the count of a long shift is still One; a cmp result
is always One; a conversion takes its two widths from its name, and /2addr reads the old A and B
before writing A instead of being treated as reading B only.

The remaining 035 reserved/unused values MUST NOT have their width guessed and MUST NOT be
treated as nop; optimized and quickened formats MUST NOT be mixed into the standard table.
const/high16 shifts left by 16 bits and wide/high16 by 48 bits; wide/16 and wide/32 sign-extend
before becoming 64 bits. The count of 35c/3rc is a **word count**, not an argument count; a wide
argument MUST occupy two adjacent words.

Payload lengths: packed=`4+2*n`, sparse=`2+4*n`, fill-array=`4+ceil(element_width*n/2)`.
Compute them with checked integers first; a payload MUST be 4-byte aligned, the array width
accepts only 1/2/4/8, and trailing padding of odd bytes is not an element. Switch keys MUST
increase and be unique per the specification, and MUST NOT be reordered to repair them; every
target is relative to the switch PC that references it.

Each registered family has one lowering classification: scalar/move/conversion
and comparison use §4; branches/switch/return use §6; heap/fill use §2.4 and §9;
call/allocation use §9; throw/catch/monitor use §10–§11. nop remains a raw site.
move-result/move-exception are consumed by the binding rules of §1.3/§2, not
printed as independent Java assignments before their producer succeeds. Known
newer raw-only families follow §2 and §8. A classification dispatches to those
rules; it does not authorize lowering without their type/effect/binding checks.
The implementation MUST NOT keep a second width/mnemonic table in the printer or
scanner; The analysis-oriented debug snapshot is defined in §7.3.

### Two-pass validation and CFG splitting

```text
pass1:
  each step first runs charge/check_cancel, then reads the complete format/payload with checked arithmetic
  record InsnStart/PayloadStart and RawInsn(original words, operands, refs, origin)
pass2:
  check register spans, index kinds, branch/payload targets and alignment
  check try/debug boundaries, handler entries, invoke word/proto and result/exception adjacency
  seal ValidatedRawMethod only after everything passes
```

Unreachable code is also validated for width and explicit references; the implementation MUST
NOT hide truncation with DCE. An explicit branch into the middle of an instruction is always
rejected; fallthrough into a payload or past the end of the code is a reachable-execution check,
and dead padding after a return MUST NOT be misjudged as an actual execution overrun. raw keeps
every decoded instruction, while the published FlowBody contains only the reachable part. The
delta of goto/goto16 and if-* MUST NOT be 0, and goto32 MAY be 0; a legal self-loop MUST NOT be
rejected wholesale. CFG leaders are the entry, jump targets, the point after a terminator, try
boundaries, handler entries, and the point after a protected may-throw operation. Every
protected may-throw is the last operation of its block, and normal and exception edges are kept
separate; the same target under different cases or ranks still yields different EdgeIds.

invoke/filled-new-array and the immediately following move-result are first bound into one
semantic atom, and only then is the CFG split along atoms, keeping both raw sites; the result
writes its target register on the normal path only, and discarding a result still keeps the call.
The implementation MUST reject a result kind/return kind mismatch, a branch into move-result, and
an illegal move-exception entry. The original protection domain covers only genuine may-throw
sites; Java operand evaluation and other calls MUST NOT be widened into the try.

### Deterministic rules for metadata decoding

- The low 5 bits of an encoded_value header are the tag and the high 3 bits are value_arg.
  byte/short/int/long and the corresponding reference indices are read at the allowed width and
  integers are sign-extended; float/double restore the original bits by right-padding with 0 and
  undergo no numeric conversion. The arg of array/annotation/null MUST be 0; the arg of boolean is
  0/1 with no following payload.
- annotation and array use an explicit bounded stack; each level first charges the element count
  and depth, and reference indices are checked against their own tables. Debug decoding reads
  line_start and parameter names with ulebp1; NO_INDEX does not become pool index 0.
- Debug opcode 0 ends, 1 is advance_pc, 2 is advance_line, 3/4 are start_local[/extended], 5 is
  end_local, 6 is restart_local, 7/8 are prologue/epilogue, 9 is set_file; >=0x0a updates by
  `adj=op-0x0a; pc+=adj/15; line+=-4+adj%15`. Addresses and lines are checked, and local lifetimes
  are valid.
- When metadata cannot be decoded, report the actual failure and missing range without
  discarding other checked code/declaration facts. Debug coverage MUST NOT claim that the
  missing information was decoded; header damage and method-local failures stay distinct.

### Derivation and execution order of range checks

For `n` records with stride `s`, the valid condition is `off<=N` and `n<=(N-off)/s` (s>0).
The implementation first computes the division-form upper bound or checked multiply-add and only
then takes the slice; it MUST NOT compute the possibly overflowing `off+n*s<=N` first. A record
count that satisfies the minimum byte bound is only a necessary condition: every record is then
checked for its variable-length tail and references. Reads do not allocate; when a Vec is needed,
the implementation checks the input facts, the semantic count and the resource quota first and
then calls try_reserve, and none of the three substitutes for another.

If the minimum byte count of an already read field directly proves an overrun of the remaining
bytes, record Malformed first; the implementation MUST NOT report BudgetExceeded instead merely
because the same count also exceeds the analysis limit. A cancellation or a request-admission
resource failure before that field has been read still returns its own reason, and parsing MUST
NOT be forced to continue for diagnosis.

```text
read_table(off, n, stride):
  reject stride==0 or off>N
  reject n>(N-off)/stride
  end=checked_add(off, checked_mul(n,stride))
  return bytes.get(off..end) or Malformed
```

This is a local memory-safety derivation; it does not prove that map or type semantics are
correct. 041 has three further intervals: physical `[0,N)`, logical span `[h,h+file_size)`, and
the later physical range that the current member may reference; the span is the header-walk
interval, not an upper bound for referenced data. For example, with h=0x100, file_size=0x80 and
N=0x500, data at 0x400 may be legal, and 0x80 is not forward data that this member may reference.
Section type, alignment and overlap rules are still checked separately, and another header MUST
NOT be treated as string or code merely because it lies inside the physical range. All addresses
keep their original values; the implementation MUST NOT add h or copy shared regions to correct
them.

## Application input, scanning and extraction

The following uses the determined native scheme; it MUST NOT carry over the old session/MCP or build
a general file framework. Safe file access uses the cap-std Dir and the ordinary-file safe
interface, cross-platform file identity uses same-file, and private temporary files use tempfile;
compression continues the reviewed bounded native inflate approach and CRC uses crc32fast. The
dependencies are locked by Cargo. Diana code uses no unsafe; the file, decompression and CRC calls
go through safe interfaces.

### InputLease and directories

1. Fix the allowed root and the Dir handle first, and open the explicit input by the
   capability-relative rule; devices and FIFOs are forbidden, Unix adds O_NONBLOCK/O_NOCTTY through
   the safe OpenOptionsExt so that opening a FIFO or terminal neither blocks nor changes the process
   terminal, and Windows rejects the device/pipe namespace. After obtaining the handle, re-check
   regular-file status, len and file identity.
2. A small input is copied into owned bytes and a large APK into a private snapshot file; read it in
   chunks by the entry file length and check cancellation and any explicit input quota. An early
   EOF, bytes beyond the entry length, or a change of the original handle's identity/len/mtime
   before or after all report INPUT_CHANGED and discard the snapshot; an unlimited quota also does
   not chase a continuously growing file. This promises immutability after the copy only, and does
   not claim
   that a stat check proves an atomic version under arbitrary concurrent writes of the original
   file; the analysis reads only the snapshot and never the original path again.
3. A large APK reads only the central directory and the entry ranges currently needed; each
   materialized entry has its own owned buffer, a stored entry borrows that buffer and a deflated
   entry borrows the decompression buffer, all held by the lease. There is no unsafe mmap, the
   implementation MUST NOT copy the whole APK into every DexImage, and this safe copy path MUST NOT
   be claimed to be zero-copy.
4. EOCD is searched in at most the last 65535+22 bytes, and the comment length MUST match the file
   end; it is single-disk, and the directory count/bytes are checked first against the actual
   directory range/minimum record size and any explicit limit. On the classic sentinel, verify the
   signatures, lengths, single-disk fields and 64-bit directory range of the ZIP64 locator/EOCD
   records before reading the directory; 0xffffffff MUST NOT be treated as a real allocation amount.
5. Each central record stores an EntryKey by its original ordinal, and the fixed 46 bytes plus the
   name/extra/comment lengths are checked. UTF8-flagged names are decoded strictly and others follow
   ZIP CP437; the original name bytes are kept without basename or normalization. Records with the
   same name are all kept, a non-unique name lookup MUST require the original entry index, and the
   manifest is selected by the original ordinal rule, which the worker completion order MUST NOT
   decide. The local header separately verifies the signature/flags/method and the data range, and
   the final CRC/size of a bit3 data descriptor is determined by the central directory and the
   actual decompression check.

### Lightweight definition and xref scanning

1. H03/H09 build no Engine/SSA. Accumulate the four class_data delta groups from class_defs and
   list **actual definitions**, not every field_ids/method_ids reference. Each row carries
   scope/physical/logical/class/member original keys.
2. String matching prefilters with ASCII/raw first; non-ASCII and exact xref MUST decode MUTF8 to
   u16 before comparing, and replacement text MUST NOT be used as lookup truth. Display escaping
   is separate from matching; only the content is filtered and all results are returned.
3. Instruction layout reuses the engine's stateless `raw::instruction_layout(code_bytes, pc_cu,
   version)`,
   returning the checked width and at most two `(RefKind,index,operand_slot)` and initializing no
   ProgramIndex.
   That function is implemented by the same format table as T03; the engine's internal full decode
   uses it too, and the width table MUST NOT be duplicated. Metadata/index are still checked by the
   current scanner against its own logical tables. An unknown width makes this scan fail explicitly,
   and the implementation MUST NOT guess the next address, skip a site and then report success.
4. Every actual direct ref operand generates a site whose identity contains
   MethodKey+code_unit+operand_slot; it MUST NOT be expanded into transitive references of the
   signature or deduplicated per method. The same numeric index in different logical images MUST NOT
   be merged.
5. The worker count follows [01-input](01-input.md), with bounded byte/row queues and weighted admission; results
   are published on completion with no global sort or ordinal waiting. Closing the consumer stops
   the producer; a mid-run failure fails explicitly, and an emitted prefix MUST NOT be passed off as
   complete success. There is no pagination, preview or successful truncation.

### Extraction and publication of the original entry

1. Select the EntryKey exactly and first check the local/central header, flags/method consistency
   and the data range. After the structure is valid, reject any encrypted entry first
   (EncryptedEntry, accepting no password or decryption path), and then reject a compression method
   other than 0/8 (UnsupportedCompression, not Malformed for a format-valid unsupported method).
   Both decisions happen before decoding, before the plaintext size/CRC check and before creating
   output; the compressed_size of an encrypted stored entry includes the encryption header, so the
   unencrypted equal-length rule MUST NOT be applied early. Only unencrypted method 0 requires equal
   compressed/uncompressed lengths; method 8 uses bounded inflate within the declared length and any
   explicit limit, checks the actual length and end state, and traverses CRC32 against the central
   CRC. The original payload does not pass through UTF8/JSON, DEX normalization or header rewriting.
2. Output uses only an explicit -o and MUST NOT concatenate the entry name. Open the output parent
   Dir once and hold it; reject a directory or symlink target and a target with the same
   handle/hardlink identity as the input. All temporary and final operations are relative to that
   Dir, and the mutable parent path is not resolved again.
3. In that Dir, create a mode 0600 temporary file with create_new, with a fixed upper bound on
   collision retries; publish only after the write/flush and the CRC/size/budget checks complete.
   Writes and copies check the public request cancellation and budget in chunks of at most 64KiB,
   and one large unchecked write or copy MUST NOT bypass cooperative cancellation. On error or
   cancellation only the own temp is removed, never the user target.
4. no-force publishes atomically with **hard_link(temp, final)** in the same Dir, fails when any
   final item exists, and then deletes the temp. At most one of two publishers succeeds; the
   exists-then-rename sequence MUST NOT be used. When the file system does not support that
   primitive, the operation fails explicitly and MUST NOT degrade to an implementation with an
   overwrite race; the platform evidence for D070 is still not-run.
5. force allows only an ordinary target that is not an input alias at entry, then performs an atomic
   rename relative to the held Dir to replace the directory entry and does not follow a final
   symlink to write elsewhere. It is not a CAS interface that “replaces only the previous inode”,
   and it
   makes no promise that an external process cannot rename after the check; the fixed input path
   itself is forbidden as output, and the bytes of the source handle MUST NOT be written. A platform
   that cannot provide these safe primitives gets no weakened implementation. Atomic visibility MUST
   NOT be passed off as power-failure durability.
6. After the output file succeeds, stdout carries no binary content or status line; the text/JSON of
   query/get is kept separate from the binary output adapter. BrokenPipe or an I/O error stops the
   request, the erroneous target is unchanged before file publication, and no half file remains. By
   default get generates the complete bounded Java/Mixed/Debug/Unavailable presentation per §8.1 and
   then writes it, does not let JavaUnit eligibility block Mixed, and does not equate a
   deliberately marked partial-failure result with a writer truncation.

Domain errors keep distinguishable reasons: AmbiguousEntry, TargetExists, UnsafeTarget,
CrcMismatch, BudgetExceeded, Cancelled, UnsupportedCompression, EncryptedEntry and I/O. The
snake_case names in cases are the semantic projection of these public reasons, and the
implementation MUST NOT guess the stage from stderr or replace a specified CRC/budget/alias
rejection with an arbitrary error.

The H02/H10 core file-safety tests continue through the public domain interfaces and the established
self-made directories; the dependencies, path wrappers and command parsing above get no separate
mechanical or auxiliary tests. Original permissions/provenance, cross-platform not-run results and
release cost are recorded honestly.

### Manifest decoding and AXML safety

Selection uses [10-cli](10-cli.md)'s original physical-entry rule and the same checked
InputLease/inflate/CRC as extraction. ResChunk_header, ResStringPool and
ResXMLTree field layouts follow the AOSP source identified in SOURCE_MAP §7. A bare DEX or a JAR-only input is an input
error; a structurally usable APK/ZIP with no AndroidManifest.xml is no-result.
The manifest Module decodes bytes only after entry integrity passes. It shares
request cancellation and explicit allocation/work/output limits; no manifest
size or nesting quota is invented by default.

| Binary structure | Checked read and semantic rule |
|---|---|
| Document/chunk | Outer type `0x0003`; little-endian `type:u16, header_size:u16, size:u32`; `8 <= header_size <= size`, four-byte chunk alignment, checked end inside the entry, and positive advancement ending exactly at the document extent |
| String pool `0x0001` | Header at least 28 bytes; checked string/style counts and offset arrays, stringsStart/stylesStart relative to this chunk; entries and terminators remain in their own data regions |
| UTF8 / UTF16 pool entry | UTF8 stores UTF16-unit and byte lengths using one/two-byte lengths; UTF16 stores one/two-u16 lengths. Verify both actual length and terminator; this is AXML string encoding, not DEX MUTF8 |
| Style span | Checked name index and UTF16 interval, terminated within style data; styles never justify reading past the pool or altering the XML text |
| Resource map `0x0180` | Optional, before nodes; header and payload length checked, payload divisible by 4, entries associate resource IDs with valid string-pool positions |
| XML node `0x0100..0x0104` | Header at least 16 bytes including line/comment; extensions start at header_size, and their complete fixed record fits before reading any reference |
| Namespace start/end | Checked prefix/URI refs; balanced scope with matching ends. Namespace aliases are scoped, not global substitutions |
| Start element | Checked namespace/name, extension at least 20 bytes; attributeStart is relative to the extension and at least 20, attributeSize at least 20, and the complete count*stride fits this chunk; id/class/style indices are 0 or 1..attributeCount |
| Attribute / typed value | Checked namespace/name/rawValue refs and 8-byte Res_value size/type/data; a missing optional ref is `0xffffffff`, not pool index 0 |
| End element / CDATA | Matching expanded element name on an explicit stack; checked text/typedData; no second root, unmatched close or character data outside the root |

String refs are checked before conversion or lookup. Rendering preserves namespace
meaning, attribute values and node order; it does not sort to make output stable.
Raw string values are preferred when present; otherwise typed values use Android
resource/attribute IDs, booleans, signed/hex integers, FP, packed dimension/fraction
and color formatting from the reviewed reader. Unsupported typed values remain
explicit raw-type/data text, not a guessed resource lookup. The decoder does not
load resources, resolve a network namespace or invoke Android tools.

The renderer escapes XML metacharacters and checks XML character/name legality;
illegal characters are errors, not replacement characters. Plain UTF8 XML entries
require an actual well-formed, single-root check; seeing a leading `<` does not
prove validity. External entity/DTD expansion is unsupported and never accesses
files or the network. Output is complete UTF8 XML or a classified error; line
numbers, byte layout and quote style are not source-byte identity commitments.

Parsing, rendering and release use explicit stacks/arenas and checked growth.
A reused decoder MUST expose the required cancellation/allocation points and
return errors for every reachable malformed read; a skeleton precheck,
`catch_unwind` or the existing PATCHES cannot prove that contract. A known but
unhandled legal chunk/profile is Unsupported; a broken size/ref/stack/string is
Malformed; cancellation/resources retain their own kinds. Extraction remains the
way to retrieve bytes that the XML presentation cannot express.

## 7. Application limits and lifecycle

Every numeric quota MUST be configured explicitly by the caller and is unset by default; the application MUST NOT restore an engine default limit that was cancelled by using its own fixed threshold.

| Layer | Contract |
|---|---|
| File/ZIP | input, directory count/bytes and inflated bytes are unlimited by default; Diana MUST still check the actual file range, the minimum directory record size, the end state of the compressed stream, and the actual size and CRC |
| CLI request/decompression | an explicit quota covers held bytes, decompression, dependencies and scratch; cancellation and bounded-queue backpressure are always retained, and only the bytes this request needs are reused |
| Engine | Limits are all None by default; the application passes only explicit configuration and MUST NOT inject numeric initial values for input/work/graph/output |
| CLI output | a query may stream, while a decompilation report MUST be rendered completely before it is committed; text/JSON MUST be checked against their actual bytes separately, and Diana MUST NOT truncate an explicit overrun into a success |

The application JSON output limit is optional configuration as well and defaults to None, with no fixed threshold such as 64MiB. The application MUST check a decompilation JSON against
the corresponding item only when the application limit or `Limits.max_report_output_bytes` is set explicitly, and the smaller value MUST apply only when both are set;
a lightweight query JSON MUST use only the explicit application limit. The application MUST check the configured allowance segment by segment against the final JSON bytes, and it MUST count repeated source, escapes,
diagnostics and spans. The application MUST check text/JSON against their own length separately and MUST NOT add the two representations together; actual work/
allocation MUST still be charged to the same request, and Diana MUST NOT claim that the cumulative ledger is RSS.

A query MUST use bounded admission and output queues and MUST NOT wait for display
order. Without `--jobs`, the worker count is min(8, available_parallelism), or 1
when the CPU count cannot be obtained. Explicit `--jobs N` selects N workers for
independent-image scanning. N is positive and platform-representable; the
application starts at most N workers and no more than the independent physical
inputs available. Failure to create a needed worker is a resource failure, not
silent clamping. A worker scans the 041 logical members of its admitted physical
image under that image's lease; the pool is not recreated per member or method.
Decompilation runs sequentially on the calling thread and has no `--jobs` option.

Admission holds at most one input per worker, and each worker has at most one
pending complete output row; it waits for that row to be consumed before queuing
the next. The sink consumes whichever worker is ready. The weight is
the actual retained input, decompression buffer and queued row/string capacity,
charged before growth against explicitly configured request limits. A capacity
estimate from a ZIP header is not permission to allocate unchecked. One row or
image larger than a transport chunk is processed with checked growth, not rejected
by a hidden default quota. Backpressure bounds outstanding rows, not the bytes
of an arbitrarily large single row or a retained complete JSON result.

Closing the consumer stops admission and wakes blocked workers; cancellation or
a hard producer failure stops the request and releases pending owners. Completed
streaming records may already be visible, but that prefix is not a success.
Native inflate is not preemptible; incremental calls and queue waits cooperate
with cancellation. See [01-input](01-input.md) for input and publication rules.
