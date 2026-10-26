//! The ⌘P search palette: a floating panel over the workbench with grouped results
//! on the left and the selected session's preview on the right.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Hit {
    NewSession,
    NextWaiting,
    /// A session; `quote` is the user's own words that matched, when that is why it is here.
    Session {
        id: String,
        quote: Option<String>,
    },
    Task(String),
}
pub(super) struct Section {
    pub title: Option<&'static str>,
    pub hits: Vec<Hit>,
}
/// The palette's own state: whether it shows, its query field and the selected row.
pub(super) struct Search {
    open: bool,
    field: Entity<Field>,
    /// The selected row in the palette, counted across all groups.
    index: usize,
    scroll: gpui::ScrollHandle,
}
impl Search {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Workbench>) -> Self {
        let search = cx.new(|cx| {
            Field::new("搜索会话、待办或你说过的话", "", cx)
                .compact()
                .bare()
        });
        let mut last_query = String::new();
        cx.observe(&search, move |this, field, cx| {
            let query = field.read(cx).text.clone();
            if query != last_query {
                last_query = query;
                this.search.index = 0;
                this.search.scroll.scroll_to_item(0);
            }
            cx.notify();
        })
        .detach();
        cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
            crate::ui::field::FieldEvent::Submit => this.open_selected_hit(window, cx),
            crate::ui::field::FieldEvent::Cancel => this.close_search(cx),
        })
        .detach();
        Self {
            open: false,
            field: search,
            index: 0,
            scroll: gpui::ScrollHandle::new(),
        }
    }
    pub(super) fn is_open(&self) -> bool {
        self.open
    }
    /// Hides the palette without the refocus that closing it from its own keys does.
    pub(super) fn hide(&mut self) {
        self.open = false;
    }
    #[cfg(feature = "visual-test")]
    pub(super) fn field(&self) -> &Entity<Field> {
        &self.field
    }
}
const RECENT: usize = 12;
const EXCERPT: usize = 90;

