# AGENT.md — Working in this repository

rasc is the Rust rewrite of ASC (an APK/DEX analysis tool). The repository ships one product: a
native Rust CLI.

## Gates

Run the native gates before considering a change complete:

| Command | Proves |
|---|---|
| `cargo test` | unit and integration tests pass |
| `cargo build --release` | the native release binary builds |
| `cargo test --workspace --all-features` | all workspace features compile and the native tests pass |

Sample archives do not travel with the repository. Tests that need an APK accept
`APK=/path/to/app.apk`.

## Invariants

1. Native output is the baseline. No change may alter native byte output unless the change explicitly
   requires it.
2. Error text is a CLI contract. Keep diagnostics stable when changing parser or decoder code.
3. Parsing code works on byte slices and should not depend on external runtimes.
4. `skill/SKILL.md` is embedded in the `rasc skill` output; edits to it change CLI output bytes.

## Traps

- Confirm a release build succeeded before trusting its measurements.
- A malformed ZIP must produce a useful error rather than an unbounded allocation.
- Contract harnesses must verify that their input control case really parsed and that comparisons ran.

## Layout

```
src/            native CLI and APK/DEX parsing
Cargo.lock      locked native dependency graph
vendor/         patched crates.io dependencies
crates/         workspace members used by `getclass`
tools/audit/    desensitization gate
docs/           release notes
skill/          embedded Agent Skill payload
tests/          self-contained integration tests
```
