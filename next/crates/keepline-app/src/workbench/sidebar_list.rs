//! The sessions part of the left column, arranged by the chosen preset: 经典, 状态, 目录
//! or a plugin's (`keepline_core::sidebar`). Every preset reads the same `sidebar_status`,
//! so what needs you is the same row in all of them.
use super::*;
use keepline_core::sidebar::{self as presets, Bucket, Detail, Grouping, Preset};

/// A session the column lists: its shown state and whether it still asks for a look.
pub(super) type Listed<'a> = (&'a AgentSession, Group, bool);
/// What a session row ends with: its state (or age, once seen), a text, or nothing.
pub(super) enum End {
    State,
    Text(String),
    Nothing,
}

pub(super) fn bucket(g: Group) -> Bucket {
    match g {
        Group::Waiting | Group::Error => Bucket::Attention,
        Group::Working | Group::Running => Bucket::Running,
        // Ready is a finished turn you have already seen: still yours to check.
        Group::Done | Group::Review | Group::Ready => Bucket::Done,
        Group::Paused => Bucket::Stopped,
        Group::Stale => Bucket::Stale,
    }
}
/// The state a bucket's summary shows its dot in.
fn bucket_group(b: Bucket) -> Group {
    match b {
        Bucket::Attention => Group::Waiting,
        Bucket::Running => Group::Working,
        Bucket::Done => Group::Done,
        Bucket::Stopped => Group::Paused,
        Bucket::Stale => Group::Stale,
    }
}

/// The left column on this machine: the chosen preset, whether it is folded away, and
/// the groups opened or closed against their default, and the pinned terminals. Kept in
/// `<config>/sidebar`.
#[derive(Default)]
pub(super) struct SidebarPrefs {
    pub preset: String,
    pub collapsed: bool,
    pub toggled: HashSet<String>,
    /// WASM presets loaded so far, with their last answers.
    pub wasm: super::sidebar_wasm::Views,
    /// Terminals whose session sits in 置顶, in the order pinned; dropped when it ends.
    pub pinned: Vec<String>,
}
impl SidebarPrefs {
    fn path() -> Option<PathBuf> {
        crate::ui::status_colors::config_dir().map(|d| d.join("sidebar"))
    }
    pub fn load() -> Self {
        let text = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let mut lines = text.lines().map(str::trim);
        let preset = lines.next().unwrap_or_default().to_owned();
        let rest: Vec<&str> = lines.collect();
        Self {
            preset,
            collapsed: rest.contains(&"collapsed"),
            toggled: HashSet::new(),
            wasm: Default::default(),
            pinned: rest
                .iter()
                .filter_map(|l| l.strip_prefix("pin "))
                .map(str::to_owned)
                .collect(),
        }
    }
    /// Best effort: a choice that cannot be written still holds until quit.
    pub fn save(&self) {
        if let Some(path) = Self::path() {
            let _ = path.parent().map(std::fs::create_dir_all);
            let mut body = format!("{}\n", self.preset);
            if self.collapsed {
                body.push_str("collapsed\n");
            }
            for terminal in &self.pinned {
                body.push_str(&format!("pin {terminal}\n"));
            }
            let _ = std::fs::write(path, body);
        }
    }
    pub fn preset(&self) -> &'static Preset {
        presets::find(&self.preset)
    }
    /// Every preset but a flat list (经典) uses the denser look of sidebar-density.html:
    /// the spaces fold into one line, rows carry a state symbol and a time.
    pub fn dense(&self) -> bool {
        self.preset().group != Grouping::Flat
    }
    /// Whether a group is open: its default, flipped once per click.
    pub fn open(&self, key: &str, default: bool) -> bool {
        default != self.toggled.contains(key)
    }
}

/// The last path segment, as a folder is named.
fn folder_name(cwd: &str) -> String {
    Path::new(cwd)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| cwd.to_owned())
}

