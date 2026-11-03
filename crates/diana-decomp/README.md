# diana-decomp — native engine library

This is Diana's internal Rust decompiler library, imported as `diana_decomp`.
`RunContext` implements cumulative request work/allocation charging and latched
cancellation. Method budgets isolate normal/raw work and reserve only explicitly
finite fallback allowances. Resource errors expose typed failure kinds and codes.
The public `input` module implements checked DEX 035 reads and borrowed class,
method, MUTF-8 and code-range views. Method visits preserve original IDs and local
declaration/code failures; complete prototype matching distinguishes overloads.
Byte/UTF16/code-unit coordinates and table IDs use distinct types. Method-body
states are explicit, and range-checked code views are read-only.
`raw::instruction_layout` checks instruction/payload extents and preserves typed
operand references using one opcode specification. `ProgramIndex::check_method_layout`
checks whole-method register/reference bounds, branch/payload/exception boundaries,
035 invoke prototype word counts, wide argument adjacency, result kinds and
reachable fallthrough. Its sealed `CheckedMethodLayout` keeps structural
checks distinct from complete raw validation. `RawMethodView::visit_layouts`
preserves readable prefixes under the independent fallback budget.
`RawMethodView::visit_instructions` exposes borrowed original operand/payload
facts, including literal bits and ordered invoke words, through that same budget.
The validator shares its operand decoder. `ProgramIndex::debug_method` produces
owned original-information JSON with resolved references, UTF16 units, handler
order and explicit missing facts. Its reader and output use the same fallback
ledger; known malformed input remains classified even if output later exhausts
resources. Original debug programs retain parameter names, line/PC state, ordered
local-variable events and encoded bytes. Their reader checks instruction
boundaries, reference indices and local lifetimes; a failure retains the decoded
prefix with partial coverage. Static encoded values retain original tags, signed
integers, float/double bits, references and nested array/annotation order. Their
explicit stack uses checked counts and fallback accounting; malformed tails
retain their checked prefix. Core tests exercise malformed input, UTF16
integrity, resource limits and cancellation.
Engine exposes class/method requests and owned reports through the public library.
Ordinary code uses the shared CFG → SSA → declaration/type → storage → region →
Java chain. Unit verification distinguishes complete Java from Mixed results;
independent members and original metadata remain available after local failure.
P1 checks dynamic call-site/handle encodings and original references; invoke-custom
retains a classified unsupported exit and a readable original snapshot. P8 renders
existing products and maps source slices to original identities. Historical acceptance records are available in git history.

From the repository root:

```sh
cargo build --locked -p diana-decomp
cargo clippy --locked -p diana-decomp --all-targets -- -D warnings
cargo nextest run --locked -p diana-decomp --test contracts
```

Dependency choices are recorded in the root and package AGENTS.md files; the
manifest and root lockfile own versions. The engine remains independent of rasc,
dexdec, rusty-dex, Kuna and radguard implementations. Inputs are borrowed
DEX bytes; the engine derives declaration facts from them and the application
supplies no external declarations. File/ZIP loading and CLI dispatch do not
belong here. Production code stays native Rust, with unsafe
code forbidden and no external runtime/process dependency.

Only create modules when an implemented capability needs them. Do not pre-create
P1–P8 directories, fake Engine results, pass registries or tests for placeholder
getters. See the [specification chapters](../../docs/diana/spec/00-overview.md).

The `0.0.0` version is an unpublished scaffold marker. rasc and its release
tooling were removed at commit 5cb156c and remain in git history only. Crate tests
cross public interfaces and keep their self-made fixtures alongside the tests.
