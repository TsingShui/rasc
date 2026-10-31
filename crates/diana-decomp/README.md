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
checks whole-method register/reference bounds, branch/payload/exception boundaries
and reachable fallthrough. Its sealed `CheckedMethodLayout` keeps structural
checks distinct from complete raw validation. `RawMethodView::visit_layouts`
preserves readable prefixes under the independent fallback budget.
`RawMethodView::visit_instructions` exposes borrowed original operand/payload
facts, including literal bits and ordered invoke words, through that same budget.
The validator shares its operand decoder. `ProgramIndex::debug_method` produces
owned original-information JSON with resolved references, UTF16 units, handler
order and explicit missing facts. Its reader and output use the same fallback
ledger; known malformed input remains classified even if output later exhausts
resources. Core tests exercise malformed input, UTF16 integrity, resource limits and cancellation.
The Engine interface awaits integration.

From the repository root:

```sh
cargo build --locked -p diana-decomp
cargo clippy --locked -p diana-decomp --all-targets -- -D warnings
cargo nextest run --locked -p diana-decomp --test contracts
```

Dependency choices are recorded in the root and package AGENTS.md files; the
manifest and root lockfile own versions. The engine remains independent of rasc,
dexdec, rusty-dex, Kuna and radguard implementations. Inputs are borrowed
DEX bytes and declaration facts provided by `apps/diana/`; file/ZIP loading and
CLI dispatch do not belong here. Production code stays native Rust, with unsafe
code forbidden and no external runtime/process dependency.

Only create modules when an implemented capability needs them. Do not pre-create
P0–P9 directories, fake Engine results, pass registries or tests for placeholder
getters. See [interfaces / data](../../docs/diana/DESIGN.md),
[algorithms](../../docs/diana/ALGORITHMS.md) and
[preparation / tasks](../../docs/diana/TASKS.md).

The `0.0.0` version is an unpublished scaffold marker. rasc and its release
tooling are isolated under `ref-fork/rasc/`, outside the active workspace/CI. Shared test inputs/oracles live in root
`tests/e2e/`; crate tests cross public interfaces for actual core capabilities.
