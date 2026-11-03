# Working on the Diana application

The root `AGENTS.md` and `docs/diana/CONTRIBUTING.md` apply. Read the relevant
DESIGN and ALGORITHMS sections before changing application behavior.

- This package owns file/input snapshots, ZIP/APK handling, query and result
  adaptation, CLI routing and atomic output publication. The engine remains the
  independent library in `crates/diana-decomp/`.
- Use `clap` major version 4 with `derive` for argument parsing and `libdeflater`
  major version 1 for ZIP DEFLATE. Obtain operator approval before introducing an
  alternative parser or DEFLATE backend. The bundled native C dependency of
  libdeflater is approved; active Rust code forbids unsafe.
- Do not delegate to or depend on rasc, dexdec or rusty-dex. Reuse historical
  input/ZIP ideas only after focused review with compatibility and security evidence.
- Exercise complex input, query, extraction and safety contracts through public
  modules. Core fixtures and independent expectations live with the Rust tests.
- Build from the repository root with `cargo build --locked -p diana`.