impl Workbench {
    pub(super) fn toggle_sidebar_group(&mut self, key: &str, cx: &mut Context<Self>) {
        if !self.sidebar.toggled.remove(key) {
            self.sidebar.toggled.insert(key.to_owned());
        }
        cx.notify();
    }
    pub(super) fn choose_sidebar_preset(&mut self, id: &str, cx: &mut Context<Self>) {
        self.sidebar.preset = id.to_owned();
        self.sidebar.toggled.clear();
        self.sidebar.save();
        cx.notify();
    }
    /// The row's dot: pins its terminal to 置顶, or takes it off.
    pub(super) fn toggle_pin(&mut self, terminal: &str, cx: &mut Context<Self>) {
        let pins = &mut self.sidebar.pinned;
        match pins.iter().position(|t| t == terminal) {
            Some(i) => {
                pins.remove(i);
            }
            None => pins.push(terminal.to_owned()),
        }
        self.sidebar.save();
        cx.notify();
    }
    /// A pin lasts as long as its terminal runs: one that ended or was closed comes off.
    /// Only against a workspace the host has sent, never the empty one before it.
    pub(super) fn prune_pins(&mut self) {
        if self.sidebar.pinned.is_empty() || self.workspace.revision == 0 || self.store.cut_off() {
            return;
        }
        let before = self.sidebar.pinned.len();
        let w = &self.workspace;
        self.sidebar.pinned.retain(|id| {
            w.terminals
                .iter()
                .any(|t| &t.id == id && t.state == ProcessState::Running)
        });
        if self.sidebar.pinned.len() != before {
            self.sidebar.save();
        }
    }
    /// ⌘B: the column folds to a rail and back, as Claude's desktop app does.
    pub(super) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar.collapsed = !self.sidebar.collapsed;
        self.sidebar.save();
        cx.notify();
    }
    /// What a row's second line says: what it stopped at (the question it asks you, or
    /// the error), what it is doing in words (`正在改 tabs.rs`, never the raw command), or
    /// how its last turn ended. One line, cut to fit. The pane header's 停在 line reads
    /// the same.
    pub(super) fn row_detail(&self, s: &AgentSession, g: Group) -> Option<String> {
        let first = |t: &str| {
            let line = keepline_core::content::plain_line(t);
            (!line.is_empty()).then(|| line.chars().take(T::TAB_TIP_REASON_CHARS).collect())
        };
        let content = self.workspace.content.get(&s.id);
        match bucket(g) {
            Bucket::Attention => self.attention_reason(s, g).map(|r| format!("停在 {r}")),
            Bucket::Running => content.and_then(|c| c.doing()).map(|d| format!("正在{d}")),
            _ => content.and_then(|c| first(c.last_reply.lines().find(|l| !l.trim().is_empty())?)),
        }
    }
    fn session_row(
        &self,
        (session, g, attention): Listed,
        current: Option<&str>,
        detail: Option<String>,
        end: End,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.sidebar.dense() {
            return self.dense_row((session, g, attention), current, detail, end, cx);
        }
        let open = session.clone();
        let seen = crate::ui::status_colors::StatusColors::get();
        let title = match self
            .layout
            .renaming_session
            .as_ref()
            .filter(|(id, ..)| id == &session.id)
        {
            Some((_, field, _)) => div()
                .flex_1()
                .min_w_0()
                .text_size(px(T::MONO_SIZE))
                .child(field.clone()),
            None => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(T::MONO_SIZE))
                .when(!attention, |d| d.text_color(T::color(seen.seen_title())))
                .child(self.session_title(session)),
        };
        // Rows that all wait for your next step tell apart by how long they have.
        let trailing = label(match end {
            End::Text(text) => text,
            End::Nothing => String::new(),
            End::State if g == Group::Ready && !attention => {
                match age(self.session_updated(session)) {
                    a if a == "刚刚" => a,
                    a => format!("{a}前"),
                }
            }
            End::State => g.label().to_owned(),
        })
        .flex_shrink_0()
        .text_size(px(T::CHIP_SIZE))
        .text_color(if attention {
            g.colors().1
        } else {
            T::color(seen.seen_status())
        });
        let row_group = gpui::SharedString::from(format!("session-row-{}", session.id));
        let pin = self.pin_control(session, &row_group, cx);
        div()
            .id(gpui::SharedString::from(format!("session-{}", session.id)))
            .group(row_group.clone())
            .min_h(px(T::ROW))
            .px(px(T::S8))
            .flex()
            .flex_col()
            .justify_center()
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .when(current == Some(session.id.as_str()), |d| {
                d.bg(T::color(T::raised()))
            })
            .hover(|s| s.bg(T::color(T::raised())))
            .child(
                div()
                    .h(px(T::ROW))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(if attention { dot(g) } else { seen_dot(g) })
                    .child(title)
                    .child(trailing)
                    .children(pin),
            )
            .when_some(detail, |d, text| {
                d.child(
                    div()
                        .mt(-px(T::S4))
                        .mb(px(T::S4))
                        .pl(px(T::DOT + T::S8))
                        .truncate()
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(T::color(T::muted()))
                        .child(text),
                )
            })
            .tooltip(super::diagnostics::tip(self.session_tip(session)))
            // Straight to its pane; an exited one resumes in a new pane.
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.rename_session(&open.id, window, cx);
                        return;
                    }
                    this.peek_or_open(&open, window, cx)
                }),
            )
            .into_any_element()
    }
    /// The row's pin: only a session with a live terminal can be pinned, the pin being
    /// that terminal. Shown on hover, or always once pinned.
    fn pin_control(
        &self,
        session: &AgentSession,
        row_group: &gpui::SharedString,
        cx: &Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let terminal = live_terminal(&self.workspace, session).map(|t| t.id.clone())?;
        let pinned = self.sidebar.pinned.contains(&terminal);
        let row_group = row_group.clone();
        Some(
            div()
                .id(gpui::SharedString::from(format!("pin-{}", session.id)))
                .role(gpui::accesskit::Role::Button)
                .aria_label(if pinned { "取消置顶" } else { "置顶" })
                .flex_shrink_0()
                .w(px(T::S12))
                .flex()
                .justify_center()
                .cursor_pointer()
                .text_size(px(T::CHIP_SIZE))
                .text_color(T::color(if pinned { T::link() } else { T::muted() }))
                .child(if pinned { "●" } else { "○" })
                .when(!pinned, |d| {
                    d.opacity(0.).group_hover(row_group, |s| s.opacity(1.))
                })
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_pin(&terminal, cx);
                })),
        )
    }
    /// A row in the 状态 / 目录 presets (and plugin presets beside them), after
    /// sidebar-density.html: one state symbol, the title in the system font, and on the
    /// right the running turn's clock or a short age (`4m`, `2h`); symbol and time are
    /// monospace. The second line comes only for what needs you and the selected row.
    fn dense_row(
        &self,
        (session, g, attention): Listed,
        current: Option<&str>,
        detail: Option<String>,
        end: End,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let open = session.clone();
        let seen = crate::ui::status_colors::StatusColors::get();
        let selected = current == Some(session.id.as_str());
        let mono = T::mono(cx).family;
        let title = match self
            .layout
            .renaming_session
            .as_ref()
            .filter(|(id, ..)| id == &session.id)
        {
            Some((_, field, _)) => div().flex_1().min_w_0().child(field.clone()),
            None => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(T::UI_FONT)
                .text_size(px(T::SMALL))
                .text_color(if attention || selected {
                    T::color(T::primary())
                } else {
                    T::color(seen.seen_title())
                })
                .child(self.session_title(session)),
        };
        let time = match end {
            End::Text(text) => text,
            End::Nothing => String::new(),
            End::State => match self.turn_started(session) {
                Some(at) => {
                    self.tick_turn(cx);
                    super::layout::turn_clock(now().saturating_sub(at))
                }
                None => short_ago(self.session_updated(session), now()),
            },
        };
        let row_group = gpui::SharedString::from(format!("session-row-{}", session.id));
        let pin = self.pin_control(session, &row_group, cx);
        div()
            .id(gpui::SharedString::from(format!("session-{}", session.id)))
            .group(row_group)
            .px(px(T::S8))
            .py(px(T::S4))
            .flex()
            .flex_col()
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .when(selected, |d| d.bg(T::color(T::raised())))
            .hover(|s| s.bg(T::color(T::raised())))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(
                        // A finished turn you have seen keeps 做完了's ✓, in its quieter ink.
                        glyph(if g == Group::Ready { Group::Done } else { g })
                            .when(g == Group::Ready, |d| d.text_color(g.colors().1))
                            .w(px(T::S12))
                            .flex()
                            .justify_center()
                            .font_family(mono.clone())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(px(T::SMALL)),
                    )
                    .child(title)
                    .child(
                        div()
                            .flex_shrink_0()
                            .font_family(mono)
                            .font_features(T::tabular())
                            .text_size(px(T::CHIP_SIZE))
                            .text_color(T::color(T::muted()))
                            .child(time),
                    )
                    .children(pin),
            )
            .when_some(detail, |d, text| {
                d.child(
                    div()
                        .pl(px(T::S12 + T::S8))
                        .truncate()
                        .font_family(T::UI_FONT)
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(T::color(T::secondary()))
                        .child(text),
                )
            })
            .tooltip(super::diagnostics::tip(self.session_tip(session)))
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.rename_session(&open.id, window, cx);
                        return;
                    }
                    this.peek_or_open(&open, window, cx)
                }),
            )
            .into_any_element()
    }
    /// A group's heading in the 状态 / 目录 presets: small, its count in monospace, no rule.
    pub(super) fn group_title(
        title: String,
        count: Option<usize>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_baseline()
            .gap(px(T::S6))
            .px(px(T::S8))
            .mt(px(T::S14))
            .mb(px(T::S2))
            .text_size(px(T::CHIP_SIZE))
            .child(
                div()
                    .font_family(T::UI_FONT)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(T::color(T::secondary()))
                    .child(title),
            )
            .when_some(count, |d, n| {
                d.child(
                    div()
                        .font_family(T::mono(cx).family)
                        .text_color(T::color(T::muted()))
                        .child(n.to_string()),
                )
            })
    }
    pub(super) fn section_title(title: String, count: usize) -> gpui::Div {
        div()
            .flex()
            .px(px(T::S8))
            .mt(px(T::S12))
            .mb(px(T::S2))
            .child(label(title).flex_1().text_size(px(T::CHIP_SIZE)))
            .child(label(count.to_string()).text_size(px(T::CHIP_SIZE)))
    }
    /// A group's heading in the chosen preset's look.
    pub(super) fn heading(&self, title: String, count: usize, cx: &Context<Self>) -> gpui::Div {
        if self.sidebar.dense() {
            Self::group_title(title, Some(count), cx)
        } else {
            Self::section_title(title, count)
        }
    }
    /// One row standing for folded buckets; a click opens them. Under a heading it reads
    /// `✓ 做完了 4  ‖ 停下了 3  很久没动 165`, each with its state's symbol; inside a folder
    /// it is the quieter `另有 做完了 2 · 停下了 1`.
    fn fold_row(
        &self,
        id: String,
        parts: &[(Bucket, usize)],
        open: bool,
        prefix: &str,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let key = id.clone();
        let mono = T::mono(cx).family;
        let row = div()
            .id(gpui::SharedString::from(format!("sidebar-fold-{id}")))
            .min_h(px(T::ROW))
            .px(px(T::S8))
            .flex()
            .items_center()
            .gap(px(T::S4))
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(T::color(T::raised())))
            .font_family(T::UI_FONT)
            .text_size(px(if prefix.is_empty() {
                T::META
            } else {
                T::CHIP_SIZE
            }))
            .text_color(T::color(if prefix.is_empty() {
                T::secondary()
            } else {
                T::muted()
            }));
        // The parts give way before the chevron does in a narrow column.
        let mut line = div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap_x(px(if prefix.is_empty() { T::S10 } else { T::S6 }));
        if !prefix.is_empty() {
            line = line.child(prefix.to_owned());
        }
        for (i, (b, n)) in parts.iter().enumerate() {
            let count = div()
                .font_family(mono.clone())
                .text_color(T::color(if prefix.is_empty() {
                    T::primary()
                } else {
                    T::muted()
                }))
                .child(n.to_string());
            let mut part = div().flex_shrink_0().flex().items_center().gap(px(T::S4));
            if prefix.is_empty() {
                if *b != Bucket::Stale {
                    part = part.child(glyph(bucket_group(*b)).font_family(mono.clone()));
                }
            } else if i > 0 {
                line = line.child("·");
            }
            line = line.child(part.child(b.label()).child(count));
        }
        row.child(line)
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(T::color(T::muted()))
                    .child(if open { "⌄" } else { "›" }),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_sidebar_group(&key, cx)))
            .into_any_element()
    }
    /// The session rows for the current space, arranged by the chosen preset; returns the
    /// rows, how many sessions were left for 鸟瞰, and whether the closed ones were placed
    /// by the preset (a WASM one) rather than left to the summary under the list.
    pub(super) fn session_list(
        &self,
        sessions: Vec<Listed>,
        closed: &super::sidebar_wasm::Closed,
        current: Option<&str>,
        cx: &Context<Self>,
    ) -> (Vec<gpui::AnyElement>, usize, bool) {
        let preset = self.sidebar.preset();
        let detail = |l: &Listed| match preset.detail {
            Detail::Never => None,
            Detail::Always => self.row_detail(l.0, l.1),
            Detail::Attention => (bucket(l.1) == Bucket::Attention
                || current == Some(l.0.id.as_str()))
            .then(|| self.row_detail(l.0, l.1))
            .flatten(),
        };
        let mut out = vec![];
        // 置顶 comes first in every preset; its rows leave their usual group.
        let pinned_terminal = |l: &Listed| {
            l.0.terminal
                .as_ref()
                .filter(|t| {
                    self.sidebar.pinned.contains(t) && live_terminal(&self.workspace, l.0).is_some()
                })
                .cloned()
        };
        let (mut top, sessions): (Vec<Listed>, Vec<Listed>) = sessions
            .into_iter()
            .partition(|l| pinned_terminal(l).is_some());
        let all_pinned = !top.is_empty() && sessions.is_empty();
        if !top.is_empty() {
            top.sort_by_key(|l| {
                pinned_terminal(l).and_then(|t| self.sidebar.pinned.iter().position(|p| *p == t))
            });
            out.push(
                self.heading("置顶".into(), top.len(), cx)
                    .mt(px(T::S14))
                    .into_any_element(),
            );
            for l in top {
                let d = detail(&l);
                out.push(self.session_row(l, current, d, End::State, cx));
            }
        }
        let mut budget = preset.limit;
        let mut more = 0;
        let mut owns_closed = false;
        let mut push = |out: &mut Vec<gpui::AnyElement>, l: Listed, d: Option<String>, e: End| {
            if budget == 0 && current != Some(l.0.id.as_str()) {
                more += 1;
                return;
            }
            budget = budget.saturating_sub(1);
            out.push(self.session_row(l, current, d, e, cx));
        };
        match preset.group {
            Grouping::Flat if all_pinned => (),
            Grouping::Flat => {
                out.push(
                    Self::section_title("会话".into(), sessions.len())
                        .mt(px(T::S14))
                        .into_any_element(),
                );
                for l in sessions {
                    let d = detail(&l);
                    push(&mut out, l, d, End::State);
                }
            }
            Grouping::Status => {
                let folded: Vec<(Bucket, usize)> = Bucket::ALL
                    .into_iter()
                    .filter(|b| preset.folds(*b))
                    .map(|b| (b, sessions.iter().filter(|l| bucket(l.1) == b).count()))
                    .filter(|(_, n)| *n > 0)
                    .collect();
                let rest_open = self.sidebar.open("status:rest", false);
                for b in Bucket::ALL.into_iter().filter(|b| !preset.folds(*b)) {
                    let rows: Vec<_> = sessions.iter().filter(|l| bucket(l.1) == b).collect();
                    if rows.is_empty() {
                        continue;
                    }
                    out.push(
                        self.heading(b.label().into(), rows.len(), cx)
                            .into_any_element(),
                    );
                    for l in rows {
                        push(&mut out, *l, detail(l), End::State);
                    }
                }
                if !folded.is_empty() {
                    out.push(
                        div()
                            .mt(px(T::S12))
                            .child(self.fold_row("status:rest".into(), &folded, rest_open, "", cx))
                            .into_any_element(),
                    );
                }
                // Opened, in the summary's order: done, stopped, then gone quiet.
                let mut rest: Vec<_> = sessions
                    .iter()
                    .filter(|l| preset.folds(bucket(l.1)))
                    .collect();
                rest.sort_by_key(|l| Bucket::ALL.iter().position(|b| *b == bucket(l.1)));
                for l in rest {
                    // The session you are in stays in sight while its group is folded.
                    if rest_open || current == Some(l.0.id.as_str()) {
                        push(&mut out, *l, detail(l), End::State);
                    }
                }
            }
            Grouping::Wasm => {
                owns_closed =
                    self.wasm_rows(preset, sessions, closed, current, &mut push, &mut out, cx);
            }
            Grouping::Directory => {
                // What needs you is listed once, on top, with its folder in its second
                // line; the folders below only count it.
                let pinned: Vec<_> = sessions
                    .iter()
                    .filter(|l| bucket(l.1) == Bucket::Attention)
                    .collect();
                if !pinned.is_empty() {
                    out.push(
                        self.heading("需要你".into(), pinned.len(), cx)
                            .into_any_element(),
                    );
                    for l in &pinned {
                        let d = detail(l).map(|d| format!("{} · {d}", folder_name(&l.0.cwd)));
                        push(&mut out, **l, d, End::State);
                    }
                }
                // Folders in the order their first session sorts: busy ones first.
                let mut folders: Vec<(&str, Vec<Listed>)> = vec![];
                for l in &sessions {
                    match folders.iter_mut().find(|(cwd, _)| *cwd == l.0.cwd) {
                        Some((_, rows)) => rows.push(*l),
                        None => folders.push((l.0.cwd.as_str(), vec![*l])),
                    }
                }
                // Open folders (something running) first, then those holding what needs
                // you, then the quiet ones; otherwise as their first session sorts.
                folders.sort_by_key(|(_, rows)| {
                    let has = |b: Bucket| rows.iter().any(|l| bucket(l.1) == b);
                    (!has(Bucket::Running), !has(Bucket::Attention))
                });
                if !folders.is_empty() {
                    out.push(Self::group_title("目录".into(), None, cx).into_any_element());
                }
                // A folder's rows sit in from its name.
                let indent = |out: &mut Vec<gpui::AnyElement>, from: usize| {
                    let rows: Vec<_> = out.drain(from..).collect();
                    out.extend(
                        rows.into_iter()
                            .map(|r| div().pl(px(T::S14)).child(r).into_any_element()),
                    );
                };
                for (cwd, rows) in folders {
                    let needs = rows
                        .iter()
                        .filter(|l| bucket(l.1) == Bucket::Attention)
                        .count();
                    let running = rows.iter().any(|l| bucket(l.1) == Bucket::Running);
                    let key = format!("dir:{cwd}");
                    let open = self.sidebar.open(&key, running);
                    out.push(self.folder_row(cwd, &key, rows.len(), needs, open, running, cx));
                    if !open {
                        continue;
                    }
                    let from = out.len();
                    let mut folded: Vec<(Bucket, usize)> = vec![];
                    for l in rows.iter().filter(|l| bucket(l.1) != Bucket::Attention) {
                        let b = bucket(l.1);
                        if preset.folds(b) {
                            match folded.iter_mut().find(|(f, _)| *f == b) {
                                Some((_, n)) => *n += 1,
                                None => folded.push((b, 1)),
                            }
                        } else {
                            push(&mut out, *l, detail(l), End::State);
                        }
                    }
                    folded.sort_by_key(|(b, _)| Bucket::ALL.iter().position(|a| a == b));
                    if !folded.is_empty() {
                        let more_key = format!("dir-more:{cwd}");
                        let more_open = self.sidebar.open(&more_key, false);
                        out.push(self.fold_row(more_key, &folded, more_open, "另有", cx));
                        if more_open {
                            for l in rows.iter().filter(|l| {
                                bucket(l.1) != Bucket::Attention && preset.folds(bucket(l.1))
                            }) {
                                push(&mut out, *l, detail(l), End::State);
                            }
                        }
                    }
                    indent(&mut out, from);
                }
            }
        }
        (out, more, owns_closed)
    }
    /// A folder's heading: its name, `1 需要你` in the error ink when something in it
    /// needs you, and its session count. A folder with nothing running or waiting is
    /// folded and dimmed; its path shows only while the pointer is on it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn folder_row(
        &self,
        cwd: &str,
        key: &str,
        count: usize,
        needs: usize,
        open: bool,
        active: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let key = key.to_owned();
        let quiet = !active && needs == 0;
        let row_group = gpui::SharedString::from(format!("sidebar-{key}"));
        let mono = T::mono(cx).family;
        div()
            .id(row_group.clone())
            .group(row_group.clone())
            .h(px(T::ROW))
            .px(px(T::S8))
            .flex()
            .items_center()
            .gap(px(T::S6))
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .hover(|s| s.bg(T::color(T::raised())))
            .font_family(T::UI_FONT)
            .text_size(px(T::SMALL))
            .child(
                div()
                    .w(px(T::S12))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .child(if open { "▾" } else { "▸" }),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .max_w(px(T::SIDEBAR * 0.6))
                    .truncate()
                    .when(!quiet, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                    .text_color(T::color(if quiet { T::muted() } else { T::primary() }))
                    .child(folder_name(cwd)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .opacity(0.)
                    .group_hover(row_group, |s| s.opacity(1.))
                    .child(home_relative(cwd)),
            )
            .when(needs > 0, |d| {
                d.child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(T::CHIP_SIZE))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(Group::Error.colors().1)
                        .child(format!("{needs} 需要你")),
                )
            })
            .child(
                div()
                    .flex_shrink_0()
                    .font_family(mono)
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_sidebar_group(&key, cx)))
            .into_any_element()
    }
    /// The switch over the list: one segment per preset.
    pub(super) fn preset_switch(&self, cx: &Context<Self>) -> gpui::Div {
        let chosen = self.sidebar.preset().id.clone();
        let mut row = div()
            .flex()
            .gap(px(T::S2))
            .p(px(T::S2))
            .rounded(px(T::CONTROL_RADIUS))
            .bg(T::color(T::chip()));
        for p in presets::all() {
            let id = p.id.clone();
            let on = p.id == chosen;
            row = row.child(
                div()
                    .id(gpui::SharedString::from(format!("sidebar-preset-{}", p.id)))
                    .flex_1()
                    .flex()
                    .justify_center()
                    .py(px(T::S2))
                    .rounded(px(T::CONTROL_RADIUS))
                    .cursor_pointer()
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(if on { T::primary() } else { T::muted() }))
                    .when(on, |d| d.bg(T::color(T::raised())))
                    .child(p.name.clone())
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.choose_sidebar_preset(&id, cx)),
                    ),
            );
        }
        row
    }
}
