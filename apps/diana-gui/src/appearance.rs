//! Application theme and density. Kit still owns widget rendering and interaction.
//! Ayu Dark color values: Zed assets/themes/ayu/ayu.json at
//! 654efae441bc8eb117d0042bb28cd7cf3570bdc9; MIT notice below.
use std::sync::Arc;

use gpui_kit::{
    App, Hsla,
    component::{
        Theme, ThemeMode,
        highlighter::{FontStyle, FontWeightContent, ThemeStyle},
    },
    px, rgb,
};

pub const SIDEBAR_WIDTH: f32 = 255.;
pub const READING_WIDTH: f32 = 1120.;
pub const CODE_SIZE: f32 = 16.;
pub const CODE_LEADING: f32 = 24.;
pub const TREE_ROW: f32 = 26.;
pub const TREE_INDENT: f32 = 12.;

pub const THEME_NOTICE: &str = r#"Ayu Dark palette, adapted to GPUI Kit theme roles.
Source: zed-industries/zed, assets/themes/ayu, revision
654efae441bc8eb117d0042bb28cd7cf3570bdc9.

The MIT License (MIT)

Copyright (c) 2016 Ike Ku

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE."#;

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

pub fn configure(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    let fira_code_available = cx
        .text_system()
        .all_font_names()
        .iter()
        .any(|name| name == "Fira Code");
    Theme::update(cx, |theme| {
        // Share semantic roles across Tree, tabs, menus, dialogs, sheets and Editor.
        // Do not maintain local hover/selection paint in each control.
        theme.font_size = px(16.);
        theme.mono_font_size = px(CODE_SIZE);
        if fira_code_available {
            theme.mono_font_family = "Fira Code".into();
        }
        theme.radius = px(3.);
        theme.radius_lg = px(6.);
        theme.background = color(0x0d1016);
        theme.foreground = color(0xbfbdb6);
        theme.border = color(0x2d2f34);
        theme.muted = color(0x1f2127);
        theme.muted_foreground = color(0x8a8986);
        theme.accent = color(0x3e4043);
        theme.accent_foreground = theme.foreground;
        theme.primary = color(0x5ac1fe);
        theme.primary_foreground = theme.background;
        theme.ring = color(0x1b4a6e);
        theme.input = theme.border;
        theme.caret = theme.foreground;
        theme.selection = theme.primary.opacity(0.24);
        theme.sidebar = color(0x1f2127);
        theme.sidebar_border = theme.border;
        theme.sidebar_foreground = theme.foreground;
        theme.sidebar_accent = theme.accent;
        theme.sidebar_accent_foreground = theme.foreground;
        theme.colors.list = theme.sidebar;
        theme.list_hover = color(0x2d2f34);
        theme.list_active = theme.accent;
        theme.list_active_border = theme.ring;
        theme.tab_bar = theme.sidebar;
        theme.tab = theme.sidebar;
        theme.tab_foreground = theme.muted_foreground;
        theme.tab_active = theme.background;
        theme.tab_active_foreground = theme.foreground;
        theme.title_bar = color(0x313337);
        theme.title_bar_border = color(0x3f4043);
        theme.button = theme.sidebar;
        theme.button_foreground = theme.foreground;
        theme.button_hover = theme.list_hover;
        theme.button_active = theme.accent;
        theme.secondary = theme.sidebar;
        theme.secondary_foreground = theme.foreground;
        theme.secondary_hover = theme.list_hover;
        theme.secondary_active = theme.accent;
        theme.popover = theme.sidebar;
        theme.popover_foreground = theme.foreground;
        theme.description_list_label = theme.sidebar;
        theme.description_list_label_foreground = theme.muted_foreground;
        theme.scrollbar = color(0x1f2127).opacity(0.);
        theme.scrollbar_thumb = theme.foreground.opacity(0.3);
        theme.scrollbar_thumb_hover = theme.muted_foreground;
        theme.warning = color(0x572815);
        theme.warning_foreground = color(0xfeb454);

        let highlight = Arc::make_mut(&mut theme.highlight_theme);
        highlight.name = "Diana · Ayu Dark".into();
        highlight.style.editor_background = Some(color(0x0d1016));
        highlight.style.editor_foreground = Some(color(0xbfbdb6));
        highlight.style.editor_gutter_background = Some(color(0x0d1016));
        highlight.style.editor_active_line = Some(color(0x1f2127).opacity(0.75));
        highlight.style.editor_line_number = Some(color(0x4b4c4e));
        highlight.style.editor_active_line_number = Some(color(0xcbcccd));
        highlight.style.editor_invisible = Some(color(0x666767));
        let syntax = &mut highlight.style.syntax;
        let base = syntax.variable.or(syntax.keyword).or(syntax.type_);
        let tinted = |hex| {
            base.map(|mut style: ThemeStyle| {
                style.color = Some(color(hex));
                style.font_style = Some(FontStyle::Normal);
                style.font_weight = Some(FontWeightContent::Normal);
                style
            })
        };
        syntax.keyword = tinted(0xff8f3f);
        syntax.preproc = syntax.keyword;
        syntax.type_ = tinted(0x5ac1fe);
        syntax.constructor = syntax.type_;
        syntax.function = tinted(0xffb353);
        syntax.string = tinted(0xa9d94b);
        syntax.string_escape = tinted(0x8c8b88);
        syntax.string_regex = tinted(0x95e6cb);
        syntax.comment = tinted(0x5c6773);
        syntax.comment_doc = tinted(0x8c8b88);
        syntax.number = tinted(0xd2a6ff);
        syntax.constant = syntax.number;
        syntax.boolean = syntax.number;
        syntax.variable = tinted(0xbfbdb6);
        syntax.variable_special = syntax.variable;
        syntax.property = syntax.type_;
        syntax.attribute = syntax.type_;
        syntax.operator = tinted(0xf29668);
        syntax.punctuation = tinted(0xa6a5a0);
        syntax.punctuation_bracket = syntax.punctuation;
        syntax.punctuation_delimiter = syntax.punctuation;
    });
}
