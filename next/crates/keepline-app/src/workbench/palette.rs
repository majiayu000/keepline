//! The ⌘P search palette: a floating panel over the workbench with grouped results
//! on the left and the selected session's preview on the right
//! (specs/terminal-workbench/design/palette-here.html).
use super::overview::{quiet, solid};
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Hit {
    NewSession,
    NextWaiting,
    /// A session; `quote` is the latest of the user's own words that matched too.
    Session {
        id: String,
        quote: Option<String>,
    },
    /// The user's own words in a session whose title did not match.
    Quote {
        id: String,
        text: String,
    },
    Task(String),
    /// A plugin action, by its index in `keepline_core::action::all()`, run for the
    /// focused pane.
    Action(usize),
    /// 「再显示 N 个」: the rest of a folded group, `hidden` rows past what shows.
    More {
        section: &'static str,
        hidden: usize,
    },
}
pub(super) struct Section {
    pub title: Option<&'static str>,
    /// What the heading says at its right: the count, sometimes with a note.
    pub note: String,
    /// Every match, including the ones folded away behind 「再显示」.
    pub total: usize,
    pub hits: Vec<Hit>,
}
/// The palette's own state: whether it shows, its query field and the selected row.
pub(super) struct Search {
    open: bool,
    field: Entity<Field>,
    /// The selected row in the palette, counted across all groups.
    index: usize,
    scroll: gpui::ScrollHandle,
    /// The status count picked above an empty query: only that group is listed.
    group: Option<Group>,
    /// Rows a group shows past its first few, by group title, after 「再显示」.
    shown: HashMap<&'static str, usize>,
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
                this.search.shown.clear();
                // A status pick narrows the recent list; typing searches everything.
                if !last_query.trim().is_empty() {
                    this.search.group = None;
                }
                this.search.index = this.first_session_row(last_query.trim());
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
            group: None,
            shown: HashMap::new(),
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
    #[cfg(feature = "visual-test")]
    /// As a click on a status count: a picked group lists no actions, so its first
    /// session is the first row.
    pub(super) fn pick_group(&mut self, group: Option<Group>) {
        self.group = group;
        self.index = 0;
    }
}
/// Rows a group shows before 「再显示」, and how many more each press adds.
const COLLAPSED: usize = 6;
const MORE_STEP: usize = 20;
const EXCERPT: usize = 90;
/// The status counts above an empty query, as the top bar and 鸟瞰 name them.
const COUNTED: [(Group, &str); 4] = [
    (Group::Waiting, "等你"),
    (Group::Error, "出错"),
    (Group::Running, "在跑"),
    (Group::Review, "做完"),
];
/// A live phase and its inbox group count as one: 在跑 is 在跑 either way.
fn counted(group: Group) -> Group {
    match group {
        Group::Working => Group::Running,
        Group::Done | Group::Ready => Group::Review,
        g => g,
    }
}

