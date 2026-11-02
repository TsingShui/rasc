# H05 decompilation display and locator wiring

Baseline: c642263. Contracts: DESIGN §6.5 (default decompilation display, JSON
schema 1), §6.8; TASKS H05. `ALGORITHMS.md` is unchanged.

## Products

`analysis/decompile.rs` is the single engine adapter: it resolves the locator to an
original selection, builds the borrowed image list (including 041 logical members),
opens the engine with the caller's `DeclarationCatalog`, runs exactly one
`decompile` request and renders the owned report. The report and the rendered text
are owned; no borrow of the leased input escapes the call. The selection is mapped
back to the canonical `at` of both the class and the method.

`diana get-class <input> <selector>` and `diana get-method <input> <selector>`
accept the canonical locators, `Lowner;`, dotted class names and the complete
`owner->name(proto)` method reference. A method reference without the prototype is
a usage error (exit 2), and a short reference is resolved only when exactly one
defined member matches its name and prototype.

Text output is the rendered report: a complete unit stays Java, a Mixed class keeps
the shell/fields/Java members and the original-fact debug or reason of each failing
member, and a method request prints exactly the selected member's slice. JSON output
follows schema 1 with `selection`, `status`, `source`, `methods`, `diagnostics` and
`spans`; a method record carries its `at`, status, emission, language, source,
`source_range`, and the nested reason diagnostic, and every span range addresses the
delivered `source`.

## Limitation recorded

The CLI currently opens the engine with an empty declaration catalog: the
application-side catalog assembly from the request's own declarations belongs to the
dependency-declaration work in H04 step 4 and is not implemented. Members whose
Java binding needs an external declaration therefore keep their classified
fallback instead of becoming a guessed binding. The library entry
`run_with_catalog` takes an explicit catalog so H05's semantic evidence can be
produced now.

## Evidence

`cargo test -p diana --test product_contracts h05_` runs three public cases:

- With the declaration facts of the fixture, a class request recovers the Java
  constructor; its rendered span slices exactly the member source text.
- A class with an `invoke-custom` member stays Mixed: all three original members are
  present, the constructor remains Java, the dynamic member keeps
  `InvokeCustomUnsupported` as its reason, and a method request returns only that
  member with the same reason and an `@cu:`-addressed debug slice containing
  `invoke-custom`.
- A method reference without a prototype is rejected with exit 2; a complete
  reference selects the expected `m1` locator even when its member cannot be
  expressed as Java.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 225 tests with zero
skips.

## Remaining work

CLI catalog assembly, the full F05 Driver loop through the CLI and the application
default-text seam (D085 `application-default-text`) belong to the following H
batches; this batch wires the semantic loop and the presentation contract.
