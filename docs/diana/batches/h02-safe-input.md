# H02 safe input and the CLI request scope

Baseline: f95000d. Contracts: DESIGN §6.1/§7 and TASKS H02. Preparation reuses
the accepted macOS arm64 B01–B04 inputs; no reference source is copied, and the
new fixtures are byte vectors built by the test itself.

## Goal and products

The application now owns an immutable input snapshot. `analysis/input.rs`
opens one regular file after applying the path and size policy, copies it fully
under explicit bounds, and exposes `InputLease`: the request-local identity
(path and original length), the owned bytes and optional restricted entry
metadata. `BytesSource` lets a consumer borrow either the lease or an entry view
without letting a borrow escape its owner. `analysis/archive.rs` reads the ZIP
end-of-central-directory and central-directory records, slices stored payloads
and inflates deflate payloads on demand; `analysis/apk.rs` selects root DEX
images in physical directory order, and `analysis/request.rs` holds one request's
lease and hands out only borrowed roots inside that lifetime.

`unsafe_code = "forbid"` stays in force, so the snapshot uses no mmap and no
`'static` extension. Non-regular files are rejected from metadata before any
open; a FIFO therefore cannot block a request. A configured `allowed_root`
compares the resolved path against the resolved root and rejects an escape;
without it the platform's ordinary resolution policy applies to regular files.
A read that ends early or a length that changes between the metadata checks is
`Changed`, never a short snapshot. Budgets (`max_input_bytes`, `max_entries`,
`max_total_entry_bytes`) are unset by default and checked before the read or
before the directory is trusted. Unknown compression methods, encryption, a
damaged payload or CRC, a truncated deflate stream, an exceeded inflation bound,
ZIP64 fields and inconsistent records are distinct classified reasons rather
than empty results. Deflate uses the reviewed `libdeflater` dependency already
declared by the scaffold; the output buffer is sized from the declared size
after the bound check, the stream must fill it exactly and the CRC is verified
over the actual inflated bytes.

Local failures keep their own reason: an unreadable archive is
`MalformedArchive`, a lease of an unrecognized file is still a valid lease whose
roots fail as `NotDex`, and an empty file is opaque rather than guessed to be a
damaged archive. Nothing is cached across requests, so a second request observes
the file's current content, and dropping the lease never deletes or modifies the
user's input. H02 creates no temporary file; the atomic output owner belongs to
H10.

## Evidence

`cargo test -p diana --test product_contracts h02_` runs eight public cases:

- Ordinary file, bare DEX and stored archive leases, including entry metadata
  (`name`, method, sizes, CRC), `EntryView` payloads, root selection order and
  the root-name rule (`classes.dex`, `classesN.dex` for N >= 2, not
  `classes1.dex` or a nested path). The standard CRC-32 vector
  `"123456789" -> 0xcbf43926` is asserted independently of our writer.
- Snapshot isolation: truncating and rewriting the source after the lease does
  not change the leased bytes; after the lease is dropped the file still exists
  and a new lease observes the new content.
- Path policy: an allowed root admits an inside file and rejects a symlink that
  resolves outside it, a directory, a character device and a FIFO; the FIFO open
  is probed from another thread with a five-second bound, so a blocking open
  fails the case instead of hanging it.
- Budgets: input bytes one below the actual length, an entry count one below the
  record count and a total-entry-byte bound one below the declared total are each
  rejected with the specific limit; the equal boundary is admitted.
- Entry damage: a flipped stored payload byte is `DamagedEntry`, method 12 is
  `UnsupportedCompression(12)`, and the encrypted flag is `EncryptedEntry`.
- Deflate: a repetitive self-made payload compresses smaller, inflates exactly
  through `read_entry` and through root selection, admits the exact
  `max_inflated_bytes` boundary, rejects one byte below it as
  `InflatedBytesExceeded`, and classifies a flipped stream byte as
  `DecompressionFailed`/`DamagedEntry`.
- Truncated archives at three cut points never publish a partial directory; an
  empty file is opaque.
- Request scope: opening a scope yields borrowed roots, dropping it leaves the
  input file byte-identical, a later scope observes replaced content instead of a
  cached snapshot, and the file can be removed once no scope exists.

A unit test in `input.rs` drives the snapshot reader with a short source and
expects `Changed`; the complete-length control succeeds.

Snapshot copy cost was measured once on a release build with a temporary
`examples/lease_cost.rs` driver (deleted afterwards) reading a 64 MiB self-made
DEX-prefixed file: best of five `open_input` calls was 0.004455 seconds for
67,108,872 bytes, i.e. a cold-cache-carrying copy on this host. This is a
single-machine measurement of the copy, not a claim of byte-identical
performance against a mapped or lazily read input, and it defines no threshold.

## Gates

- `cargo fmt --all -- --check`, strict workspace Clippy and the locked release
  build pass.
- `cargo nextest run --workspace --all-features --locked` passes 201 tests with
  zero skips, including the new `h02_` cases and every engine regression.
- The engine, its fixtures and the E2E specification are untouched by this
  batch, so no E2E count changes; `python3 tools/e2e.py run` is not re-run here.

## Remaining obligations

The request/CLI surface, `extract` publication with atomic output, the AXML
manifest reader and query commands stay with H05/H03/H10. `request.rs` was not created because no
consumer exists yet; H05 introduces the request scope that owns this lease. H01's
eleven-command contract and Q01 review remain pending.