impl Workbench {
    /// Every session search can show, newest first.
    fn searchable(&self) -> Vec<&AgentSession> {
        let mut visible: Vec<&AgentSession> = self
            .workspace
            .agent_sessions
            .iter()
            .filter(|s| self.session_hidden(s).is_none())
            .collect();
        visible.sort_by_key(|s| std::cmp::Reverse(s.updated));
        visible
    }
    /// The status its row shows, when that is one of the four the palette counts. Only
    /// sessions still open count, as on 鸟瞰.
    fn counted_group(&self, s: &AgentSession) -> Option<Group> {
        let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, s);
        let group = counted(group);
        (s.outcome.needs_review(now()) && COUNTED.iter().any(|(g, _)| *g == group)).then_some(group)
    }
    fn status_counts(&self) -> Vec<(Group, &'static str, usize)> {
        let groups: Vec<Group> = self
            .searchable()
            .into_iter()
            .filter_map(|s| self.counted_group(s))
            .collect();
        COUNTED
            .iter()
            .map(|(g, name)| (*g, *name, groups.iter().filter(|x| *x == g).count()))
            .collect()
    }
    /// A group's first rows and a 「再显示」 row for the rest.
    fn section(&self, title: &'static str, note: String, mut hits: Vec<Hit>) -> Section {
        let total = hits.len();
        let limit = COLLAPSED + self.search.shown.get(title).copied().unwrap_or(0);
        if total > limit {
            hits.truncate(limit);
            hits.push(Hit::More {
                section: title,
                hidden: total - limit,
            });
        }
        Section {
            title: Some(title),
            note,
            total,
            hits,
        }
    }
    /// What the palette lists for `query` (already trimmed): actions, recent sessions by
    /// time and the todos when empty, otherwise matching sessions, quotes and todos.
    pub(super) fn search_sections(&self, query: &str) -> Vec<Section> {
        let visible = self.searchable();
        let session = |s: &AgentSession, quote: Option<String>| Hit::Session {
            id: s.id.clone(),
            quote,
        };
        let mut sections = Vec::new();
        if query.is_empty() {
            let group = self.search.group;
            if group.is_none() {
                let mut actions = vec![Hit::NewSession];
                if !self.needs_you().is_empty() {
                    actions.push(Hit::NextWaiting);
                }
                sections.push(Section {
                    title: None,
                    note: String::new(),
                    total: actions.len(),
                    hits: actions,
                });
            }
            let at = now();
            let mut runs: Vec<(&'static str, Vec<Hit>)> = Vec::new();
            for s in visible
                .into_iter()
                .filter(|s| group.is_none() || self.counted_group(s) == group)
            {
                let title = recency(s.updated, at);
                match runs.last_mut() {
                    Some((last, hits)) if *last == title => hits.push(session(s, None)),
                    _ => runs.push((title, vec![session(s, None)])),
                }
            }
            for (title, hits) in runs {
                sections.push(self.section(title, hits.len().to_string(), hits));
            }
            if group.is_none() && !self.workspace.tasks.is_empty() {
                let tasks: Vec<Hit> = self
                    .workspace
                    .tasks
                    .iter()
                    .map(|t| Hit::Task(t.id.clone()))
                    .collect();
                sections.push(self.section("待办", tasks.len().to_string(), tasks));
            }
            return sections;
        }
        let latest_ask = |s: &AgentSession| {
            self.workspace.content.get(&s.id).and_then(|c| {
                c.asks
                    .iter()
                    .rev()
                    .find(|q| contains(&q.text, query))
                    .map(|q| q.text.clone())
            })
        };
        let mut titled = HashSet::new();
        let sessions: Vec<Hit> = visible
            .iter()
            .filter(|s| {
                // The first ask stays findable after the conversation is renamed.
                let first = self
                    .workspace
                    .content
                    .get(&s.id)
                    .and_then(|c| c.asks.first().filter(|_| c.first_ask_known))
                    .map_or("", |q| q.text.as_str());
                contains(
                    &format!(
                        "{} {first} {} {}",
                        self.session_title(s),
                        s.cli_session_id,
                        s.cwd
                    ),
                    query,
                )
            })
            .map(|s| {
                titled.insert(s.id.as_str());
                session(s, latest_ask(s))
            })
            .collect();
        // A session already listed carries its quote on its own row, not twice.
        let quotes: Vec<Hit> = visible
            .iter()
            .filter(|s| !titled.contains(s.id.as_str()))
            .filter_map(|s| {
                Some(Hit::Quote {
                    id: s.id.clone(),
                    text: latest_ask(s)?,
                })
            })
            .collect();
        let tasks: Vec<Hit> = self
            .workspace
            .tasks
            .iter()
            .filter(|t| contains(&format!("{} {}", t.title, t.description), query))
            .map(|t| Hit::Task(t.id.clone()))
            .collect();
        let pane = self.nav.focused.clone();
        let actions: Vec<Hit> = pane
            .map(|p| self.actions_for(&p))
            .unwrap_or_default()
            .into_iter()
            .filter(|a| contains(&format!("{} {}", a.title, a.command), query))
            .filter_map(|a| {
                keepline_core::action::all()
                    .iter()
                    .position(|b| std::ptr::eq(a, b))
                    .map(Hit::Action)
            })
            .collect();
        for (title, hits) in [
            ("会话", sessions),
            ("原话", quotes),
            ("待办", tasks),
            ("动作", actions),
        ] {
            if !hits.is_empty() {
                let note = match title {
                    "原话" if !titled.is_empty() => format!("{} · 来自其他会话", hits.len()),
                    _ => hits.len().to_string(),
                };
                sections.push(self.section(title, note, hits));
            }
        }
        sections
    }
    /// Where the selection starts: on the first session, so its preview shows at once;
    /// ↑ reaches 新建会话 above it.
    fn first_session_row(&self, query: &str) -> usize {
        self.search_sections(query)
            .iter()
            .flat_map(|s| &s.hits)
            .position(|h| matches!(h, Hit::Session { .. } | Hit::Quote { .. }))
            .unwrap_or(0)
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
        // 「再显示」 unfolds in place; the palette stays open on the same row.
        if let Hit::More { section, .. } = hit {
            *self.search.shown.entry(section).or_default() += MORE_STEP;
            cx.notify();
            return;
        }
        self.search.open = false;
        match hit {
            Hit::NewSession => {
                self.nav.overview = false;
                self.launch(false, None, cx);
            }
            Hit::NextWaiting => self.next_waiting(window, cx),
            Hit::Action(i) => {
                if let (Some(action), Some(pane)) = (
                    keepline_core::action::all().get(i),
                    self.nav.focused.clone(),
                ) {
                    self.run_action(&pane, action, cx);
                }
            }
            Hit::Session { id, .. } | Hit::Quote { id, .. } => {
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
            Hit::More { .. } => {}
        }
        self.nav.pending_focus = true;
        cx.notify();
    }
    /// ↵ opens the selected row; with nothing found it starts a session, as the empty
    /// result's button says.
    pub(super) fn open_selected_hit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_hit(cx) {
            Some(hit) => self.open_hit(hit, window, cx),
            None if !self.search_query(cx).is_empty() => self.open_hit(Hit::NewSession, window, cx),
            None => {}
        }
    }
    fn set_query(&mut self, text: &str, cx: &mut Context<Self>) {
        self.search.field.update(cx, |f, cx| {
            f.set_text(text);
            cx.notify();
        });
    }
    pub(super) fn search_palette(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let query = self.search_query(cx);
        let sections = self.search_sections(&query);
        let total: usize = sections.iter().map(|s| s.hits.len()).sum();
        let index = self.search.index.min(total.saturating_sub(1));
        let selected = sections.iter().flat_map(|s| &s.hits).nth(index).cloned();
        let sessions = self.searchable().len();
        // Each row is one direct child of the list, so the scroll handle can reveal it.
        let mut list = div()
            .id("search-list")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.search.scroll)
            .pt(px(T::S6))
            .pb(px(T::S8))
            .flex()
            .flex_col();
        let mut row = 0;
        for section in &sections {
            for (i, hit) in section.hits.iter().enumerate() {
                let heading = section
                    .title
                    .filter(|_| i == 0)
                    .map(|t| (t, section.note.as_str()));
                list = list.child(self.hit_row(hit, heading, row, row == index, &query, cx));
                row += 1;
            }
        }
        let preview = match &selected {
            Some(Hit::Session { id, quote }) => self.hit_preview(id, quote.as_deref(), &query, cx),
            Some(Hit::Quote { id, text }) => self.hit_preview(id, Some(text), &query, cx),
            _ => None,
        };
        let count = |title: &str| {
            sections
                .iter()
                .find(|s| s.title == Some(title))
                .map_or(0, |s| s.total)
        };
        let found = if query.is_empty() {
            None
        } else if total == 0 {
            Some("0 项".to_owned())
        } else {
            Some(format!(
                "会话 {} · 原话 {} · 待办 {}",
                count("会话"),
                count("原话"),
                count("待办")
            ))
        };
        let field_row = div()
            .h(px(T::SEARCH_FIELD))
            .flex_none()
            .px(px(T::S16))
            .flex()
            .items_center()
            .gap(px(T::S10))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .child(
                div()
                    .text_size(px(T::SEARCH_TEXT))
                    .text_color(T::color(T::secondary()))
                    .child("⌕"),
            )
            .child(div().flex_1().min_w_0().child(self.search.field.clone()))
            .when_some(found, |d, found| {
                d.child(label(found).font_features(T::tabular()))
            })
            .child(kbd("esc"));
        let nothing = !query.is_empty() && total == 0;
        let body = if nothing {
            self.no_results(&query, cx).into_any_element()
        } else {
            div()
                .h(px(T::PALETTE_LIST))
                .flex()
                .min_h_0()
                .child(list)
                .when_some(preview, |d, preview| d.child(preview))
                .into_any_element()
        };
        let keys = |key: &'static str, what: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(T::S4))
                .child(kbd(key))
                .child(what)
        };
        let footer = div()
            .flex_none()
            .px(px(T::S16))
            .py(px(T::S8))
            .flex()
            .items_center()
            .gap(px(T::S14))
            .border_t(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .text_size(px(T::META))
            .text_color(T::color(T::muted()))
            .font_features(T::tabular())
            .map(|d| {
                if nothing {
                    d.child(keys("⌫", "改关键词")).child(keys("↵", "新建会话"))
                } else {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(T::S4))
                            .child(kbd("↑"))
                            .child(kbd("↓"))
                            .child("选择"),
                    )
                    .child(keys("↵", "打开"))
                    .child(keys("⇥", "切换分组"))
                }
            })
            .child(div().flex_1())
            .child(if query.is_empty() {
                format!("只搜索本地已索引的内容 · {sessions} 个会话")
            } else {
                "只搜索本地已索引的内容".into()
            });
        let panel = div()
            .id("search-palette")
            .w(px(T::PALETTE_WIDTH))
            .max_w_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .rounded(px(T::PALETTE_RADIUS))
            .shadow_lg()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event, _, cx| this.search_key(event, cx)))
            .child(field_row)
            .when(query.is_empty(), |d| d.child(self.status_filters(cx)))
            .child(body)
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
                .bg(gpui::rgba(T::scrim()))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.close_search(cx)),
                )
                .child(panel),
        )
        .with_priority(1)
        .into_any_element()
    }
    /// 等你 / 出错 / 在跑 / 做完 with their counts; one narrows the list to that group.
    fn status_filters(&self, cx: &Context<Self>) -> gpui::Div {
        let mut row = div()
            .flex_none()
            .px(px(T::S16))
            .py(px(T::S8))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(T::S6))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .font_features(T::tabular());
        for (group, name, count) in self.status_counts() {
            let picked = self.search.group == Some(group);
            row = row.child(
                div()
                    .id(gpui::SharedString::from(format!("search-status-{name}")))
                    .role(gpui::accesskit::Role::Button)
                    .aria_label(format!("{name} {count} 个，只看这一组"))
                    .h(px(T::FILTER_PILL))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .rounded(px(T::FILTER_PILL_RADIUS))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(if picked { T::focus() } else { T::border() }))
                    .when(picked, |d| d.bg(T::color(T::palette_row())))
                    .when(count == 0 && !picked, |d| d.border_dashed())
                    .text_size(px(T::SMALL))
                    .text_color(T::color(if count == 0 { T::muted() } else { T::body() }))
                    .cursor_pointer()
                    .hover(|d| d.bg(T::color(T::button_hover())))
                    .child(dot(group).when(count == 0, |d| d.opacity(T::ZERO_OPACITY)))
                    .child(name)
                    .child(
                        div()
                            .when(count > 0, |d| {
                                d.font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(T::color(T::primary()))
                            })
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.search.group = (!picked).then_some(group);
                        this.search.shown.clear();
                        this.search.index = this.first_session_row("");
                        this.search.scroll.scroll_to_item(0);
                        cx.notify();
                    })),
            );
        }
        row.child(div().flex_1()).child(
            label(match self.search.group {
                Some(_) => "再点一下看全部",
                None => "还没收尾的会话按状态计数，点一下只看这一组",
            })
            .text_color(T::color(T::muted())),
        )
    }
    /// Nothing matched: what was searched, smaller searches that do find something, and
    /// a session to start instead.
    fn no_results(&self, query: &str, cx: &Context<Self>) -> gpui::Div {
        let visible = self.searchable();
        let asks: usize = visible
            .iter()
            .filter_map(|s| self.workspace.content.get(&s.id))
            .map(|c| c.asks.len())
            .sum();
        let tries: Vec<String> = split_terms(query)
            .into_iter()
            .filter(|t| !self.search_sections(t).is_empty())
            .collect();
        let mut suggestions = div().flex().flex_wrap().justify_center().gap(px(T::S6));
        for term in tries {
            let text = term.clone();
            suggestions = suggestions.child(
                div()
                    .id(gpui::SharedString::from(format!("search-try-{term}")))
                    .role(gpui::accesskit::Role::Button)
                    .h(px(T::FILTER_PILL))
                    .px(px(T::S8))
                    .flex()
                    .items_center()
                    .rounded(px(T::FILTER_PILL_RADIUS))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::body()))
                    .cursor_pointer()
                    .hover(|d| d.bg(T::color(T::button_hover())))
                    .child(format!("只搜「{term}」"))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_query(&text, cx))),
            );
        }
        div()
            .h(px(T::PALETTE_EMPTY))
            .p(px(T::S20))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(T::S12))
            .text_center()
            .child(
                div()
                    .text_size(px(T::CARD_TITLE))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(T::color(T::primary()))
                    .child(format!("没有找到「{query}」")),
            )
            .child(
                div()
                    .max_w(px(T::FORM_WIDTH))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()))
                    .child("标题、会话 ID、目录、你说过的话和待办里都没有这几个字。没被索引的旧历史不会出现在这里。"),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(T::S14))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .font_features(T::tabular())
                    .child(format!("会话标题 {}", visible.len()))
                    .child(format!("原话 {asks} 句"))
                    .child(format!("待办 {}", self.workspace.tasks.len())),
            )
            .child(suggestions)
            .child(
                solid(button("search-empty-new", ""))
                    .aria_label("在当前空间新建会话")
                    .gap(px(T::S6))
                    .child("在当前空间新建会话")
                    .child(kbd("↵").text_color(T::color(T::on_accent())))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_hit(Hit::NewSession, window, cx)
                    })),
            )
    }
    fn hit_row(
        &self,
        hit: &Hit,
        heading: Option<(&'static str, &str)>,
        row: usize,
        selected: bool,
        query: &str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        // Lead icon, title, second line, status at the right, and the time column.
        let mut status: Option<Group> = None;
        let (lead, title, sub, right, when): (gpui::AnyElement, String, String, String, String) =
            match hit {
                Hit::NewSession => {
                    let key = crate::keymap::current(cx)
                        .chords(crate::keymap::Action::NewTab)
                        .first()
                        .map(|c| c.display())
                        .unwrap_or_default();
                    (
                        glyph("＋"),
                        "在当前空间新建会话".into(),
                        String::new(),
                        String::new(),
                        key,
                    )
                }
                Hit::NextWaiting => (
                    glyph("→"),
                    "跳到下一个等你的会话".into(),
                    String::new(),
                    format!("{} 个", self.needs_you().len()),
                    String::new(),
                ),
                Hit::More { section, hidden } => {
                    let unit = if *section == "原话" { "条" } else { "个" };
                    return self
                        .more_row(row, selected, (*hidden).min(MORE_STEP), unit, hit, cx)
                        .into_any_element();
                }
                Hit::Action(i) => {
                    let action = keepline_core::action::all().get(*i);
                    (
                        glyph("▸"),
                        action.map(|a| a.title.clone()).unwrap_or_default(),
                        String::new(),
                        action
                            .map(|a| match a.run {
                                keepline_core::action::Run::Split => "在旁边分屏执行",
                                keepline_core::action::Run::Tab => "在新标签执行",
                                keepline_core::action::Run::Paste => "放进输入行，你按回车",
                            })
                            .unwrap_or_default()
                            .into(),
                        String::new(),
                    )
                }
                Hit::Task(id) => {
                    let task = self.workspace.tasks.iter().find(|t| &t.id == id);
                    (
                        task_icon().into_any_element(),
                        task.map(|t| t.title.clone()).unwrap_or_default(),
                        String::new(),
                        task.map(|t| {
                            format!("{} · {} 条验收", t.agent.executable(), t.acceptance.len())
                        })
                        .unwrap_or_default(),
                        task.and_then(|t| self.space_name(&t.space))
                            .unwrap_or_default(),
                    )
                }
                Hit::Session { id, .. } | Hit::Quote { id, .. } => {
                    let Some(s) = self.workspace.agent_sessions.iter().find(|s| &s.id == id) else {
                        return div().into_any_element();
                    };
                    let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, s);
                    status = Some(group);
                    let when = ago(s.updated, now());
                    match hit {
                        Hit::Quote { text, .. } => (
                            glyph("“"),
                            format!("“{}”", excerpt(text, query, EXCERPT)),
                            self.session_title(s),
                            group.label().into(),
                            when,
                        ),
                        _ => {
                            let place = session_space(&self.workspace, s)
                                .and_then(|space| self.space_name(space))
                                .unwrap_or_else(|| home_relative(&s.cwd));
                            let mut sub = format!("{place} · {}", s.runtime.executable());
                            if let Some(c) = self.workspace.content.get(&s.id)
                                && !query.is_empty()
                            {
                                let said = c.asks.len();
                                let matched =
                                    c.asks.iter().filter(|q| contains(&q.text, query)).count();
                                if matched > 0 {
                                    sub.push_str(&format!(
                                        " · 你说过 {said} 句，{matched} 句含「{query}」"
                                    ));
                                }
                            }
                            (
                                glyph("❯"),
                                self.session_title(s),
                                sub,
                                group.label().into(),
                                when,
                            )
                        }
                    }
                }
            };
        let two_lines = !query.is_empty() && !sub.is_empty();
        let open = hit.clone();
        let text = if two_lines {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_color(T::color(T::primary()))
                        .child(highlighted(&title, query)),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(T::META))
                        .text_color(T::color(T::muted()))
                        .child(sub),
                )
        } else {
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(T::color(T::primary()))
                .child(highlighted(&title, query))
        };
        let line = div()
            .id(gpui::SharedString::from(format!("search-hit-{row}")))
            .mx(px(T::S6))
            .px(px(T::S10))
            .min_h(px(if two_lines {
                T::PALETTE_ROW_TWO
            } else {
                T::PALETTE_ROW
            }))
            .flex()
            .gap(px(T::S10))
            .items_center()
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .when(selected, |d| d.bg(T::color(T::palette_row())))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && this.search.index != row {
                    this.search.index = row;
                    cx.notify();
                }
            }))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_hit(open.clone(), window, cx)),
            )
            .child(
                div()
                    .w(px(T::PALETTE_ICON))
                    .flex_none()
                    .flex()
                    .justify_center()
                    .child(lead),
            )
            .child(text)
            .when(!right.is_empty(), |d| {
                d.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(T::S4))
                        .text_size(px(T::META))
                        .text_color(T::color(T::secondary()))
                        .when_some(status, |d, g| d.child(dot(g)))
                        .child(right),
                )
            })
            .child(
                div()
                    .w(px(T::PALETTE_WHEN))
                    .flex_none()
                    .flex()
                    .justify_end()
                    .truncate()
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .font_features(T::tabular())
                    .map(|d| match hit {
                        Hit::NewSession if !when.is_empty() => d.child(kbd(when)),
                        _ => d.child(when),
                    }),
            );
        div()
            .flex()
            .flex_col()
            .when_some(heading, |d, (heading, note)| {
                d.child(section_heading(heading, note))
            })
            .child(line)
            .into_any_element()
    }
    fn more_row(
        &self,
        row: usize,
        selected: bool,
        next: usize,
        unit: &str,
        hit: &Hit,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let open = hit.clone();
        div()
            .id(gpui::SharedString::from(format!("search-hit-{row}")))
            .mx(px(T::S6))
            .mt(px(T::S2))
            .h(px(T::PALETTE_MORE))
            .pl(px(T::S10 + T::PALETTE_ICON + T::S10))
            .flex()
            .items_center()
            .rounded(px(T::CONTROL_RADIUS))
            .text_size(px(T::SMALL))
            .text_color(T::color(T::link()))
            .font_features(T::tabular())
            .cursor_pointer()
            .when(selected, |d| d.bg(T::color(T::palette_row())))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && this.search.index != row {
                    this.search.index = row;
                    cx.notify();
                }
            }))
            .on_click(
                cx.listener(move |this, _, window, cx| this.open_hit(open.clone(), window, cx)),
            )
            .child(format!("再显示 {next} {unit} ↓"))
    }
    fn hit_preview(
        &self,
        id: &str,
        quote: Option<&str>,
        query: &str,
        cx: &Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        use keepline_core::content::short;
        let s = self.workspace.agent_sessions.iter().find(|s| s.id == id)?;
        let (group, _) = sidebar_status(&self.workspace, &self.attention.unseen, s);
        let content = self.workspace.content.get(&s.id);
        let place = session_space(&self.workspace, s)
            .and_then(|space| self.space_name(space))
            .unwrap_or_else(|| home_relative(&s.cwd));
        let fact = |name: &'static str, value: gpui::AnyElement| {
            div()
                .flex()
                .gap(px(T::S10))
                .text_size(px(T::SMALL))
                .child(
                    div()
                        .w(px(T::PALETTE_FACT))
                        .flex_none()
                        .text_color(T::color(T::muted()))
                        .child(name),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(T::color(T::body()))
                        .line_clamp(3)
                        .child(value),
                )
        };
        let mut facts = div().flex().flex_col().gap(px(T::S6));
        if let Some(q) = quote.filter(|_| !query.is_empty()) {
            facts = facts.child(fact(
                "命中",
                highlighted(&format!("“{}”", excerpt(q, query, EXCERPT * 2)), query)
                    .into_any_element(),
            ));
        }
        let line = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        let ask = match content.map(|c| (c, &c.asks[..])) {
            Some((c, [first, .., last])) => div()
                .child(if c.first_ask_known {
                    short(&line(&first.text), 120)
                } else {
                    format!("（最初要求未覆盖）{}", short(&line(&first.text), 100))
                })
                .child(div().text_color(T::color(T::muted())).child(format!(
                    "追加 {} 句，最后：{}",
                    c.asks.len() - 1,
                    short(&line(&last.text), 40)
                ))),
            Some((_, [only])) => div().child(short(&line(&only.text), 160)),
            _ => div().child(content.map_or("内容尚未索引".into(), |c| c.here().ask)),
        };
        facts = facts.child(fact("你要它", ask.into_any_element()));
        if let Some(c) = content {
            facts = facts.child(fact(
                "停在",
                div().child(c.here().stopped).into_any_element(),
            ));
        }
        let resume = s.clone();
        let command = self.workspace.resume_command(s);
        Some(
            div()
                .id("search-preview")
                .w(px(T::PALETTE_PREVIEW))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .px(px(T::S16))
                .py(px(T::S14))
                .flex()
                .flex_col()
                .gap(px(T::S10))
                .border_l(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .child(
                    div()
                        .text_size(px(T::CARD_TITLE))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(T::color(T::primary()))
                        .line_clamp(2)
                        .child(self.session_title(s)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(T::S6))
                        .text_size(px(T::META))
                        .text_color(T::color(T::muted()))
                        .child(chip(group))
                        .child(format!(
                            "{} · {place} · {}",
                            s.runtime.executable(),
                            ago(s.updated, now())
                        )),
                )
                .child(
                    div()
                        .text_size(px(T::META))
                        .text_color(T::color(T::secondary()))
                        .font_family(T::mono(cx).family)
                        .child(home_relative(&s.cwd)),
                )
                .child(facts)
                .when_some(content.and_then(|c| ask_track(&c.asks)), |d, track| {
                    d.child(track)
                })
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(T::S6))
                        .child(
                            solid(button("search-open", ""))
                                .aria_label("打开")
                                .gap(px(T::S6))
                                .child("打开")
                                .child(kbd("↵").text_color(T::color(T::on_accent())))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.search.open = false;
                                    this.enter_card(&resume, window, cx);
                                    this.nav.pending_focus = true;
                                    cx.notify();
                                })),
                        )
                        .child(quiet(button("search-copy", "复制恢复命令")).on_click(
                            move |_, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    command.clone(),
                                ))
                            },
                        )),
                ),
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
            .bg(T::color(T::panel()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .rounded(px(T::CONTROL_RADIUS))
            .text_color(T::color(T::secondary()))
            .hover(|d| d.border_color(T::color(T::focus())))
            .child("⌕")
            .child(div().flex_1().truncate().child("搜索会话、待办或原话"))
            .child(kbd("⌘P"))
            .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx)))
    }
    pub(super) fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = true;
        self.search.group = None;
        self.search.shown.clear();
        self.search.index = self.first_session_row("");
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
        .text_color(T::color(T::secondary()))
        .child(text)
        .into_any_element()
}
pub(super) fn kbd(text: impl Into<gpui::SharedString>) -> gpui::Div {
    div()
        .flex_shrink_0()
        .px(px(T::S4))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::control()))
        .rounded(px(T::CHIP_RADIUS))
        .text_size(px(T::CHIP_SIZE))
        .text_color(T::color(T::secondary()))
        .child(text.into())
}
fn section_heading(title: &'static str, note: &str) -> gpui::Div {
    div()
        .px(px(T::S16))
        .pt(px(T::S10))
        .pb(px(T::S4))
        .flex()
        .gap(px(T::S6))
        .text_size(px(T::META))
        .text_color(T::color(T::muted()))
        .font_features(T::tabular())
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(T::color(T::secondary()))
                .child(title),
        )
        .child(div().flex_1())
        .child(note.to_owned())
}
/// When the user spoke over the session, one tick per timed ask between the first and
/// the last; nothing for a single ask or asks without times.
fn ask_track(asks: &[keepline_core::content::Quote]) -> Option<gpui::Div> {
    let times: Vec<u64> = asks.iter().map(|q| q.at).filter(|at| *at > 0).collect();
    let (first, last) = (*times.iter().min()?, *times.iter().max()?);
    if times.len() < 2 || last <= first {
        return None;
    }
    let span = last - first;
    // The right margin keeps the last tick inside the track.
    let mut track = div().relative().mr(px(T::S2)).h(px(T::ASK_TRACK)).child(
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(T::S4))
            .h(px(T::S2))
            .rounded(px(T::BORDER_WIDTH))
            .bg(T::color(T::chip())),
    );
    for at in &times {
        let share = (at - first) as f32 / span as f32;
        track = track.child(
            div()
                .absolute()
                .top(px(T::BORDER_WIDTH))
                .left(gpui::relative(share))
                .w(px(T::S2))
                .h(px(T::ASK_TICK))
                .rounded(px(T::BORDER_WIDTH))
                .bg(T::color(T::secondary())),
        );
    }
    let hhmm = |at: u64| clock(at).chars().take(5).collect::<String>();
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .text_size(px(T::META))
            .text_color(T::color(T::muted()))
            .font_features(T::tabular())
            .child(
                div()
                    .flex()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::secondary()))
                            .child(format!("你说过 {} 句", asks.len())),
                    )
                    .child(div().flex_1())
                    .child(format!("共 {}", span_label(span))),
            )
            .child(track)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(hhmm(first))
                    .child(hhmm(last)),
            ),
    )
}
fn highlighted(text: &str, query: &str) -> gpui::StyledText {
    let style = gpui::HighlightStyle {
        background_color: Some(T::color(T::matched())),
        ..Default::default()
    };
    gpui::StyledText::new(text.to_owned())
        .with_highlights(match_ranges(text, query).into_iter().map(|r| (r, style)))
}