impl Workbench {
    /// What the palette lists for `query` (already trimmed): actions and recent sessions
    /// when empty, otherwise matching sessions, quotes and todos.
    pub(super) fn search_sections(&self, query: &str) -> Vec<Section> {
        let mut visible: Vec<&AgentSession> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| self.session_hidden(s).is_none())
            .collect();
        visible.sort_by_key(|b| std::cmp::Reverse(b.updated));
        let session = |s: &AgentSession, quote: Option<String>| Hit::Session {
            id: s.id.clone(),
            quote,
        };
        let mut sections = Vec::new();
        if query.is_empty() {
            let mut actions = vec![Hit::NewSession];
            if !self.needs_you().is_empty() {
                actions.push(Hit::NextWaiting);
            }
            sections.push(Section {
                title: None,
                hits: actions,
            });
            let at = now();
            for s in visible.into_iter().take(RECENT) {
                let title = recency(s.updated, at);
                match sections.last_mut() {
                    Some(last) if last.title == Some(title) => last.hits.push(session(s, None)),
                    _ => sections.push(Section {
                        title: Some(title),
                        hits: vec![session(s, None)],
                    }),
                }
            }
            return sections;
        }
        let sessions: Vec<Hit> = visible
            .iter()
            .filter(|s| {
                contains(
                    &format!("{} {} {}", self.session_title(s), s.cli_session_id, s.cwd),
                    query,
                )
            })
            .map(|s| session(s, None))
            .collect();
        let quotes: Vec<Hit> = visible
            .iter()
            .filter_map(|s| {
                let content = self.workspace.content.get(&s.id)?;
                let ask = content
                    .asks
                    .iter()
                    .rev()
                    .find(|q| contains(&q.text, query))?;
                Some(session(s, Some(ask.text.clone())))
            })
            .collect();
        let tasks: Vec<Hit> = self
            .workspace
            .tasks
            .iter()
            .filter(|t| contains(&format!("{} {}", t.title, t.description), query))
            .map(|t| Hit::Task(t.id.clone()))
            .collect();
        for (title, hits) in [("会话", sessions), ("你说过的话", quotes), ("待办", tasks)]
        {
            if !hits.is_empty() {
                sections.push(Section {
                    title: Some(title),
                    hits,
                });
            }
        }
        sections
    }
    fn search_query(&self, cx: &gpui::App) -> String {
        self.search.field.read(cx).text.trim().to_owned()
    }
    fn selected_hit(&self, cx: &gpui::App) -> Option<Hit> {
        self.search_sections(&self.search_query(cx))
            .into_iter()
            .flat_map(|s| s.hits)
            .nth(self.search.index)
    }
    /// ↑ ↓ move through every row; Tab and ⇧Tab jump between groups.
    pub(super) fn search_key(&mut self, event: &gpui::KeyDownEvent, cx: &mut Context<Self>) {
        if self.search.field.read(cx).composing() {
            return;
        }
        let k = &event.keystroke;
        let sizes: Vec<usize> = self
            .search_sections(&self.search_query(cx))
            .iter()
            .map(|s| s.hits.len())
            .collect();
        let len = sizes.iter().sum();
        self.search.index = match k.key.as_str() {
            "up" => step(self.search.index, len, -1),
            "down" => step(self.search.index, len, 1),
            "tab" => section_jump(&sizes, self.search.index, k.modifiers.shift),
            _ => return,
        };
        self.search.scroll.scroll_to_item(self.search.index);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn open_hit(&mut self, hit: Hit, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = false;
        match hit {
            Hit::NewSession => {
                self.nav.overview = false;
                self.launch(false, None, cx);
            }
            Hit::NextWaiting => self.next_waiting(window, cx),
            Hit::Session { id, .. } => {
                if let Some(s) = self.workspace.agent_sessions.iter().find(|s| s.id == id) {
                    let s = s.clone();
                    self.enter_card(&s, window, cx);
                }
            }
            Hit::Task(id) => {
                if let Some(task) = self.workspace.tasks.iter().find(|t| t.id == id) {
                    self.nav.space = task.space.clone();
                    self.nav.overview = false;
                    self.nav.task_detail = Some(id);
                    self.sync_panes(cx);
                }
            }
        }
        self.nav.pending_focus = true;
        cx.notify();
    }
    pub(super) fn open_selected_hit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(hit) = self.selected_hit(cx) {
            self.open_hit(hit, window, cx);
        }
    }
    pub(super) fn search_palette(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let query = self.search_query(cx);
        let sections = self.search_sections(&query);
        let total: usize = sections.iter().map(|s| s.hits.len()).sum();
        let index = self.search.index.min(total.saturating_sub(1));
        let selected = sections.iter().flat_map(|s| &s.hits).nth(index).cloned();
        // Each row is one direct child of the list, so the scroll handle can reveal it.
        let mut list = div()
            .id("search-list")
            .flex_1()
            .min_w_0()
            .max_h(px(T::PALETTE_LIST))
            .overflow_y_scroll()
            .track_scroll(&self.search.scroll)
            .py(px(T::S6))
            .flex()
            .flex_col();
        let mut row = 0;
        for section in &sections {
            for (i, hit) in section.hits.iter().enumerate() {
                let heading = (i == 0).then_some(section.title).flatten();
                list = list.child(self.hit_row(hit, heading, row, row == index, &query, cx));
                row += 1;
            }
        }
        if total == 0 {
            list = list.child(
                div()
                    .px(px(T::S16))
                    .py(px(T::S20))
                    .text_color(T::color(T::SECONDARY))
                    .child(format!("没有找到“{query}”。未索引的历史不会出现在这里。")),
            );
        }
        let preview = match (&selected, query.is_empty()) {
            (Some(Hit::Session { id, quote }), false) => {
                self.hit_preview(id, quote.as_deref(), &query)
            }
            _ => None,
        };
        let field_row = div()
            .px(px(T::S16))
            .flex()
            .items_center()
            .gap(px(T::S10))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .child(
                div()
                    .text_size(px(T::SEARCH_TEXT))
                    .text_color(T::color(T::SECONDARY))
                    .child("⌕"),
            )
            .child(div().flex_1().min_w_0().child(self.search.field.clone()))
            .when(!query.is_empty(), |d| d.child(label(format!("{total} 项"))))
            .child(kbd("esc"));
        let footer = div()
            .px(px(T::S16))
            .py(px(T::S8))
            .flex()
            .gap(px(T::S14))
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .text_size(px(T::META))
            .text_color(T::color(T::MUTED))
            .child("↑↓ 选择")
            .child("↵ 打开")
            .child("⇥ 切换分组")
            .child(div().flex_1())
            .child("只搜索本地已索引的内容");
        let panel = div()
            .id("search-palette")
            .w(px(T::PALETTE_WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::POPOVER))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::PALETTE_RADIUS))
            .shadow_lg()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event, _, cx| this.search_key(event, cx)))
            .child(field_row)
            .child(
                div()
                    .flex()
                    .min_h_0()
                    .child(list)
                    .when_some(preview, |d, preview| d.child(preview)),
            )
            .child(footer);
        // Deferred so it paints after the workbench's text, which the scrim must dim too.
        gpui::deferred(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .justify_center()
                .items_start()
                .pt(px(T::PALETTE_TOP))
                .px(px(T::S16))
                .bg(gpui::rgba(T::SCRIM))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.close_search(cx)),
                )
                .child(panel),
        )
        .with_priority(1)
        .into_any_element()
    }
    fn hit_row(
        &self,
        hit: &Hit,
        heading: Option<&'static str>,
        row: usize,
        selected: bool,
        query: &str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let (lead, title, meta): (gpui::AnyElement, String, String) = match hit {
            Hit::NewSession => (glyph("＋"), "在当前空间新建会话".into(), String::new()),
            Hit::NextWaiting => (
                glyph("→"),
                "跳到下一个等你的会话".into(),
                format!("{} 个", self.needs_you().len()),
            ),
            Hit::Task(id) => {
                let task = self.workspace.tasks.iter().find(|t| &t.id == id);
                (
                    glyph("○"),
                    task.map(|t| t.title.clone()).unwrap_or_default(),
                    task.and_then(|t| self.space_name(&t.space))
                        .unwrap_or_default(),
                )
            }
            Hit::Session { id, quote } => {
                let Some(s) = self.workspace.agent_sessions.iter().find(|s| &s.id == id) else {
                    return div().into_any_element();
                };
                let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, s);
                let place = session_space(&self.workspace, s)
                    .and_then(|space| self.space_name(space))
                    .unwrap_or_else(|| home_relative(&s.cwd));
                match quote {
                    Some(q) => (
                        glyph("“"),
                        format!("“{}”", excerpt(q, query, EXCERPT)),
                        format!("{} · {}", self.session_title(s), ago(s.updated, now())),
                    ),
                    None => (
                        dot(group).into_any_element(),
                        self.session_title(s),
                        format!("{place} · {} · {}", group.label(), ago(s.updated, now())),
                    ),
                }
            }
        };
        let two_lines = !query.is_empty() && matches!(hit, Hit::Session { .. });
        let open = hit.clone();
        let line = div()
            .id(gpui::SharedString::from(format!("search-hit-{row}")))
            .mx(px(T::S6))
            .px(px(T::S10))
            .py(px(T::S6))
            .flex()
            .gap(px(T::S10))
            .items_center()
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .when(selected, |d| d.bg(T::color(T::PALETTE_ROW)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && this.search.index != row {
                    this.search.index = row;
                    cx.notify();
                }
            }))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_hit(open.clone(), window, cx)),
            )
            .child(div().w(px(T::S14)).flex().justify_center().child(lead));
        let line = if two_lines {
            line.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(div().truncate().child(highlighted(&title, query)))
                    .child(label(meta).truncate()),
            )
        } else {
            line.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(highlighted(&title, query)),
            )
            .child(label(meta).text_color(T::color(T::MUTED)).flex_shrink_0())
        };
        div()
            .flex()
            .flex_col()
            .when_some(heading, |d, heading| {
                d.child(
                    div()
                        .px(px(T::S16))
                        .pt(px(T::S10))
                        .pb(px(T::S4))
                        .text_size(px(T::META))
                        .text_color(T::color(T::MUTED))
                        .child(heading),
                )
            })
            .child(line)
            .into_any_element()
    }
    fn hit_preview(&self, id: &str, quote: Option<&str>, query: &str) -> Option<gpui::Div> {
        let s = self.workspace.agent_sessions.iter().find(|s| s.id == id)?;
        let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, s);
        let field = |name: &'static str, value: gpui::AnyElement| {
            div()
                .flex()
                .flex_col()
                .gap(px(T::S2))
                .child(
                    div()
                        .text_size(px(T::META))
                        .text_color(T::color(T::MUTED))
                        .child(name),
                )
                .child(value)
        };
        let body = |text: String| {
            div()
                .text_size(px(T::SMALL))
                .text_color(T::color(T::BODY))
                .child(text)
                .into_any_element()
        };
        let mut preview = div()
            .w(px(T::PALETTE_PREVIEW))
            .flex_shrink_0()
            .p(px(T::S14))
            .flex()
            .flex_col()
            .gap(px(T::S12))
            .border_l(px(T::BORDER_WIDTH))
            .border_color(T::color(T::BORDER))
            .child(
                div()
                    .text_size(px(T::CARD_TITLE))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(self.session_title(s)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(T::S6))
                    .child(chip(group))
                    .child(
                        div()
                            .px(px(T::S6))
                            .py(px(T::BORDER_WIDTH))
                            .rounded(px(T::CHIP_RADIUS))
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::SECONDARY))
                            .bg(T::color(T::CHIP))
                            .child(s.runtime.executable()),
                    ),
            );
        if let Some(q) = quote {
            preview = preview.child(field(
                "命中",
                div()
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::BODY))
                    .child(highlighted(
                        &format!("“{}”", excerpt(q, query, EXCERPT * 2)),
                        query,
                    ))
                    .into_any_element(),
            ));
        }
        Some(
            preview
                .child(field(
                    "目录",
                    div()
                        .text_size(px(T::SMALL))
                        .text_color(T::color(T::BODY))
                        .font_family("Menlo")
                        .child(home_relative(&s.cwd))
                        .into_any_element(),
                ))
                .child(field("最后一轮", body(self.session_line(s))))
                .child(field("更新", body(ago(s.updated, now())))),
        )
    }
    fn space_name(&self, id: &str) -> Option<String> {
        self.workspace
            .spaces
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
    }
    /// The top bar only shows where search lives; the palette opens on click or ⌘P.
    pub(super) fn search_button(&self, cx: &Context<Self>) -> gpui::Stateful<gpui::Div> {
        div()
            .id("global-search")
            .role(gpui::accesskit::Role::Button)
            .aria_label("搜索")
            .w(px(T::SEARCH_WIDTH))
            .min_w(px(T::SEARCH_MIN))
            .h(px(T::ROW))
            .px(px(T::S12))
            .flex()
            .items_center()
            .gap(px(T::S8))
            .cursor_pointer()
            .bg(T::color(T::PANEL))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::CONTROL))
            .rounded(px(T::CONTROL_RADIUS))
            .text_color(T::color(T::SECONDARY))
            .hover(|d| d.border_color(T::color(T::FOCUS)))
            .child("⌕")
            .child(div().flex_1().truncate().child("搜索会话、待办或原话"))
            .child(kbd("⌘P"))
            .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx)))
    }
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = true;
        self.search.index = 0;
        self.search.field.update(cx, |f, cx| {
            f.set_text("");
            cx.notify();
        });
        window.focus(&self.search.field.focus_handle(cx), cx);
        cx.notify();
    }
    pub(super) fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search.open = false;
        self.nav.pending_focus = true;
        cx.notify();
    }
}

fn glyph(text: &'static str) -> gpui::AnyElement {
    div()
        .text_color(T::color(T::SECONDARY))
        .child(text)
        .into_any_element()
}
pub(super) fn kbd(text: &'static str) -> gpui::Div {
    div()
        .flex_shrink_0()
        .px(px(T::S6))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::CONTROL))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(T::color(T::SECONDARY))
        .child(text)
}
fn highlighted(text: &str, query: &str) -> gpui::StyledText {
    let style = gpui::HighlightStyle {
        background_color: Some(T::color(T::MATCH)),
        ..Default::default()
    };
    gpui::StyledText::new(text.to_owned())
        .with_highlights(match_ranges(text, query).into_iter().map(|r| (r, style)))
}
