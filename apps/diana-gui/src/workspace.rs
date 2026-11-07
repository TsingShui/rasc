use std::cell::Cell;

use gpui_kit::component::{
    ActiveTheme, Disableable, IconName, IndexPath, Sizable, TitleBar, WindowExt,
    alert::Alert,
    button::{Button, ButtonVariants},
    description_list::DescriptionList,
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyTitle},
    h_flex,
    input::{Editor, EditorState, Position, TabSize},
    list::{List, ListEvent, ListState},
    menu::{DropdownMenu, PopupMenuItem},
    tab::{Tab, TabBar},
    tree::TreeState,
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::{
    appearance as style,
    demo::{ClassId, SourceLine, ViewMode},
    project_tree,
    search::Symbols,
};

actions!(
    diana_workspace,
    [
        OpenSearch,
        OpenDemo,
        OpenDex,
        FindInDocument,
        CopyDocument,
        ToggleFacts,
        CloseTab,
        NextTab,
        DismissOverlay,
        JavaView,
        DexView,
        SmallerFont,
        LargerFont,
        ResetLayout,
        ThemeCredits
    ]
);

struct Document {
    id: ClassId,
    mode: ViewMode,
    editors: [Entity<EditorState>; 2],
}

impl Document {
    fn new(id: ClassId, window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let editors = ViewMode::ALL.map(|mode| {
            cx.new(|cx| {
                let mut state = EditorState::new(window, cx)
                    .language(if mode == ViewMode::Java {
                        "java"
                    } else {
                        "text"
                    })
                    .default_value(id.source(mode))
                    .line_number(true)
                    .searchable(true)
                    .soft_wrap(false)
                    .tab_size(TabSize {
                        tab_size: 4,
                        hard_tabs: false,
                    });
                state.set_readonly(true, cx);
                state
            })
        });
        Self {
            id,
            mode: ViewMode::Java,
            editors,
        }
    }

    fn editor(&self) -> &Entity<EditorState> {
        match self.mode {
            ViewMode::Java => &self.editors[0],
            ViewMode::Dex => &self.editors[1],
        }
    }
}

struct SearchSession {
    list: Entity<ListState<Symbols>>,
    _subscription: Subscription,
}

