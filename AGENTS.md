# Contributing to rasc

`rasc` is a native Rust CLI for analyzing APK and DEX files. It is a Rust implementation of ASC, with a focus on fast analysis and predictable command-line behavior. This file is the repository-wide guide for contributors and coding agents.

## Project layout

- `src/`: CLI, APK handling, and native DEX parsing and analysis.
- `crates/dexdec/`: Java-like source decompiler used by `getclass`.
- `crates/rusty-dex/`: DEX parser and instruction model.
- `vendor/axmldecoder/`: patched vendored dependency; see its `PATCHES.md` before changing it.
- `tests/`: integration tests.
- `skill/SKILL.md`: skill text embedded in `rasc skill` output.
- `docs/`: design notes and release notes.

The root Cargo workspace contains the CLI and the two crates above; `vendor/axmldecoder` is deliberately excluded.

## Development setup

Use the Rust toolchain required by `Cargo.toml` (`rust-version`). Build and test from the repository root. The dependency lockfile is `Cargo.lock`; include intentional lockfile changes with dependency updates.

Useful commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --release
```

Run the narrowest relevant test while iterating, then run the applicable checks above before submitting. Tests that need a real APK must be explicitly opted into and supplied the documented environment variable; APK samples are not committed to the repository. Never treat a skipped or missing-fixture test as a pass for real-APK behavior.

## Engineering constraints

- Treat APK and DEX input as untrusted. Validate lengths and offsets before slicing or allocating; malformed input should return a useful error, not panic, hang, or trigger an unbounded allocation.
- Preserve existing CLI behavior unless a change intentionally revises it. This includes command names and arguments, output bytes and ordering, and useful diagnostic text. Update or add tests when changing a contract.
- Keep parsing and analysis native and independent of external runtimes.
- Preserve deterministic output when adding parallelism. Measure performance changes using a release build and representative inputs; do not infer performance from debug builds.
- `skill/SKILL.md` is part of the observable CLI output. Keep its usage examples accurate and test changes to it.
- Keep changes focused. Avoid unrelated formatting or generated-file churn. Explain and test any dependency or vendored-code change; consult upstream and existing fork/patch notes first.

## Tests and fixtures

Prefer small, self-contained fixtures that isolate the behavior under test. For binary formats, include malformed and boundary cases as well as valid inputs. Real APK tests are opt-in: follow the environment-variable instructions in the relevant test or source comments, and do not add proprietary or otherwise unauthorized APKs to the repository.

## Corpus data and sensitive inputs

Do not commit private or unauthorized APKs, real-corpus identifiers, or mappings that reveal corpus identity. Use small self-made fixtures for tests. Keep local corpus files and alias mappings under the untracked `.cache/` directory; consult `tools/audit/desensitize_words.txt` and run `tools/audit/desensitize_check.py --range HEAD` when changes may contain corpus references. Publicly documented sample names are acceptable only when the source is genuinely public and the use is authorized.

## Contribution process

1. Check existing code, tests, and relevant design or fork notes before changing behavior.
2. Add or update tests for bug fixes and features. Document any intentional CLI or output-contract change.
3. Run formatting, relevant tests, and the applicable checks listed above. Report commands that could not be run and why.
4. Submit a focused pull request describing the problem, the approach, compatibility impact, and validation performed. Include before/after measurements for performance claims.
5. Use Conventional Commit messages, for example `fix: handle truncated DEX header`, `feat: add type reference search`, or `docs: clarify installation`. These messages drive automated versioning through Release Please. Use `!` or a `BREAKING CHANGE:` footer for breaking changes.

## Releases

Release Please opens a release PR and updates the package version and changelog based on Conventional Commits. Do not manually bump the version or create a release tag as part of an ordinary feature or fix; put release-relevant changes in normal commits and let the release PR update version files. Merging that PR creates the version tag and triggers the release workflow.
