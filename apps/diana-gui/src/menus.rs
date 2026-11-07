//! Project workspace state into macOS menus; native focus dispatch owns actions.
use gpui_kit::{component::input, *};

use crate::{Quit, demo::ViewMode, workspace::*};

pub fn refresh(demo_open: bool, mode: Option<ViewMode>, code_size: f32, cx: &mut App) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let no_document = mode.is_none();
    cx.set_menus([
        Menu::new("Diana").items([
            MenuItem::action("Theme Credits…", ThemeCredits),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit Diana", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("Open DEX…", OpenDex),
            MenuItem::action("Open Demo Workspace", OpenDemo).disabled(demo_open),
            MenuItem::separator(),
            MenuItem::action("Close Document", CloseTab).disabled(no_document),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", input::Undo, OsAction::Undo),
            MenuItem::os_action("Redo", input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", input::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", input::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", input::Paste, OsAction::Paste),
            MenuItem::os_action("Select All", input::SelectAll, OsAction::SelectAll),
            MenuItem::separator(),
            MenuItem::action("Copy Document", CopyDocument).disabled(no_document),
            MenuItem::action("Find…", FindInDocument).disabled(no_document),
        ]),
        Menu::new("View").items([
            MenuItem::action("Java-like", JavaView)
                .checked(mode == Some(ViewMode::Java))
                .disabled(no_document),
            MenuItem::action("DEX Instructions", DexView)
                .checked(mode == Some(ViewMode::Dex))
                .disabled(no_document),
            MenuItem::separator(),
            MenuItem::action("Original Facts…", ToggleFacts).disabled(no_document),
            MenuItem::separator(),
            MenuItem::action("Smaller Font", SmallerFont).disabled(no_document || code_size <= 11.),
            MenuItem::action("Larger Font", LargerFont).disabled(no_document || code_size >= 18.),
            MenuItem::separator(),
            MenuItem::action("Reset Layout", ResetLayout).disabled(!demo_open),
        ]),
        Menu::new("Navigate").items([
            MenuItem::action("Go to Symbol…", OpenSearch).disabled(!demo_open),
            MenuItem::action("Next Document", NextTab).disabled(no_document),
        ]),
    ]);
}
