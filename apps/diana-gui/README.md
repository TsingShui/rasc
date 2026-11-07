# Diana GUI

Native GPUI Kit 0.9.0 UI demo. Project names, Java-like text, DEX instructions
and diagnostics are authored display content, not Engine results. The application
does not read APK/DEX files or run analysis.

## Run

From the repository root with Rust 1.93+ and the native tools described in
[GPUI Kit installation](https://gpui-kit.com/docs/installation/):

```sh
cargo run -p diana-gui --release --locked
```

## UI composition

The application uses GPUI Kit controls with a shared Ayu Dark color mapping.
Its compact tree density, neutral selection and flat document tabs take Zed's
reading-first hierarchy as a reference; this is Diana's demo, not Zed's UI crate.
Kit still owns control rendering, focus, selection and keyboard interaction:

| Area | UI APIs |
| --- | --- |
| macOS window chrome and commands | System title bar, native `Menu` / `MenuItem` |
| Other desktop toolbar | Kit `TitleBar`, ghost `Button` |
| Fixed file-tree column | `Tree`, `TreeState`, `ListItem`, bundled `Icon` |
| Document tabs | Plain `TabBar`, `Tab`, close-icon `Button` |
| View menu outside macOS | `DropdownMenu`, `PopupMenu` |
| Symbol search | `Dialog`, searchable `List`, `ListItem` |
| Readonly code and in-document find | `Editor` and its built-in search panel |
| Original-fact inspector | `Sheet`, `DescriptionList`, `Alert` |
| No open documents | `Empty` |

On macOS, the window has no drawn toolbar or Search/Copy/Facts/View button row.
Commands live in the screen's native application menu bar. Other desktop
platforms retain the in-window Kit toolbar.

Application-owned code supplies the demo hierarchy, row content, document state
and action handlers. Basic flex layout fixes the tree column's width. Kit's
`Sidebar` is a navigation-menu container, not an arbitrary Tree host; the GUI
does not add an adapter just to place a Tree inside it.

## Interaction

- Select one of eight classes in the APK → DEX → package → class tree. Kit owns
  expansion, selection, scrolling and arrow-key navigation. Tree selection
  opens the selected class without stealing keyboard focus.
- Switch or close tabs. Each document retains separate Java-like and DEX editor
  state; the views are independent illustrations, not corresponding results.
- Navigate → Go to Symbol opens the picker on macOS; other platforms use Search.
  Cmd-K / Ctrl-K opens it from the workspace.
  Kit handles query input, result navigation, confirmation and dismissal.
- Cmd-F / Ctrl-F opens the active editor's native find panel.
- On macOS, Edit provides native text-editing actions plus Copy Document
  (Cmd-Shift-C). View → Original Facts opens the class inspector. Other platforms
  retain the Copy and Facts buttons; Ctrl-Shift-C copies the document.
- The inspector names its captured class; its diagnostic action returns to that
  class and call site.
- View selects the content mode, changes code size or restores the initial layout.
  The native menu's checks and document-dependent availability follow workspace state.
- Cmd-W / Ctrl-W closes the active tab; Ctrl-Tab selects the next tab.
- Closing the last window or using Cmd-Q / Ctrl-Q exits the application.

Syntect supplies display highlighting, never a legality or availability gate.
The reader preserves native selection and copy behavior.

## Attribution

Ayu Dark palette values are adapted from Zed's MIT-licensed theme assets, with
attribution and the full MIT notice embedded in `appearance.rs` and available
under the application menu → Theme Credits on macOS, or View → Theme credits
elsewhere. Zed's GPL UI implementation is a design reference,
not copied or linked code. Source revision, paths and adaptation boundaries are
recorded in [SOURCE_MAP §6.2](../../docs/diana/SOURCE_MAP.md#62-gui-design-reference-and-theme-data).

## Implementation and checks

`main.rs` owns startup and window lifetime. `menus.rs` projects workspace state
into native macOS menus; the workspace observer updates it outside rendering.
`appearance.rs` maps shared theme
roles and reading dimensions; it uses installed Fira Code when available and
otherwise retains Kit's platform monospace fallback. `demo.rs` owns display content, `project_tree.rs` projects
its identities into Tree, `search.rs` adapts symbol matching to List, and
`workspace.rs` composes controls and manages documents. The GUI links neither
the Engine nor the CLI. Select it explicitly with `-p diana-gui`; default workspace
members remain unchanged.

```sh
cargo fmt --all -- --check
cargo clippy -p diana-gui --all-targets --all-features --locked -- -D warnings
cargo build -p diana-gui --release --locked
```

The package format, Clippy and release build checks pass on macOS. A native
window was launched and visually inspected; a complete interaction walkthrough
has not been performed. Build success does not establish interaction correctness
or accessibility. The root testing policy applies; this package adds no tests,
fixtures or test jobs.
Windows/Linux and assistive-technology behavior have not been exercised.
Workspace-wide checks remain blocked by the CLI's existing Engine/result
integration errors.
