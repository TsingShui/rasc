//! Projection of authored demo identities into Kit's Tree; no custom tree interaction.
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, h_flex,
    list::ListItem,
    tree::{Tree, TreeItem, TreeState, tree},
};
use gpui_kit::{assets::IconName, prelude::*, *};

use crate::{
    appearance as style,
    demo::{CLASSES, ClassId, PACKAGES},
};

fn key(id: ClassId) -> SharedString {
    let class = id.class();
    format!("{}.{}", class.package, class.name).into()
}

fn items() -> Vec<TreeItem> {
    let packages = PACKAGES.map(|package| {
        TreeItem::new(package, package).expanded(true).children(
            CLASSES
                .into_iter()
                .filter(|class| class.package == package)
                .map(|class| TreeItem::new(key(class.id), class.name)),
        )
    });
    let root = TreeItem::new("notes-demo.apk", "notes-demo.apk")
        .expanded(true)
        .child(
            TreeItem::new("classes.dex", "classes.dex")
                .expanded(true)
                .children(packages),
        );
    vec![root]
}

pub fn new(cx: &mut App) -> Entity<TreeState> {
    cx.new(|cx| TreeState::new(cx).items(items()))
}

pub fn reset(state: &Entity<TreeState>, cx: &mut App) {
    state.update(cx, |state, cx| state.set_items(items(), cx));
}

pub fn selection(state: &TreeState) -> Option<ClassId> {
    let selected = &state.selected_item()?.id;
    CLASSES
        .into_iter()
        .find(|class| key(class.id) == *selected)
        .map(|class| class.id)
}

pub fn select(state: &Entity<TreeState>, id: Option<ClassId>, cx: &mut App) {
    state.update(cx, |state, cx| {
        let item = id.map(|id| TreeItem::new(key(id), id.class().name));
        state.set_selected_item(item.as_ref(), cx);
        if let Some(index) = state.selected_index() {
            state.scroll_to_item(index, ScrollStrategy::Center);
        }
    });
}

pub fn view(state: &Entity<TreeState>) -> Tree {
    let focus = state.downgrade();
    tree(state, move |index, entry, _, _, cx| {
        let item = entry.item();
        let icon = match item.id.as_ref() {
            "notes-demo.apk" => IconName::FileArchive,
            "classes.dex" => IconName::File,
            _ if entry.is_folder() && entry.is_expanded() => IconName::FolderOpen,
            _ if entry.is_folder() => IconName::Folder,
            _ => IconName::FileCode,
        };
        let focus = focus.clone();
        let group: SharedString = format!("tree-row-{}", item.id).into();
        let show_disclosure = entry.depth() < 2 || !entry.is_expanded();
        ListItem::new(index)
            .group(group.clone())
            .h(px(style::TREE_ROW))
            .py_0()
            .px_2()
            .line_height(px(style::TREE_ROW))
            .accessibility_label(item.label.clone())
            .child(
                h_flex()
                    .gap_1()
                    .pl(px(entry.depth() as f32 * style::TREE_INDENT))
                    .child(div().w_3().flex_shrink_0().when(entry.is_folder(), |slot| {
                        slot.child(
                            div()
                                .opacity(if show_disclosure { 1. } else { 0. })
                                .group_hover(group.clone(), |arrow| arrow.opacity(1.))
                                .child(
                                    Icon::new(if entry.is_expanded() {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })
                                    .xsmall(),
                                ),
                        )
                    }))
                    .child(
                        Icon::new(icon)
                            .small()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().min_w_0().truncate().child(item.label.clone())),
            )
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                _ = focus.update(cx, |tree, cx| tree.focus(window, cx));
            })
    })
}
