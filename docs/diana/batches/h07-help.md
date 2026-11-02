# H07 command help and agent documentation

Baseline: 0ab05b4. Contracts: DESIGN §6 (command table, `at`, output and exit
codes); TASKS H07. `ALGORITHMS.md` is unchanged.

## Products

The binary help now lists the ten product commands plus `help`, each with the
description and the column or result shape it produces, and each argument and
option carries its own explanation. Global limit options are documented where they
appear. `--version` prints the package marker, and an unknown command, flag or alias
is rejected with exit 2 because clap accepts nothing that is not declared.

`apps/diana/README.md` is the agent-facing document: it lists the eleven entries,
per-command columns, the `at` grammar, the selector rules, text/JSON shapes, exit
codes, the `extract` exception, the explicit-limit policy and a short usage sequence
for an agent. It states that this text is ordinary documentation, that no
skill-install command exists, and that a user's installed rasc skill must not be
moved or deleted. The external `d8` conversion is named as the one conversion an
agent may reuse; Diana does not perform it and does not run recovered code.

## Q07 review

Checked by reading the source, the help output and the documents; no CLI black-box
or golden test was added.

- Command set and descriptions match DESIGN §6.1; no twelfth command and no old
  alias exists.
- Columns match DESIGN §6.4/§6.6; `xref` has exactly the four documented columns.
- `at` examples and the bare-DEX `e0:l0` mapping match §6.3; the same locator can be
  passed back to `get-class`, `get-method` and `xref`.
- Text escaping, JSON table shape and the decompilation/manifest report schemas
  match §6.2/§6.5/§6.7; `extract` writes only its file and never stdout.
- Exit codes 0/1/2/3/4 match §6.8, including the partial-document rules and the
  `--strict` behaviour.
- A limited Windows-only branch exists for hardlink detection; it is compiled on
  other platforms but not executed here, which is recorded as not-run.

## Evidence

`diana --help`, `diana --version` and each `diana <command> --help` were reviewed by
hand; the help lists the ten product commands plus `help`. `apps/diana/README.md`
documents the same contract.

Gates: formatting, strict workspace Clippy and the locked release build pass;
`cargo nextest run --workspace --all-features --locked` passes 234 tests with zero
skips.

## Remaining work

H08 rolls up every required product invocation; the Windows binary target and the
application-domain seams remain not-run here.
