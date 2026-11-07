# Working on the Diana GUI

The root `AGENTS.md` and `docs/diana/CONTRIBUTING.md` apply.

- This package is a native GPUI Kit application. Keep authored demo content visibly
  distinct from actual Engine results. Additional input integration requires a
  separate user request.
- Prefer composing GPUI Kit's existing styled controls and interaction behavior
  over custom widgets or painting. On macOS, use the system title bar and native
  application menus instead of an in-window toolbar. Other desktop platforms
  retain Kit's toolbar. Keep one shared dark theme, a fixed tree column and no
  sidebar member section. Follow the Zed/Ayu
  reference for color roles and compact density through Kit's theme and sizing
  interfaces, not a second set of widget implementations.
  Use bundled icons sparingly for conventional affordances; do not replace
  standard icon controls with forced text labels. Use concise, conventional UI
  copy rather than forced translations.
- Start with an empty welcome view and a concise APK/DEX-opening hint. Do not
  populate a project tree or documents automatically. Keep authored demo content
  behind an explicit demo command; a static hint does not imply file integration.
- Read `DESIGN.md` before changing resource, identity, navigation, view or result
  interfaces. Its host-level contracts do not widen Engine stage access or
  authorize backend integration exceptions.
- Keep state changes in event handlers; rendering consumes existing state. Use
  stable class identities for open documents, not current list positions.
- Keep readonly code views selectable and copyable. Highlighting is presentation,
  never a code-legality or result-availability gate.
- Preserve native keyboard behavior, meaningful control labels and focus targets.
- Production Rust forbids unsafe code. Do not add tests, fixtures or CI test jobs.
- Check this package with `cargo clippy -p diana-gui --all-targets --all-features
  --locked -- -D warnings` and `cargo build -p diana-gui --locked`.
