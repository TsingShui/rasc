//! Projection of demo and DEX identities into Kit's virtualized Tree.
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable, h_flex,
    list::ListItem,
    tree::{Tree, TreeItem, TreeState, tree},
};
use gpui_kit::{assets::IconName, prelude::*, *};

use crate::{
    appearance as style,
    backend::{Class, Resource},
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

pub fn set_resource(state: &Entity<TreeState>, resource: &Resource, cx: &mut App) {
    let logical = resource.groups.iter().map(|group| {
        // Children are created before parents, without recursing on untrusted package depth.
        let mut items: Vec<Option<TreeItem>> = vec![None; group.packages.len()];
        for (index, package) in group.packages.iter().enumerate().rev() {
            let mut child_nodes = Vec::new();
            for &child in &package.children {
                if let Some(item) = items[child].take() {
                    child_nodes.push(item);
                }
            }
            let mut name = package.name.clone();
            if package.classes.is_empty() && child_nodes.len() == 1 {
                let child = child_nodes.remove(0);
                name.push('.');
                name.push_str(&child.label);
                child_nodes = child.children;
            }
            let classes = package.classes.iter().map(|&index| {
                let class = &resource.classes[index];
                let descriptor = class
                    .label
                    .strip_prefix('L')
                    .and_then(|v| v.strip_suffix(';'))
                    .unwrap_or(&class.label);
                let name = descriptor
                    .rsplit_once('/')
                    .map_or(descriptor, |(_, name)| name);
                TreeItem::new(
                    format!("class:{}:{}", class.header.0, class.id.0),
                    name.to_owned(),
                )
            });
            items[index] = Some(
                TreeItem::new(format!("package:{}:{index}", group.header.0), name)
                    .children(child_nodes)
                    .children(classes),
            );
        }
        let package_nodes = group
            .roots
            .iter()
            .filter_map(|&index| items[index].take())
            .collect::<Vec<_>>();
        TreeItem::new(
            format!("dex:{}", group.header.0),
            format!("DEX @ {}", group.header.0),
        )
        .expanded(true)
        .children(package_nodes)
    });
    let name = std::path::Path::new(&resource.name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| resource.name.clone());
    let root = TreeItem::new("resource", name)
        .expanded(true)
        .children(logical);
    state.update(cx, |state, cx| state.set_items(vec![root], cx));
}

pub fn resource_selection(state: &TreeState, resource: &Resource) -> Option<Class> {
    let mut parts = state.selected_item()?.id.split(':');
    if parts.next()? != "class" {
        return None;
    }
    let header = parts.next()?.parse::<usize>().ok()?;
    let id = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    resource.class(
        diana_decomp::ByteOffset(header),
        diana_decomp::ClassDefId(id),
    )
}

pub fn view(state: &Entity<TreeState>) -> Tree {
    let focus = state.downgrade();
    tree(state, move |index, entry, _, _, cx| {
        let item = entry.item();
        let icon = match item.id.as_ref() {
            "notes-demo.apk" => IconName::FileArchive,
            "classes.dex" | "resource" => IconName::File,
            id if id.starts_with("dex:") => IconName::File,
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
