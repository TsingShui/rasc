//! Authored symbol matching adapted to Kit's searchable List.
use gpui_kit::component::{
    ActiveTheme, IndexPath,
    list::{ListDelegate, ListItem, ListState},
    v_flex,
};
use gpui_kit::{prelude::*, *};

use crate::demo::{CLASSES, ClassId, SourceLine};

#[derive(Clone)]
pub struct Hit {
    pub class: ClassId,
    pub line: Option<SourceLine>,
    label: String,
    context: String,
}

fn find(query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    let mut hits = Vec::new();
    for class in CLASSES {
        if query.is_empty()
            || class.name.to_lowercase().contains(&query)
            || class.package.to_lowercase().contains(&query)
        {
            hits.push(Hit {
                class: class.id,
                line: None,
                label: class.name.into(),
                context: class.package.into(),
            });
        } else {
            for member in class.members {
                if member.name.to_lowercase().contains(&query) {
                    hits.push(Hit {
                        class: class.id,
                        line: Some(member.line),
                        label: member.name.into(),
                        context: class.name.into(),
                    });
                }
            }
        }
    }
    hits
}

pub struct Symbols {
    hits: Vec<Hit>,
}

impl Symbols {
    pub fn new() -> Self {
        Self { hits: find("") }
    }

    pub fn hit(&self, index: IndexPath) -> Option<&Hit> {
        (index.section == 0)
            .then(|| self.hits.get(index.row))
            .flatten()
    }
}

impl ListDelegate for Symbols {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.hits.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.hits = find(query);
        // Publish selection after List's query handler has reset the previous result.
        cx.defer_in(window, |state, window, cx| {
            let first = (!state.delegate().hits.is_empty()).then_some(IndexPath::default());
            state.set_selected_index(first, window, cx);
            cx.notify();
        });
        cx.notify();
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        // ListState owns selection; the application consumes ListEvent::Confirm.
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let hit = self.hit(index)?;
        Some(
            ListItem::new(index.row)
                .accessibility_label(hit.label.clone())
                .child(
                    v_flex()
                        .child(div().truncate().child(hit.label.clone()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(hit.context.clone()),
                        ),
                ),
        )
    }
}