pub struct Workspace {
    focus: FocusHandle,
    tree: Entity<TreeState>,
    _tree_observation: Subscription,
    _menu_observation: Subscription,
    search: Option<SearchSession>,
    documents: Vec<Document>,
    active: Option<ClassId>,
    demo_open: bool,
    code_size: f32,
    resource: Option<crate::backend::Resource>,
    real_editor: Option<Entity<EditorState>>,
    real_status: String,
    real_selected: Option<(diana_decomp::ByteOffset, diana_decomp::ClassDefId)>,
    generation: u64,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let tree = project_tree::new(cx);
        let observation = cx.observe_in(&tree, window, |view, tree, window, cx| {
            if view.demo_open
                && let Some(id) = project_tree::selection(tree.read(cx))
                && view.active != Some(id)
            {
                // Tree owns keyboard focus and preview selection; do not steal it.
                view.activate(id, false, window, cx);
            } else if let Some(resource) = &view.resource
                && let Some(class) = project_tree::resource_selection(tree.read(cx), resource)
                && view.real_selected != Some((class.header, class.id))
            {
                view.open_real_class(class, window, cx);
            }
        });
        let menu_observation = cx.observe_self(|view, cx| {
            crate::menus::refresh(
                view.demo_open,
                view.active_document().map(|document| document.mode),
                view.code_size,
                cx,
            );
        });
        let view = Self {
            focus: cx.focus_handle(),
            tree,
            _tree_observation: observation,
            _menu_observation: menu_observation,
            search: None,
            documents: Vec::new(),
            active: None,
            demo_open: false,
            code_size: style::CODE_SIZE,
            resource: None,
            real_editor: None,
            real_status: String::new(),
            real_selected: None,
            generation: 0,
        };
        view.focus.focus(window, cx);
        cx.notify();
        view
    }

    fn open_dex(&mut self, _: &OpenDex, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open DEX".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let path = match prompt.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let Some(path) = path else {
                return;
            };
            let Ok(generation) = this.update(cx, |view, _| {
                view.generation = view.generation.wrapping_add(1);
                if view.real_editor.is_none() {
                    view.real_selected = None;
                }
                view.generation
            }) else {
                return;
            };
            let result = cx
                .background_spawn(async move { crate::backend::open(path) })
                .await;
            _ = this.update_in(cx, |view, _, cx| {
                if view.generation != generation {
                    return;
                }
                match result {
                    Ok(resource) => {
                        view.demo_open = false;
                        project_tree::set_resource(&view.tree, &resource, cx);
                        view.real_editor = None;
                        view.real_selected = None;
                        view.real_status = if resource.issues.is_empty() {
                            format!("{} classes", resource.classes.len())
                        } else {
                            resource.issues.join("\n")
                        };
                        view.resource = Some(resource);
                    }
                    Err(error) => {
                        view.real_status = format!("Cannot open DEX: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn open_real_class(
        &mut self,
        class: crate::backend::Class,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(resource) = &self.resource else {
            return;
        };
        let key = (class.header, class.id);
        if self.real_selected == Some(key) {
            return;
        }
        let bytes = resource.bytes.clone();
        self.real_selected = Some(key);
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.real_status = format!("Decompiling {}…", class.label);
        self.real_editor = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { crate::backend::decompile(bytes, class) })
                .await;
            _ = this.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                match result {
                    Ok(text) => {
                        view.real_editor = Some(cx.new(|cx| {
                            let mut editor = EditorState::new(window, cx)
                                .language("java")
                                .default_value(text)
                                .line_number(true)
                                .searchable(true)
                                .soft_wrap(false);
                            editor.set_readonly(true, cx);
                            editor
                        }));
                        view.real_status = "Diana Engine · Java-like".into();
                    }
                    Err(error) => {
                        view.real_selected = None;
                        view.real_status = format!("Decompilation failed: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn real_view(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let resource = self.resource.as_ref();
        h_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                v_flex()
                    .w(px(style::SIDEBAR_WIDTH))
                    .h_full()
                    .bg(cx.theme().sidebar)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(project_tree::view(&self.tree)),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(div().p_2().child(self.real_status.clone()))
                    .when_some(resource, |view, resource| {
                        view.children(
                            resource
                                .issues
                                .iter()
                                .map(|issue| div().p_2().child(issue.clone())),
                        )
                    })
                    .when_some(self.real_editor.as_ref(), |view, editor| {
                        view.child(
                            Editor::new(editor)
                                .readonly(true)
                                .bordered(false)
                                .w_full()
                                .h_full(),
                        )
                    }),
            )
    }

    fn active_document(&self) -> Option<&Document> {
        self.documents
            .iter()
            .find(|document| Some(document.id) == self.active)
    }

    fn activate(
        &mut self,
        id: ClassId,
        focus_editor: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.documents.iter().any(|document| document.id == id) {
            self.documents.push(Document::new(id, window, cx));
        }
        self.active = Some(id);
        if focus_editor {
            project_tree::select(&self.tree, Some(id), cx);
            self.focus_editor(window, cx);
        }
        cx.notify();
    }

    fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(document) = self.active_document() {
            document
                .editor()
                .update(cx, |editor, cx| editor.focus(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
    }

    fn close(&mut self, id: ClassId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.documents.iter().position(|document| document.id == id) else {
            return;
        };
        self.documents.remove(index);
        if self.active == Some(id) {
            self.active = self
                .documents
                .get(index)
                .or_else(|| self.documents.last())
                .map(|d| d.id);
            project_tree::select(&self.tree, self.active, cx);
        }
        if self.active.is_none() {
            window.close_sheet(cx);
        }
        self.focus_editor(window, cx);
        cx.notify();
    }

    fn copy_document(&mut self, _: &CopyDocument, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(document) = self.active_document() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                document.editor().read(cx).value().to_string(),
            ));
        }
    }

    fn close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active {
            self.close(id, window, cx);
        }
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| Some(document.id) == self.active)
        {
            let next = self.documents[(index + 1) % self.documents.len()].id;
            self.activate(next, true, window, cx);
        }
    }

    fn open_search(&mut self, _: &OpenSearch, window: &mut Window, cx: &mut Context<Self>) {
        if !self.demo_open {
            return;
        }
        if window.has_active_dialog(cx) {
            if let Some(search) = &self.search {
                search.list.update(cx, |list, cx| list.focus(window, cx));
            }
            return;
        }
        let list = cx.new(|cx| ListState::new(Symbols::new(), window, cx).searchable(true));
        list.update(cx, |list, cx| {
            list.set_selected_index(Some(IndexPath::default()), window, cx)
        });
        let subscription =
            cx.subscribe_in(&list, window, |view, list, event, window, cx| match event {
                ListEvent::Confirm(index) => {
                    let hit = list.read(cx).delegate().hit(*index).cloned();
                    if let Some(hit) = hit {
                        window.close_dialog(cx);
                        view.search = None;
                        view.activate(hit.class, true, window, cx);
                        if let Some(line) = hit.line {
                            view.reveal_line(hit.class, ViewMode::Java, line, window, cx);
                        }
                    }
                }
                ListEvent::Cancel => {
                    window.close_dialog(cx);
                    view.search = None;
                }
                ListEvent::Select(_) => {}
            });
        self.search = Some(SearchSession {
            list: list.clone(),
            _subscription: subscription,
        });
        let workspace = cx.entity().downgrade();
        let initial_focus = Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                list.update(cx, |list, cx| list.focus(window, cx));
            }
            let workspace = workspace.clone();
            dialog
                .title("Go to Symbol")
                .child(
                    List::new(&list)
                        .search_placeholder("Search classes and methods…")
                        .h(px(360.)),
                )
                .on_close(move |_, _, cx| {
                    _ = workspace.update(cx, |view, cx| {
                        view.search = None;
                        cx.notify();
                    });
                })
        });
    }

    fn find_in_document(
        &mut self,
        _: &FindInDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.active_document() {
            document.editor().update(cx, |editor, cx| {
                editor.focus(window, cx);
                editor.open_search(false, cx);
            });
        }
    }

    fn dismiss_overlay(&mut self, _: &DismissOverlay, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_sheet(cx) {
            window.close_sheet(cx);
        } else {
            cx.propagate();
        }
    }

    fn reveal_line(
        &mut self,
        id: ClassId,
        mode: ViewMode,
        line: SourceLine,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(document) = self.documents.iter_mut().find(|document| document.id == id) {
            document.mode = mode;
            document.editor().update(cx, |editor, cx| {
                editor.set_cursor_position(Position::new(line.0.saturating_sub(1), 0), window, cx);
                editor.focus(window, cx);
            });
        }
        cx.notify();
    }

    fn change_mode(&mut self, mode: ViewMode, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(document) = self
            .documents
            .iter_mut()
            .find(|d| Some(d.id) == self.active)
        {
            document.mode = mode;
        }
        self.focus_editor(window, cx);
        cx.notify();
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.resource = None;
        self.real_editor = None;
        self.real_selected = None;
        self.demo_open = true;
        project_tree::reset(&self.tree, cx);
        self.documents = vec![
            Document::new(ClassId::NotesRepository, window, cx),
            Document::new(ClassId::MainActivity, window, cx),
        ];
        self.active = Some(ClassId::NotesRepository);
        self.code_size = style::CODE_SIZE;
        project_tree::select(&self.tree, self.active, cx);
        self.focus_editor(window, cx);
        cx.notify();
    }

    fn toggle_facts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_sheet(cx) {
            window.close_sheet(cx);
            return;
        }
        let Some(id) = self.active else {
            return;
        };
        let workspace = cx.entity().downgrade();
        // The sheet names its captured class so changing tabs cannot mislabel its facts.
        window.open_sheet(cx, move |sheet, _, _| {
            let class = id.class();
            sheet
                .title(format!("Facts — {}", class.name))
                .child(
                    DescriptionList::vertical()
                        .columns(1)
                        .item("Source", "Authored UI demo, not Engine output", 1)
                        .item(
                            "Descriptor",
                            format!("L{}/{};", class.package.replace('.', "/"), class.name),
                            1,
                        ),
                )
                .when_some(class.warning, |sheet, line| {
                    let workspace = workspace.clone();
                    sheet
                        .child(Alert::warning(
                            "call-uncertainty",
                            "The target declaration for sync() is incomplete.",
                        ))
                        .child(DescriptionList::vertical().columns(1).item(
                            "Original call (illustration)",
                            "Ldemo/notes/net/SyncApi; → push(Ljava/util/List;)V",
                            1,
                        ))
                        .child(Button::new("locate-call").label("Go to call").on_click(
                            move |_, window, cx| {
                                window.close_sheet(cx);
                                _ = workspace.update(cx, |view, cx| {
                                    view.activate(id, true, window, cx);
                                    view.reveal_line(id, ViewMode::Java, line, window, cx);
                                });
                            },
                        ))
                })
        });
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.entity().downgrade();
        TitleBar::new().bg(cx.theme().title_bar).child(
            h_flex()
                .gap_1()
                .child(
                    Button::new("search")
                        .ghost()
                        .small()
                        .label("Search")
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.open_search(&OpenSearch, window, cx)
                        })),
                )
                .child(
                    Button::new("copy")
                        .ghost()
                        .small()
                        .label("Copy")
                        .disabled(self.active.is_none())
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.copy_document(&CopyDocument, window, cx)
                        })),
                )
                .child(
                    Button::new("facts")
                        .ghost()
                        .small()
                        .label("Facts")
                        .disabled(self.active.is_none())
                        .on_click(cx.listener(|view, _, window, cx| view.toggle_facts(window, cx))),
                )
                .child(
                    Button::new("view")
                        .ghost()
                        .small()
                        .label("View")
                        .dropdown_menu(move |menu, _, cx| {
                            let Some(workspace) = workspace.upgrade() else {
                                return menu;
                            };
                            let view = workspace.read(cx);
                            let mode = view.active_document().map(|document| document.mode);
                            menu.item(
                                PopupMenuItem::new("Java-like")
                                    .action(Box::new(JavaView))
                                    .checked(mode == Some(ViewMode::Java))
                                    .disabled(mode.is_none()),
                            )
                            .item(
                                PopupMenuItem::new("DEX instructions")
                                    .action(Box::new(DexView))
                                    .checked(mode == Some(ViewMode::Dex))
                                    .disabled(mode.is_none()),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Smaller font")
                                    .action(Box::new(SmallerFont))
                                    .disabled(view.code_size <= 11.),
                            )
                            .item(
                                PopupMenuItem::new("Larger font")
                                    .action(Box::new(LargerFont))
                                    .disabled(view.code_size >= 18.),
                            )
                            .separator()
                            .menu("Reset layout", Box::new(ResetLayout))
                            .separator()
                            .menu("Theme credits", Box::new(ThemeCredits))
                        }),
                ),
        )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sidebar is a navigation-menu container, not a host for Tree. This column only sizes it.
        v_flex()
            .w(px(style::SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .pt_2()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(project_tree::view(&self.tree)),
            )
            .child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("UI demo · authored content"),
            )
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        TabBar::new("document-tabs")
            .menu(true)
            .when_some(
                self.documents
                    .iter()
                    .position(|d| Some(d.id) == self.active),
                |bar, index| bar.selected_index(index),
            )
            .children(self.documents.iter().map(|document| {
                let id = document.id;
                Tab::new().label(id.class().name).suffix(
                    Button::new(("close-tab", id.ordinal()))
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .accessibility_label(format!("Close {}", id.class().name))
                        .on_click(cx.listener(move |view, _, window, cx| {
                            cx.stop_propagation();
                            view.close(id, window, cx);
                        })),
                )
            }))
            .on_click(cx.listener(|view, index: &usize, window, cx| {
                if let Some(id) = view.documents.get(*index).map(|document| document.id) {
                    view.activate(id, true, window, cx);
                }
            }))
    }

    fn welcome(&self) -> impl IntoElement {
        Empty::new().header(
            EmptyHeader::new()
                .description(EmptyDescription::new().child("Open a DEX file to get started.")),
        )
    }

    fn reader(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_center()
            .when_some(self.active_document(), |reader, document| {
                reader.child(
                    Editor::new(document.editor())
                        .readonly(true)
                        .bordered(false)
                        .w_full()
                        .max_w(px(style::READING_WIDTH))
                        .h_full()
                        .text_size(px(self.code_size))
                        .line_height(px(self.code_size * style::CODE_LEADING / style::CODE_SIZE))
                        .aria_label(format!(
                            "{} · {} · authored demo",
                            document.id.class().name,
                            document.mode.label()
                        )),
                )
            })
            .when(self.active.is_none(), |reader| {
                reader.child(
                    Empty::new()
                        .header(
                            EmptyHeader::new()
                                .title(EmptyTitle::new().child("No open documents"))
                                .description(
                                    EmptyDescription::new().child(
                                        "Select a class in the tree or search for a symbol.",
                                    ),
                                ),
                        )
                        .child(
                            Button::new("open-demo")
                                .label("Open NotesRepository")
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.activate(ClassId::NotesRepository, true, window, cx)
                                })),
                        ),
                )
            })
    }
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .track_focus(&self.focus)
            .key_context("DianaWorkspace")
            .on_action(cx.listener(Self::open_search))
            .on_action(cx.listener(Self::open_dex))
            .on_action(cx.listener(|view, _: &OpenDemo, window, cx| {
                if !view.demo_open {
                    view.reset(window, cx);
                }
            }))
            .on_action(cx.listener(Self::find_in_document))
            .on_action(cx.listener(Self::copy_document))
            .on_action(
                cx.listener(|view, _: &ToggleFacts, window, cx| view.toggle_facts(window, cx)),
            )
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::dismiss_overlay))
            .on_action(cx.listener(|view, _: &JavaView, window, cx| {
                view.change_mode(ViewMode::Java, window, cx)
            }))
            .on_action(cx.listener(|view, _: &DexView, window, cx| {
                view.change_mode(ViewMode::Dex, window, cx)
            }))
            .on_action(cx.listener(|view, _: &SmallerFont, _, cx| {
                view.code_size = (view.code_size - 1.).max(11.);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &LargerFont, _, cx| {
                view.code_size = (view.code_size + 1.).min(18.);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &ThemeCredits, window, cx| {
                window.open_dialog(cx, |dialog, _, _| {
                    dialog
                        .title("Theme credits")
                        .child(div().text_sm().child(style::THEME_NOTICE))
                });
            }))
            .on_action(cx.listener(|view, _: &ResetLayout, window, cx| {
                if view.demo_open {
                    window.close_sheet(cx);
                    view.reset(window, cx);
                }
            }))
            .when(!cfg!(target_os = "macos"), |root| {
                if self.demo_open {
                    root.child(self.toolbar(cx))
                } else {
                    root.child(TitleBar::new().bg(cx.theme().title_bar))
                }
            })
            .map(|root| {
                if self.resource.is_some() {
                    return root.child(self.real_view(cx));
                }
                if !self.demo_open {
                    return root
                        .child(self.welcome())
                        .child(Button::new("open-dex").label("Open DEX…").on_click(
                            cx.listener(|view, _, window, cx| view.open_dex(&OpenDex, window, cx)),
                        ))
                        .child(div().child(self.real_status.clone()));
                }
                root.child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .items_start()
                        .child(self.sidebar(cx))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .when(!self.documents.is_empty(), |content| {
                                    content.child(self.tabs(cx))
                                })
                                .child(self.reader(cx)),
                        ),
                )
            })
    }
}

pub fn bind_keys(cx: &mut App) {
    let (search, find, close) = if cfg!(target_os = "macos") {
        ("cmd-k", "cmd-f", "cmd-w")
    } else {
        ("ctrl-k", "ctrl-f", "ctrl-w")
    };
    cx.bind_keys([
        KeyBinding::new(search, OpenSearch, Some("DianaWorkspace")),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-o"
            } else {
                "ctrl-o"
            },
            OpenDex,
            Some("DianaWorkspace"),
        ),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-shift-d"
            } else {
                "ctrl-shift-d"
            },
            OpenDemo,
            Some("DianaWorkspace"),
        ),
        KeyBinding::new(find, FindInDocument, Some("DianaWorkspace")),
        KeyBinding::new(close, CloseTab, Some("DianaWorkspace")),
        KeyBinding::new("ctrl-tab", NextTab, Some("DianaWorkspace")),
        KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-shift-c"
            } else {
                "ctrl-shift-c"
            },
            CopyDocument,
            Some("DianaWorkspace"),
        ),
        KeyBinding::new("escape", DismissOverlay, Some("DianaWorkspace")),
    ]);
}
