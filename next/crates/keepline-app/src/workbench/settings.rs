//! 设置: a floating panel over the workbench, opened from the sidebar, the Keepline menu
//! or ⌘,. Grouped cards in the manner of macOS System Settings: the left column holds the
//! search and the pages, the right one the page's cards. Every change applies and saves
//! at once; the shortcuts page records a new chord in place.
use super::*;
use crate::keymap::{self, Action, Chord, Keymap, Section};
use crate::ui::{
    app_icon::{AppIcon, Chosen},
    appearance::Appearance,
    terminal_text::{self, TerminalText, UnfocusedCursor},
    theme::{Scheme, mix},
};
use std::path::PathBuf;

/// The chord button keeps one width, recording or not.
const KEY_WIDTH: f32 = T::HERE_LABEL + T::S20;
/// Room kept for 恢复 on every row.
const RESET_WIDTH: f32 = T::S20 * 2. + T::S12;
const NAV_WIDTH: f32 = 212.;
const PANEL_HEIGHT: f32 = T::HERE_HEIGHT + T::CLEANUP_ACTION_HEIGHT;
const SEGMENT_HEIGHT: f32 = 26.;
const SWITCH_WIDTH: f32 = 36.;
const SWITCH_HEIGHT: f32 = 20.;
const TILE_PREVIEW: f32 = 84.;
const ICON_PREVIEW: f32 = 44.;
/// Monospace faces offered when installed; empty is the bundled JetBrains Mono.
const MONO_FACES: [&str; 15] = [
    "",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Fira Code",
    "Cascadia Code",
    "Source Code Pro",
    "IBM Plex Mono",
    "Hack",
    "Iosevka",
    "Roboto Mono",
    "Ubuntu Mono",
    "PT Mono",
    "Andale Mono",
    "Courier New",
];

/// An outlined button: plain enough for a row, still plainly a button.
fn secondary(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button(id, text)
        .flex_shrink_0()
        .h(px(SEGMENT_HEIGHT))
        .px(px(T::S10))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::control()))
        .bg(T::color(T::popover()))
        .text_color(T::color(T::primary()))
}
/// A path under the home folder written from `~`.
fn tilde(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    match path.strip_prefix(&home) {
        Some(rest) if !home.is_empty() => format!("~{rest}"),
        _ => path.to_owned(),
    }
}
/// A card: rows separated by hairlines, as macOS groups its settings.
fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .rounded(px(T::CARD_RADIUS))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::card_border()))
        .bg(T::color(T::popover()))
        .overflow_hidden()
}
fn heading(text: &'static str) -> gpui::Div {
    div()
        .pt(px(T::S20))
        .pb(px(T::S8))
        .px(px(T::S4))
        .text_size(px(T::META))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(T::color(T::secondary()))
        .child(text)
}
/// One segment of a segmented control.
fn segment(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    let text = text.into();
    div()
        .id(id)
        .role(gpui::accesskit::Role::Button)
        .aria_label(text.clone())
        .h(px(SEGMENT_HEIGHT - T::S4))
        .px(px(T::S10))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(T::CONTROL_RADIUS - T::S2))
        .text_size(px(T::SMALL))
        .cursor_pointer()
        .border(px(T::BORDER_WIDTH))
        .map(|d| {
            if on {
                d.bg(T::color(T::popover()))
                    .border_color(T::color(T::control()))
                    .text_color(T::color(T::primary()))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .shadow_sm()
            } else {
                d.border_color(gpui::transparent_black())
                    .text_color(T::color(T::secondary()))
                    .hover(|s| s.text_color(T::color(T::primary())))
            }
        })
        .child(text)
}
fn segments() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(T::S2))
        .p(px(T::S2))
        .rounded(px(T::CONTROL_RADIUS))
        .bg(T::color(T::chip()))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::border()))
}
/// An on/off switch; the knob sits on the side it means.
fn switch(id: impl Into<gpui::ElementId>, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .role(gpui::accesskit::Role::Switch)
        .aria_label(if on { "已开启" } else { "已关闭" })
        .w(px(SWITCH_WIDTH))
        .h(px(SWITCH_HEIGHT))
        .flex_shrink_0()
        .p(px(T::S2))
        .flex()
        .items_center()
        .when(on, |d| d.justify_end())
        .rounded_full()
        .cursor_pointer()
        .bg(T::color(if on { T::accent() } else { T::control() }))
        .child(
            div()
                .size(px(SWITCH_HEIGHT - T::S4))
                .rounded_full()
                .bg(T::color(if on { T::on_accent() } else { T::popover() }))
                .shadow_sm(),
        )
}
/// A chord drawn as a key cap.
fn keycap(text: impl Into<gpui::SharedString>, muted: bool) -> gpui::Div {
    div()
        .h(px(SEGMENT_HEIGHT - T::S2))
        .px(px(T::S8))
        .flex()
        .items_center()
        .rounded(px(T::CHIP_RADIUS + T::S2))
        .border(px(T::BORDER_WIDTH))
        .border_b(px(T::BORDER_WIDTH * 2.))
        .border_color(T::color(T::control()))
        .bg(T::color(T::popover()))
        .text_size(px(T::SMALL))
        .text_color(T::color(if muted { T::muted() } else { T::primary() }))
        .child(text.into())
}
/// A small window drawn in one scheme's colors, for the theme tiles.
fn window_sketch(scheme: Scheme) -> gpui::Div {
    let (bg, fg) = (scheme.bg(), scheme.fg());
    let line = |share: u32, width: f32| {
        div()
            .h(px(T::S4))
            .w(px(width))
            .rounded_full()
            .bg(T::color(mix(bg, fg, share)))
    };
    div()
        .flex_1()
        .h_full()
        .flex()
        .bg(T::color(bg))
        .child(
            div()
                .w(px(T::S20 + T::S4))
                .h_full()
                .p(px(T::S6))
                .flex()
                .flex_col()
                .gap(px(T::S4))
                .bg(T::color(mix(bg, fg, 45)))
                .child(line(400, T::S12))
                .child(line(250, T::S10))
                .child(line(250, T::S12)),
        )
        .child(
            div()
                .flex_1()
                .p(px(T::S8))
                .flex()
                .flex_col()
                .gap(px(T::S6))
                .child(line(650, T::S20 * 2.))
                .child(line(300, T::S20 * 3.))
                .child(line(300, T::S20 * 2.5))
                .child(
                    div()
                        .h(px(T::S6))
                        .w(px(T::S20))
                        .rounded_full()
                        .bg(Group::Running.colors().0),
                ),
        )
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    General,
    Look,
    Terminal,
    Keys,
    Notifications,
    Plugins,
    About,
}
impl Page {
    const ALL: [Self; 7] = [
        Self::General,
        Self::Look,
        Self::Terminal,
        Self::Keys,
        Self::Notifications,
        Self::Plugins,
        Self::About,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::General => "通用",
            Self::Look => "外观",
            Self::Terminal => "终端",
            Self::Keys => "快捷键",
            Self::Notifications => "通知",
            Self::Plugins => "插件",
            Self::About => "关于",
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Self::General => "≡",
            Self::Look => "◐",
            Self::Terminal => "›_",
            Self::Keys => "⌘",
            Self::Notifications => "◉",
            Self::Plugins => "⧉",
            Self::About => "i",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::General => "新会话用哪个 Agent，以及侧栏怎样整理旧会话。",
            Self::Look => "窗口明暗与应用图标，选中即生效。",
            Self::Terminal => "终端文字、光标与分屏，上方实时预览。",
            Self::Keys => "点击组合键重新录制，Esc 取消。",
            Self::Notifications => "Agent 需要你的时候，用系统通知或提示音叫你回来。",
            Self::Plugins => {
                "哪些插件和声明文件生效了，哪些没有、为什么。改动后重开 Keepline 生效。"
            }
            Self::About => "版本信息，以及 Keepline 把数据和偏好存在哪里。",
        }
    }
}
/// Every searchable setting: its page, its title and the words that find it.
const ENTRIES: [(Page, &str, &str); 19] = [
    (
        Page::General,
        "默认 Agent",
        "新建待办 会话 codex claude grok",
    ),
    (
        Page::General,
        "回来时显示总览",
        "离开 返回 工作进展 分钟 overview",
    ),
    (Page::General, "旧会话收起时间", "已关闭 侧栏 收起 历史 天"),
    (Page::Look, "主题", "跟随系统 亮色 暗色 明暗 dark light"),
    (Page::Look, "界面样式", "样式 标准 终端原生 tmux 等宽 style"),
    (Page::Look, "应用图标", "dock 图标 水墨 吉祥物 icon"),
    (Page::Terminal, "终端字体", "字体 family font mono 等宽"),
    (Page::Terminal, "字号", "文字大小 font size 缩放 pt"),
    (Page::Terminal, "行距", "行高 line height 紧凑 舒适 宽松"),
    (Page::Terminal, "光标闪烁", "光标 闪烁 blink cursor"),
    (Page::Terminal, "非当前分屏变暗", "分屏 变暗 焦点 dim split"),
    (
        Page::Terminal,
        "非当前分屏的光标",
        "分屏 光标 空心 焦点 cursor split",
    ),
    (
        Page::Notifications,
        "桌面通知",
        "提醒 等待 错误 macOS 权限 notification",
    ),
    (
        Page::Notifications,
        "提示音",
        "提醒 声音 sound 不开通知也会响",
    ),
    (
        Page::Plugins,
        "插件",
        "plugin 扩展 声明 agent json 动作 验证规则 左栏 sidebar",
    ),
    (Page::About, "版本", "关于 keepline version"),
    (Page::About, "运行数据", "数据目录 数据库 runtime 后台 访达"),
    (
        Page::About,
        "Agent Hook",
        "hook hooks claude settings.json 过期 重新安装 对上对话",
    ),
    (
        Page::About,
        "偏好文件",
        "配置 config colors.json 状态颜色 访达",
    ),
];
pub(super) enum SettingsAction {
    Close,
    Change(Settings),
}
pub(super) struct SettingsView {
    pub(super) focus: gpui::FocusHandle,
    pub(super) page: Page,
    pub(super) values: Settings,
    pub(super) busy: bool,
    pub(super) save_error: Option<String>,
    text_error: Option<String>,
    pub(super) query: Entity<Field>,
    /// The workspace's runtime directory, shown on 关于.
    runtime: Option<PathBuf>,
    /// The action waiting for its new chord.
    pub(super) recording: Option<Action>,
    /// A recorded chord another action already has: it moves only when confirmed.
    pub(super) taken: Option<(Action, Chord, Action)>,
    /// Why the last recorded chord was refused, shown under its row.
    pub(super) refused: Option<(Action, &'static str)>,
    /// The Agent CLIs' hook entries, read once when the panel opens; None while reading.
    hooks: Option<Result<Vec<crate::agent_hooks::Hook>, String>>,
    /// The CLI whose hooks are being installed after a click.
    installing: Option<String>,
}
impl gpui::EventEmitter<SettingsAction> for SettingsView {}
impl SettingsView {
    pub(super) fn new(
        values: Settings,
        busy: bool,
        runtime: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| {
            Field::new("搜索设置", "", cx)
                .compact()
                .placeholder("搜索设置")
        });
        cx.observe(&query, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&query, |_, _, event, cx| {
            if matches!(event, crate::ui::field::FieldEvent::Cancel) {
                cx.emit(SettingsAction::Close);
            }
        })
        .detach();
        Self::check_hooks(None, cx);
        Self {
            focus: cx.focus_handle(),
            page: Page::General,
            values,
            busy,
            save_error: None,
            text_error: None,
            query,
            runtime,
            recording: None,
            taken: None,
            refused: None,
            hooks: None,
            installing: None,
        }
    }
    /// Installs `agent`'s hooks first when given (only ever from a click), then reads every
    /// CLI's again. Both run in the background: they spawn `keeplined` and read small files.
    fn check_hooks(agent: Option<String>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if let Some(agent) = agent {
                        crate::agent_hooks::install(&agent)?;
                    }
                    crate::agent_hooks::check()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.hooks = Some(result.map_err(|e| e.to_string()));
                this.installing = None;
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn record(&mut self, action: Action, cx: &mut Context<Self>) {
        self.page = Page::Keys;
        self.recording = Some(action);
        self.taken = None;
        self.refused = None;
        cx.notify();
    }
    fn key(&mut self, k: &gpui::Keystroke, cx: &mut Context<Self>) {
        let Some(action) = self.recording else {
            match keymap::action(k, cx) {
                _ if k.key == "escape" => cx.emit(SettingsAction::Close),
                Some(Action::Close | Action::Settings) => cx.emit(SettingsAction::Close),
                _ => (),
            }
            return;
        };
        if k.key == "escape" {
            self.recording = None;
            cx.notify();
            return;
        }
        // A modifier on its own is the start of a chord, not one.
        let Some(chord) = Chord::from_keystroke(k) else {
            return;
        };
        self.recording = None;
        let mut map = keymap::current(cx);
        if let Some(why) = chord.refusal() {
            self.refused = Some((action, why));
        } else if let Some(holder) = map.holder(action, &chord) {
            self.taken = Some((action, chord, holder));
        } else {
            map.assign(action, chord);
            keymap::update(map, cx);
        }
        cx.notify();
    }
    /// Gives the taken chord to the action that asked for it.
    pub(super) fn take(&mut self, cx: &mut Context<Self>) {
        if let Some((action, chord, _)) = self.taken.take() {
            let mut map = keymap::current(cx);
            map.assign(action, chord);
            keymap::update(map, cx);
        }
        cx.notify();
    }
    pub(super) fn reset(&mut self, action: Option<Action>, cx: &mut Context<Self>) {
        let map = match action {
            Some(action) => {
                let mut map = keymap::current(cx);
                map.reset(action);
                map
            }
            None => Keymap::default(),
        };
        self.taken = None;
        self.refused = None;
        keymap::update(map, cx);
        cx.notify();
    }
    /// Sends a workspace setting to the host; nothing while the last one is saving.
    fn change(&self, cx: &mut Context<Self>, edit: impl FnOnce(&mut Settings)) {
        if self.busy {
            return;
        }
        let mut values = self.values.clone();
        edit(&mut values);
        cx.emit(SettingsAction::Change(values));
    }
    fn nav_item(&self, page: Page, searching: bool, cx: &Context<Self>) -> impl IntoElement {
        let on = self.page == page && !searching;
        let dim = searching && !self.page_matches(page, cx);
        div()
            .id(page.label())
            .role(gpui::accesskit::Role::Tab)
            .aria_label(page.label())
            .h(px(T::ROW + T::S2))
            .px(px(T::S8))
            .flex()
            .items_center()
            .gap(px(T::S10))
            .rounded(px(T::CONTROL_RADIUS))
            .cursor_pointer()
            .text_size(px(T::BODY_SIZE))
            .text_color(T::color(if dim { T::muted() } else { T::body() }))
            .map(|d| {
                if on {
                    d.bg(T::color(T::raised()))
                        .text_color(T::color(T::primary()))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                } else {
                    d.hover(|s| s.bg(T::color(T::button_hover())))
                }
            })
            .child(
                div()
                    .size(px(T::ICON_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(T::CONTROL_RADIUS))
                    .text_size(px(T::SMALL))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .map(|d| {
                        if on {
                            d.bg(T::color(T::accent()))
                                .text_color(T::color(T::on_accent()))
                        } else {
                            d.bg(T::color(T::chip()))
                                .text_color(T::color(T::secondary()))
                        }
                    })
                    .child(page.glyph()),
            )
            .child(page.label())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.page = page;
                this.recording = None;
                this.taken = None;
                this.refused = None;
                this.query.update(cx, |field, cx| {
                    field.set_text("");
                    cx.notify();
                });
                cx.notify();
            }))
    }
    fn query(&self, cx: &gpui::App) -> String {
        self.query.read(cx).text.trim().to_lowercase()
    }
    fn matches(&self, page: Page, title: &str, note: &str, cx: &gpui::App) -> bool {
        let query = self.query(cx);
        query.is_empty()
            || format!("{} {title} {note}", page.label())
                .to_lowercase()
                .contains(&query)
    }
    /// Whether the setting called `title` on `page` answers the search.
    fn shows(&self, page: Page, title: &str, cx: &gpui::App) -> bool {
        let words = ENTRIES
            .iter()
            .find(|(p, t, _)| *p == page && *t == title)
            .map_or("", |(_, _, words)| words);
        self.matches(page, title, words, cx)
    }
    fn page_matches(&self, page: Page, cx: &gpui::App) -> bool {
        match page {
            Page::Keys => {
                Keymap::actions().any(|a| self.matches(page, &a.label(), a.section().label(), cx))
                    || self.matches(page, "切换到第几个标签", "⌘1 ⌘9", cx)
            }
            _ => ENTRIES
                .iter()
                .any(|(p, title, _)| *p == page && self.shows(page, title, cx)),
        }
    }
    /// A titled row inside a card; every row after the first gets a hairline above.
    fn row(&self, first: bool, title: &str, note: &str, control: impl IntoElement) -> gpui::Div {
        div()
            .min_h(px(T::TASK_ROW + T::S6))
            .px(px(T::S16))
            .py(px(T::S12))
            .flex()
            .items_center()
            .gap(px(T::S16))
            .when(!first, |d| {
                d.border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(T::S2))
                    .child(
                        div()
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .child(title.to_owned()),
                    )
                    .when(!note.is_empty(), |d| d.child(label(note.to_owned()))),
            )
            .child(div().flex_shrink_0().child(control))
    }
    /// Cards and their headings, keeping only rows the search leaves.
    fn cards(&self, groups: Vec<(&'static str, Vec<gpui::AnyElement>)>) -> gpui::Div {
        let mut page = div().flex().flex_col();
        for (title, rows) in groups {
            if rows.is_empty() {
                continue;
            }
            page = page.child(heading(title)).child(card().children(rows));
        }
        page
    }
    fn general_page(&self, cx: &Context<Self>) -> gpui::Div {
        let mut start = Vec::new();
        if self.shows(Page::General, "默认 Agent", cx) {
            let mut choices = segments();
            for agent in Agent::all() {
                choices = choices.child(
                    segment(
                        format!("agent-{}", agent.name()),
                        agent.name(),
                        self.values.default_agent == agent,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change(cx, |v| v.default_agent = agent)
                    })),
                );
            }
            start.push(
                self.row(
                    start.is_empty(),
                    "默认 Agent",
                    "新建待办和未指定 Agent 时用它",
                    choices,
                )
                .into_any_element(),
            );
        }
        let mut tidy = Vec::new();
        for (away, title, note, options, selected) in [
            (
                true,
                "回来时显示总览",
                "离开窗口超过这段时间，回来先看各会话进展",
                &[5, 15, 30, 60][..],
                self.values.away_minutes,
            ),
            (
                false,
                "旧会话收起时间",
                "已关闭的会话过了这段时间从侧栏收起，历史仍保留",
                &[1, 3, 7, 14][..],
                self.values.stale_days,
            ),
        ] {
            if !self.shows(Page::General, title, cx) {
                continue;
            }
            // Show an existing custom value even when it is not one of the presets.
            let mut numbers = options.to_vec();
            if !numbers.contains(&selected) {
                numbers.push(selected);
                numbers.sort_unstable();
            }
            let mut choices = segments();
            for value in numbers {
                let text = format!("{value} {}", if away { "分钟" } else { "天" });
                choices = choices.child(
                    segment(format!("{away}-{value}"), text, value == selected).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.change(cx, |v| {
                                if away {
                                    v.away_minutes = value;
                                } else {
                                    v.stale_days = value;
                                }
                            })
                        }),
                    ),
                );
            }
            tidy.push(
                self.row(tidy.is_empty(), title, note, choices)
                    .into_any_element(),
            );
        }
        self.cards(vec![("启动", start), ("会话整理", tidy)])
    }
    /// A selectable tile: a picture above, its name below.
    fn tile(
        &self,
        id: impl Into<gpui::ElementId>,
        name: &'static str,
        on: bool,
        picture: impl IntoElement,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .role(gpui::accesskit::Role::RadioButton)
            .aria_label(name)
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(T::S8))
            .cursor_pointer()
            .child(
                div()
                    .w_full()
                    .h(px(TILE_PREVIEW))
                    .flex()
                    .overflow_hidden()
                    .rounded(px(T::CONTROL_RADIUS + T::S2))
                    .border(px(T::FOCUS_RING))
                    .border_color(T::color(if on { T::accent() } else { T::border() }))
                    .child(picture),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(if on { T::primary() } else { T::secondary() }))
                    .when(on, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                    .child(
                        div()
                            .size(px(T::DOT + T::S4))
                            .rounded_full()
                            .border(px(if on { T::S4 } else { T::BORDER_WIDTH }))
                            .border_color(T::color(if on { T::accent() } else { T::control() })),
                    )
                    .child(name),
            )
    }
    fn look_page(&self, cx: &Context<Self>) -> gpui::Div {
        let mut theme = Vec::new();
        if self.shows(Page::Look, "主题", cx) {
            let chosen = cx.try_global::<Appearance>().copied().unwrap_or_default();
            let mut tiles = div().flex().gap(px(T::S16)).p(px(T::S16));
            for (i, look) in Appearance::ALL.into_iter().enumerate() {
                let picture = match look {
                    Appearance::Light => div().flex_1().flex().child(window_sketch(Scheme::Light)),
                    Appearance::Dark => div().flex_1().flex().child(window_sketch(Scheme::Dark)),
                    Appearance::System => div()
                        .flex_1()
                        .flex()
                        .child(window_sketch(Scheme::Light))
                        .child(window_sketch(Scheme::Dark)),
                };
                tiles = tiles.child(
                    self.tile(format!("look-{i}"), look.label(), look == chosen, picture)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.text_error = crate::ui::appearance::choose(look, cx).err();
                            crate::set_menus(cx);
                            cx.notify();
                        })),
                );
            }
            theme.push(tiles.into_any_element());
            theme.push(
                div()
                    .px(px(T::S16))
                    .pb(px(T::S12))
                    .child(label("窗口与终端一起切换；跟随系统时随 macOS 自动变换"))
                    .into_any_element(),
            );
        }
        let mut style = Vec::new();
        if self.shows(Page::Look, "界面样式", cx) {
            use crate::ui::style::{self, Style};
            let chosen = style::current(cx);
            let mut choices = segments();
            for (i, look) in Style::ALL.into_iter().enumerate() {
                choices = choices.child(
                    segment(format!("style-{i}"), look.label(), look == chosen).on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.text_error = style::choose(look, cx).err();
                            cx.notify();
                        }),
                    ),
                );
            }
            style.push(
                self.row(
                    true,
                    "界面样式",
                    "标准：现在的布局去掉重复；终端原生：外壳退成细线，等宽字，标签在底部",
                    choices,
                )
                .into_any_element(),
            );
        }
        let mut icon = Vec::new();
        if self.shows(Page::Look, "应用图标", cx) {
            let current = cx.global::<Chosen>().icon;
            let mut tiles = div().flex().gap(px(T::S16)).p(px(T::S16));
            for choice in AppIcon::ALL {
                let picture = div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(T::color(T::raised()))
                    .child(gpui::img(choice.title_image()).size(px(ICON_PREVIEW)));
                tiles = tiles.child(
                    self.tile(
                        format!("icon-{}", choice.label()),
                        choice.label(),
                        choice == current,
                        picture,
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        layout::choose_icon(choice, cx);
                        cx.refresh_windows();
                        cx.notify();
                    })),
                );
            }
            // Keep the two tiles the width of the three theme tiles.
            icon.push(tiles.child(div().flex_1()).into_any_element());
            icon.push(
                div()
                    .px(px(T::S16))
                    .pb(px(T::S12))
                    .child(label("标题栏和程序坞一起更换，下次启动仍保留"))
                    .into_any_element(),
            );
        }
        self.cards(vec![
            ("主题", theme),
            ("界面样式", style),
            ("应用图标", icon),
        ])
    }
    fn set_text(&mut self, text: TerminalText, cx: &mut Context<Self>) {
        self.text_error = terminal_text::update(text).err();
        cx.refresh_windows();
        cx.notify();
    }
    /// A segmented control over one terminal choice; `pick` writes it.
    fn text_choices<V: Copy + 'static>(
        &self,
        id: &'static str,
        options: &[(V, &'static str)],
        on: impl Fn(V) -> bool,
        pick: fn(&mut TerminalText, V),
        cx: &Context<Self>,
    ) -> gpui::Div {
        let mut choices = segments();
        for (i, &(value, name)) in options.iter().enumerate() {
            choices = choices.child(segment(format!("{id}-{i}"), name, on(value)).on_click(
                cx.listener(move |this, _, _, cx| {
                    let mut text = terminal_text::current();
                    pick(&mut text, value);
                    this.set_text(text, cx);
                }),
            ));
        }
        choices
    }
    fn font_list(&self, text: &TerminalText, cx: &Context<Self>) -> gpui::Div {
        let installed = cx.text_system().all_font_names();
        let mut list = div().flex().flex_col();
        for (i, family) in MONO_FACES
            .into_iter()
            .filter(|f| f.is_empty() || installed.iter().any(|n| n == f))
            .enumerate()
        {
            let on = text.family == family;
            let name = if family.is_empty() {
                "JetBrains Mono"
            } else {
                family
            };
            list = list.child(
                div()
                    .id(format!("font-{family}"))
                    .role(gpui::accesskit::Role::RadioButton)
                    .aria_label(name)
                    .h(px(T::TODO_HEIGHT + T::S2))
                    .px(px(T::S16))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .cursor_pointer()
                    .when(i > 0, |d| {
                        d.border_t(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                    })
                    .map(|d| {
                        if on {
                            d.bg(T::color(T::raised()))
                        } else {
                            d.hover(|s| s.bg(T::color(T::button_hover())))
                        }
                    })
                    .child(
                        div()
                            .w(px(T::S12))
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::primary()))
                            .child(if on { "✓" } else { "" }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap(px(T::S8))
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .when(on, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                            .child(name)
                            .when(family.is_empty(), |d| d.child(label("内置 · 默认"))),
                    )
                    .child(
                        div()
                            .font_family(name)
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::secondary()))
                            .child("Aa 0O il1 => {}"),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.family = family.into();
                        this.set_text(text, cx);
                    })),
            );
        }
        list
    }
    fn preview(&self, text: &TerminalText) -> gpui::Div {
        let dot = |hue: u32| div().size(px(T::S10)).rounded_full().bg(T::color(hue));
        div()
            .flex()
            .flex_col()
            .rounded(px(T::CARD_RADIUS))
            .overflow_hidden()
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::card_border()))
            .bg(T::color(T::terminal_bg()))
            .child(
                div()
                    .h(px(T::PANE_HEADER - T::S4))
                    .px(px(T::S12))
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(dot(Group::Error.hue()))
                    .child(dot(Group::Paused.hue()))
                    .child(dot(Group::Running.hue()))
                    .child(div().flex_1())
                    .child(label(format!(
                        "{} · {} pt · {}×",
                        if text.family.is_empty() {
                            "JetBrains Mono"
                        } else {
                            &text.family
                        },
                        text.size,
                        text.line_height
                    ))),
            )
            .child(
                div().p(px(T::S16)).child(
                    gpui::canvas(
                        |_, _, _| (),
                        |bounds, _, window, cx| {
                            use crate::terminal::screen::{self, Run};
                            let font = T::terminal_mono(cx);
                            let Some(metrics) = screen::CellMetrics::measure(&font, window) else {
                                return;
                            };
                            let fg = T::terminal_fg();
                            let green = Group::Running.hue();
                            let blue = Group::Review.hue();
                            let quiet = mix(T::terminal_bg(), fg, 550);
                            let line = |parts: &[(&str, u32)]| {
                                let mut col = 0;
                                parts
                                    .iter()
                                    .map(|&(text, color)| {
                                        let cols = text
                                            .chars()
                                            .map(|c| if c.is_ascii() { 1 } else { 2 })
                                            .sum();
                                        let run = Run {
                                            col,
                                            cols,
                                            text: text.into(),
                                            fg: color,
                                            bg: None,
                                            attrs: screen::Attrs::default(),
                                        };
                                        col += cols;
                                        run
                                    })
                                    .collect::<Vec<_>>()
                            };
                            let preview = screen::Screen {
                                background: T::terminal_bg(),
                                rows: vec![
                                    line(&[
                                        ("~/workspace", blue),
                                        (" on ", quiet),
                                        ("main", green),
                                    ]),
                                    line(&[
                                        ("$ ", quiet),
                                        ("codex", fg),
                                        (" resume --last", quiet),
                                    ]),
                                    line(&[("▸ 准备继续你的工作", fg), ("  0123456789", quiet)]),
                                    line(&[("✓ ", green), ("3 files changed, 0 errors", fg)]),
                                ],
                                ..Default::default()
                            };
                            screen::paint_cached(
                                &preview,
                                &font,
                                bounds,
                                metrics,
                                screen::CursorPaint::Hidden,
                                &mut screen::LayoutCache::default(),
                                window,
                                cx,
                            );
                        },
                    )
                    .w_full()
                    .h(px(text.size * text.line_height * 4.6)),
                ),
            )
    }
    fn terminal_page(&self, cx: &Context<Self>) -> gpui::Div {
        let text = terminal_text::current();
        let shows = |title| self.shows(Page::Terminal, title, cx);
        let mut page = div().flex().flex_col();
        if shows("终端字体") || shows("字号") || shows("行距") {
            page = page.child(heading("预览")).child(self.preview(&text));
        }
        let mut size = Vec::new();
        if shows("字号") {
            let stepper = segments()
                .child(segment("font-smaller", "−", false).on_click(cx.listener(
                    |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.size = (text.size - 1.).max(8.);
                        this.set_text(text, cx);
                    },
                )))
                .child(
                    div()
                        .w(px(RESET_WIDTH))
                        .text_center()
                        .text_size(px(T::SMALL))
                        .text_color(T::color(T::primary()))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(format!("{} pt", text.size)),
                )
                .child(segment("font-larger", "+", false).on_click(cx.listener(
                    |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.size = (text.size + 1.).min(36.);
                        this.set_text(text, cx);
                    },
                )));
            size.push(
                self.row(
                    size.is_empty(),
                    "字号",
                    "与 ⌘+ / ⌘− 缩放共用，重开应用后保留",
                    stepper,
                )
                .into_any_element(),
            );
        }
        if shows("行距") {
            let line_height = text.line_height;
            let choices = self.text_choices(
                "line",
                &[
                    (1., "紧凑 1.0×"),
                    (1.2, "标准 1.2×"),
                    (1.4, "舒适 1.4×"),
                    (1.6, "宽松 1.6×"),
                ],
                |h: f32| (line_height - h).abs() < 0.01,
                |t, h| t.line_height = h,
                cx,
            );
            size.push(
                self.row(
                    size.is_empty(),
                    "行距",
                    "相对字体自然行高，文字始终居中",
                    choices,
                )
                .into_any_element(),
            );
        }
        let mut font = Vec::new();
        if shows("终端字体") {
            font.push(self.font_list(&text, cx).into_any_element());
        }
        let mut cursor = Vec::new();
        if shows("光标闪烁") {
            let blink = text.cursor_blink;
            let choices = self.text_choices(
                "blink",
                &[(true, "闪烁"), (false, "常亮")],
                |b| b == blink,
                |t, b| t.cursor_blink = b,
                cx,
            );
            cursor.push(
                self.row(
                    cursor.is_empty(),
                    "光标闪烁",
                    "当前分屏的光标；程序要求常亮时照办，打字时保持亮着",
                    choices,
                )
                .into_any_element(),
            );
        }
        if shows("非当前分屏变暗") {
            let dim = text.unfocused_dim;
            let choices = self.text_choices(
                "dim",
                &[(0., "不变暗"), (0.15, "轻"), (0.3, "明显")],
                |d: f32| (dim - d).abs() < 0.01,
                |t, d| t.unfocused_dim = d,
                cx,
            );
            cursor.push(
                self.row(
                    cursor.is_empty(),
                    "非当前分屏变暗",
                    "一个标签里有多个分屏时，没有焦点的分屏淡一些",
                    choices,
                )
                .into_any_element(),
            );
        }
        if shows("非当前分屏的光标") {
            let shape = text.unfocused_cursor;
            let choices = self.text_choices(
                "unfocused-cursor",
                &[
                    (UnfocusedCursor::Hidden, "隐藏"),
                    (UnfocusedCursor::Hollow, "空心框"),
                ],
                |s| s == shape,
                |t, s| t.unfocused_cursor = s,
                cx,
            );
            cursor.push(
                self.row(
                    cursor.is_empty(),
                    "非当前分屏的光标",
                    "隐藏时整个窗口只有一个光标；空心框能看出各分屏输入到哪",
                    choices,
                )
                .into_any_element(),
            );
        }
        let typing = !size.is_empty() || !font.is_empty();
        page.child(self.cards(vec![
            ("文字大小", size),
            ("终端字体", font),
            ("光标与分屏", cursor),
        ]))
        .when(typing, |d| {
            d.child(
                div()
                    .pt(px(T::S12))
                    .flex()
                    .items_center()
                    .child(label("只列出本机已安装的等宽字体；中文和符号自动回退").flex_1())
                    .child(secondary("reset-text", "恢复文字默认值").on_click(
                        // Only the type: the cursor and split choices stay as they were.
                        cx.listener(|this, _, _, cx| {
                            let fresh = TerminalText::default();
                            let mut text = terminal_text::current();
                            text.family = fresh.family;
                            text.size = fresh.size;
                            text.line_height = fresh.line_height;
                            this.set_text(text, cx);
                        }),
                    )),
            )
        })
    }
    fn key_row(&self, first: bool, action: Action, map: &Keymap, cx: &Context<Self>) -> gpui::Div {
        let name = action.name();
        let recording = self.recording == Some(action);
        let refused = self
            .refused
            .filter(|(a, _)| *a == action)
            .map(|(_, why)| why);
        let taken = self.taken.as_ref().filter(|(a, ..)| *a == action);
        // The row being recorded, or answering for its chord, stands out with its notes.
        let active = recording || refused.is_some() || taken.is_some();
        let chords = map.chords(action);
        let note = |text: String, color: gpui::Hsla| {
            div()
                .flex_1()
                .text_size(px(T::META))
                .text_color(color)
                .child(text)
        };
        let mut caps = div().flex().gap(px(T::S4)).justify_end();
        if recording {
            caps = caps.child(keycap("按下组合键…", true).border_color(T::color(T::focus())));
        } else if chords.is_empty() {
            caps = caps.child(keycap("未设置", true));
        } else {
            for chord in chords {
                caps = caps.child(keycap(chord.display(), false));
            }
        }
        let mut row = div()
            .flex()
            .flex_col()
            .px(px(T::S16))
            .when(!first, |d| {
                d.border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
            })
            .when(active, |d| d.bg(T::color(T::raised())))
            .child(
                div()
                    .h(px(T::TODO_HEIGHT + T::S4))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .child(action.label()),
                    )
                    // A fixed slot: the chord never moves when 恢复 comes and goes.
                    .child(div().w(px(RESET_WIDTH)).flex().justify_end().when(
                        !map.is_default(action),
                        |d| {
                            d.child(secondary(format!("reset-{name}"), "恢复").on_click(
                                cx.listener(move |this, _, _, cx| this.reset(Some(action), cx)),
                            ))
                        },
                    ))
                    .child(
                        div()
                            .id(format!("key-{name}"))
                            .role(gpui::accesskit::Role::Button)
                            .aria_label(action.label())
                            .min_w(px(KEY_WIDTH))
                            .flex()
                            .justify_end()
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.75))
                            .child(caps)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                window.focus(&this.focus, cx);
                                this.record(action, cx);
                            })),
                    ),
            );
        if recording {
            row = row.child(div().pb(px(T::S10)).flex().child(note(
                "按下新的组合键，需包含 ⌘；Esc 取消".into(),
                T::color(T::secondary()),
            )));
        }
        if let Some(why) = refused {
            row = row.child(
                div()
                    .pb(px(T::S10))
                    .flex()
                    .child(note(why.into(), Group::Error.colors().1)),
            );
        }
        if let Some((_, chord, holder)) = taken {
            row = row.child(
                div()
                    .pb(px(T::S10))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(note(
                        format!("{} 已被「{}」使用", chord.display(), holder.label()),
                        Group::Waiting.colors().1,
                    ))
                    .child(
                        button("take-chord", "仍然使用")
                            .h(px(SEGMENT_HEIGHT))
                            .bg(T::color(T::accent()))
                            .text_color(T::color(T::on_accent()))
                            .on_click(cx.listener(|this, _, _, cx| this.take(cx))),
                    )
                    .child(secondary("keep-chord", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.taken = None;
                            cx.notify();
                        },
                    ))),
            );
        }
        row
    }
    /// ⌘1–⌘9 are fixed: one read-only row instead of nine.
    fn tab_numbers_row(&self, first: bool) -> gpui::Div {
        div()
            .h(px(T::TODO_HEIGHT + T::S4))
            .px(px(T::S16))
            .flex()
            .items_center()
            .gap(px(T::S8))
            .when(!first, |d| {
                d.border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_baseline()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .child("切换到第几个标签"),
                    )
                    .child(label("⌘9 为最后一个；固定")),
            )
            .child(keycap("⌘1 – ⌘9", true))
    }
    fn keys_page(&self, cx: &Context<Self>) -> gpui::Div {
        let map = keymap::current(cx);
        let mut groups = Vec::new();
        for section in Section::ALL {
            let mut rows = Vec::new();
            for action in Keymap::actions().filter(|a| {
                a.section() == section && self.matches(Page::Keys, &a.label(), section.label(), cx)
            }) {
                rows.push(
                    self.key_row(rows.is_empty(), action, &map, cx)
                        .into_any_element(),
                );
            }
            if section == Section::Tabs && self.matches(Page::Keys, "切换到第几个标签", "⌘1 ⌘9", cx)
            {
                rows.push(self.tab_numbers_row(rows.is_empty()).into_any_element());
            }
            groups.push((section.label(), rows));
        }
        self.cards(groups).child(
            div().pt(px(T::S12)).flex().justify_end().child(
                secondary("settings-reset-all", "全部恢复默认")
                    .on_click(cx.listener(|this, _, _, cx| this.reset(None, cx))),
            ),
        )
    }
    fn notifications_page(&self, cx: &Context<Self>) -> gpui::Div {
        let notify = self.shows(Page::Notifications, "桌面通知", cx);
        let sound = self.shows(Page::Notifications, "提示音", cx);
        if !notify && !sound {
            return div();
        }
        let on = self.values.notifications;
        let mut switches = Vec::new();
        if notify {
            let toggle =
                switch("notifications-toggle", on).on_click(cx.listener(|this, _, _, cx| {
                    this.change(cx, |v| v.notifications = !v.notifications)
                }));
            switches.push(
                self.row(
                    true,
                    "桌面通知",
                    "Agent 停下来等你时，发一条系统通知",
                    toggle,
                )
                .into_any_element(),
            );
        }
        // Off until chosen, so an upgrade stays quiet; it sounds without notifications too.
        if sound {
            let toggle = switch("sound-toggle", self.values.sound)
                .on_click(cx.listener(|this, _, _, cx| this.change(cx, |v| v.sound = !v.sound)));
            switches.push(
                self.row(
                    switches.is_empty(),
                    "提示音",
                    "Agent 等你或出错时响一声；不开桌面通知也会响",
                    toggle,
                )
                .into_any_element(),
            );
        }
        let alerting = on || self.values.sound;
        let when = |group: Group, title: &'static str, note: &'static str, first: bool| {
            div()
                .px(px(T::S16))
                .py(px(T::S12))
                .flex()
                .items_center()
                .gap(px(T::S12))
                .when(!first, |d| {
                    d.border_t(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::border()))
                })
                .when(!alerting, |d| d.opacity(0.5))
                .child(dot(group))
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(T::S2))
                        .child(
                            div()
                                .text_size(px(T::BODY_SIZE))
                                .text_color(T::color(T::primary()))
                                .child(title),
                        )
                        .child(label(note)),
                )
                .child(chip(group))
                .into_any_element()
        };
        self.cards(vec![
            ("开关", switches),
            (
                "什么时候提醒",
                vec![
                    when(
                        Group::Waiting,
                        "等你回复",
                        "Agent 提问或请求批准；同一次等待只提醒一次",
                        true,
                    ),
                    when(
                        Group::Error,
                        "出错停下",
                        "Agent 报错中断；同一条错误只提醒一次",
                        false,
                    ),
                ],
            ),
        ])
        .child(
            div()
                .pt(px(T::S12))
                .px(px(T::S4))
                .flex()
                .flex_col()
                .gap(px(T::S4))
                .child(label("只提醒由 Keepline 打开的会话；你正看着的那个终端不会打扰你。"))
                .child(label(
                    "首次开启时 macOS 会询问权限；若已拒绝，到 系统设置 → 通知 → Keepline 重新允许。",
                )),
        )
    }
    /// Every plugin directory and declaration file the app read at start, with its outcome.
    fn plugins_page(&self, cx: &Context<Self>) -> gpui::Div {
        use keepline_core::{agent, plugin};
        if !self.shows(Page::Plugins, "插件", cx) {
            return div();
        }
        let status = |ok: bool, text: &'static str| {
            div()
                .flex_shrink_0()
                .text_size(px(T::SMALL))
                .text_color(if ok {
                    T::color(T::secondary())
                } else {
                    Group::Error.colors().1
                })
                .child(text)
        };
        let where_ = agent::config_dir().map_or_else(
            || "~/.config/keepline".to_owned(),
            |d| d.display().to_string(),
        );
        let mut loaded = Vec::new();
        let found = plugin::report();
        if found.is_empty() {
            loaded.push(
                self.row(
                    true,
                    "还没有插件",
                    "按下面的位置放一个插件目录，重开 Keepline 后会显示在这里。",
                    div(),
                )
                .into_any_element(),
            );
        }
        for f in found {
            let name = f
                .dir
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let first = loaded.is_empty();
            loaded.push(
                match &f.result {
                    Ok(p) => self.row(
                        first,
                        &format!("{} · {}", p.name, p.id),
                        &format!(
                            "提供 {}{}{}",
                            p.provides
                                .iter()
                                .map(|k| k.label())
                                .collect::<Vec<_>>()
                                .join("、"),
                            p.version
                                .as_deref()
                                .map_or_else(String::new, |v| format!(" · v{v}")),
                            p.description
                                .as_deref()
                                .map_or_else(String::new, |d| format!(" · {d}")),
                        ),
                        {
                            // The switch takes effect at the next start; until then the row
                            // says so rather than pretending it already changed.
                            let next = plugin::enabled_next_start(&p.id);
                            let id = p.id.clone();
                            div()
                                .flex()
                                .items_center()
                                .gap(px(T::S8))
                                .child(match (p.enabled, next) {
                                    (true, true) => status(true, "已加载"),
                                    (false, false) => status(true, "已停用"),
                                    _ => status(true, "重开后生效"),
                                })
                                .child(
                                    secondary(
                                        gpui::SharedString::from(format!("plugin-switch-{}", p.id)),
                                        if next { "停用" } else { "启用" },
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.text_error = plugin::set_enabled(&id, !next)
                                                .err()
                                                .map(|e| format!("没能保存插件开关：{e}"));
                                            cx.notify();
                                        },
                                    )),
                                )
                        },
                    ),
                    Err(e) => self.row(first, &name, e, status(false, "没有加载")),
                }
                .into_any_element(),
            );
        }
        // Each declaration file the plugins and the config folder brought: what it did.
        let files = |rows: Vec<(String, Result<String, String>)>| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (file, result))| {
                    match result {
                        Ok(did) => self.row(i == 0, &file, &did, status(true, "生效")),
                        Err(e) => self.row(i == 0, &file, &e, status(false, "没有生效")),
                    }
                    .into_any_element()
                })
                .collect::<Vec<_>>()
        };
        let actions = files(
            keepline_core::action::report()
                .iter()
                .map(|a| {
                    let did = a
                        .result
                        .clone()
                        .map(|titles| format!("加进右键菜单和 ⌘P：{titles}"));
                    (a.file.display().to_string(), did)
                })
                .collect(),
        );
        let rules = files(
            keepline_core::rule::report()
                .iter()
                .map(|r| {
                    let did = r
                        .result
                        .clone()
                        .map(|list| format!("这些命令算跑过验证：{list}"));
                    (r.file.display().to_string(), did)
                })
                .collect(),
        );
        let hooks = files(
            keepline_core::hook::report()
                .iter()
                .map(|r| {
                    let did = r.result.clone().map(|list| {
                        format!("会话状态变化时由后台调用：{list}；结果写在后台日志里")
                    });
                    (r.file.display().to_string(), did)
                })
                .collect(),
        );
        let sidebars = files(
            keepline_core::sidebar::report()
                .iter()
                .map(|r| {
                    let did = r.result.clone().map(|list| format!("左栏顶部多了：{list}"));
                    (r.file.display().to_string(), did)
                })
                .collect(),
        );
        let decorations = files(
            keepline_core::decoration::report()
                .iter()
                .map(|r| {
                    let did = r
                        .result
                        .clone()
                        .map(|list| format!("窗格标题栏或悬停卡片里显示：{list}"));
                    (r.file.display().to_string(), did)
                })
                .collect(),
        );
        let declared = files(
            agent::report()
                .iter()
                .map(|d| {
                    let did = d.result.clone().map(|key| format!("注册为 {key}"));
                    (d.file.display().to_string(), did)
                })
                .collect(),
        );
        self.cards(vec![
            ("插件", loaded),
            ("动作文件", actions),
            ("验证规则文件", rules),
            ("Hooks 文件", hooks),
            ("左栏文件", sidebars),
            ("窗格装饰文件", decorations),
            ("Agent 声明文件", declared),
        ])
        .child(div().pt(px(T::S12)).px(px(T::S4)).child(label(format!(
            "放在 {where_}/plugins/<id>/plugin.json；Agent 声明也可以直接放在 {where_}/agents/*.json。"
        ))))
    }
    fn path_row(
        &self,
        first: bool,
        title: &'static str,
        path: Option<PathBuf>,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let shown = path
            .as_ref()
            .map_or_else(|| "未知".into(), |p| tilde(&p.display().to_string()));
        let control = secondary(format!("reveal-{title}"), "在访达中显示")
            .when(path.is_none(), |b| b.opacity(0.5))
            .on_click(cx.listener(move |this, _, _, cx| {
                let Some(path) = &path else {
                    return;
                };
                // A file is selected in its folder; a folder opens itself.
                let mut open = std::process::Command::new("open");
                if path.is_file() {
                    open.arg("-R");
                }
                if let Err(e) = open.arg(path).spawn() {
                    this.text_error = Some(format!("无法打开访达：{e}"));
                    cx.notify();
                }
            }));
        self.file_row(first, title, shown, None, control, cx)
    }
    /// A file Keepline uses: its title, the path in mono below it, then an optional note,
    /// in the error colour when `alarm`.
    fn file_row(
        &self,
        first: bool,
        title: impl Into<gpui::SharedString>,
        shown: String,
        note: Option<(String, bool)>,
        control: impl IntoElement,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .px(px(T::S16))
            .py(px(T::S12))
            .flex()
            .items_center()
            .gap(px(T::S16))
            .when(!first, |d| {
                d.border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(T::S2))
                    .child(
                        div()
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .child(title.into()),
                    )
                    .child(
                        div()
                            .truncate()
                            .font(T::mono(cx))
                            .text_size(px(T::META))
                            .text_color(T::color(T::secondary()))
                            .child(shown),
                    )
                    .when_some(note, |d, (note, alarm)| {
                        d.child(label(note).when(alarm, |l| l.text_color(Group::Error.colors().1)))
                    }),
            )
            .child(control)
            .into_any_element()
    }
    /// One row per CLI whose hooks Keepline installs: their state as `keeplined hooks
    /// status` read it when the panel opened, and a button that installs them anew.
    fn hook_rows(&self, first: bool, cx: &Context<Self>) -> Vec<gpui::AnyElement> {
        use crate::agent_hooks::State;
        let hooks = match &self.hooks {
            None => {
                return vec![
                    self.row(first, "Agent Hook", "正在检查…", div())
                        .into_any_element(),
                ];
            }
            Some(Err(e)) => {
                let note = format!("没能检查：{e}");
                return vec![
                    self.row(first, "Agent Hook", &note, div())
                        .into_any_element(),
                ];
            }
            Some(Ok(hooks)) => hooks,
        };
        hooks
            .iter()
            .enumerate()
            .map(|(i, hook)| {
                let (state, action) = match hook.state {
                    State::Current => ("已安装且是最新", None),
                    State::Outdated => ("已安装但过期", Some("重新安装")),
                    State::Missing => ("未安装", Some("安装")),
                    State::Unreadable => ("读不了这个文件", None),
                };
                let mut note = state.to_owned();
                if !hook.differences.is_empty() {
                    note = format!("{note}：{}", hook.differences.join("；"));
                }
                let busy = self.installing.is_some();
                let control = action.map(|text| {
                    let agent = hook.agent.clone();
                    let text = if self.installing.as_ref() == Some(&agent) {
                        "正在安装…"
                    } else {
                        text
                    };
                    secondary(format!("hooks-{agent}"), text)
                        .when(busy, |b| b.opacity(0.5))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.installing.is_some() {
                                return;
                            }
                            // Writes the CLI's settings: only ever from this click.
                            this.installing = Some(agent.clone());
                            cx.notify();
                            Self::check_hooks(Some(agent.clone()), cx);
                        }))
                });
                self.file_row(
                    first && i == 0,
                    format!("{} Hook", hook.name),
                    tilde(&hook.settings),
                    Some((
                        note,
                        matches!(hook.state, State::Outdated | State::Unreadable),
                    )),
                    div().children(control),
                    cx,
                )
            })
            .collect()
    }
    fn about_page(&self, cx: &Context<Self>) -> gpui::Div {
        let mut page = div().flex().flex_col();
        if self.shows(Page::About, "版本", cx) {
            page = page.child(
                card().mt(px(T::S20)).child(
                    div()
                        .p(px(T::S20))
                        .flex()
                        .items_center()
                        .gap(px(T::S16))
                        .child(
                            gpui::img(cx.global::<Chosen>().icon.title_image())
                                .size(px(ICON_PREVIEW + T::S12)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(T::S4))
                                .child(
                                    div()
                                        .text_size(px(T::TITLE))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(T::color(T::primary()))
                                        .child("Keepline"),
                                )
                                .child(label(format!(
                                    "版本 {} · Agent 工作台",
                                    env!("CARGO_PKG_VERSION")
                                )))
                                .child(label("别再丢掉 Codex 与 Claude Code 里的工作。")),
                        ),
                ),
            );
        }
        let config = crate::ui::status_colors::config_dir();
        let mut data = Vec::new();
        if self.shows(Page::About, "运行数据", cx) {
            data.push(self.path_row(data.is_empty(), "运行数据", self.runtime.clone(), cx));
        }
        if self.shows(Page::About, "偏好文件", cx) {
            data.push(self.path_row(data.is_empty(), "偏好文件", config.clone(), cx));
            let colors = config.map(|dir| dir.join("colors.json"));
            if colors.as_ref().is_some_and(|p| p.is_file()) {
                data.push(self.path_row(false, "状态颜色 colors.json", colors, cx));
            }
        }
        if self.shows(Page::About, "Agent Hook", cx) {
            data.extend(self.hook_rows(data.is_empty(), cx));
        }
        page.child(self.cards(vec![("数据与偏好", data)])).child(
            div().pt(px(T::S12)).px(px(T::S4)).child(label(
                "工作区设置随数据库保存；外观、终端文字与快捷键存在偏好文件夹，只属于这台电脑。",
            )),
        )
    }
    fn content(&self, page: Page, cx: &Context<Self>) -> gpui::Div {
        match page {
            Page::General => self.general_page(cx),
            Page::Look => self.look_page(cx),
            Page::Terminal => self.terminal_page(cx),
            Page::Keys => self.keys_page(cx),
            Page::Notifications => self.notifications_page(cx),
            Page::Plugins => self.plugins_page(cx),
            Page::About => self.about_page(cx),
        }
    }
    fn body(&self, cx: &Context<Self>) -> gpui::Div {
        if self.query(cx).is_empty() {
            return self.content(self.page, cx);
        }
        let mut body = div().flex().flex_col().gap(px(T::S12));
        let mut found = false;
        for page in Page::ALL {
            if !self.page_matches(page, cx) {
                continue;
            }
            found = true;
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .pt(px(T::S8))
                            .text_size(px(T::FORM_TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child(page.label()),
                    )
                    .child(self.content(page, cx)),
            );
        }
        if !found {
            body = body.child(
                div()
                    .pt(px(T::S20 * 4.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .text_color(T::color(T::primary()))
                            .child("没有找到相关设置"),
                    )
                    .child(label("试试「字体」「光标」「图标」或「快捷键」")),
            );
        }
        body
    }
    /// Saving, saved, or why it failed: one line under the pages.
    fn status(&self) -> gpui::Div {
        let error = self.save_error.as_ref().or(self.text_error.as_ref());
        let (mark, text, color) = match error {
            Some(error) => ("!", error.clone(), Group::Error.colors().1),
            None if self.busy => ("…", "正在保存".into(), T::color(T::secondary())),
            None => ("✓", "更改自动保存".into(), T::color(T::secondary())),
        };
        div()
            .flex()
            .items_start()
            .gap(px(T::S6))
            .text_size(px(T::META))
            .text_color(color)
            .child(div().flex_shrink_0().child(mark))
            // A long path in the error is cut; the first lines say what failed.
            .child(div().min_w_0().line_clamp(3).child(text))
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let searching = !self.query(cx).is_empty();
        let body = self.body(cx);
        let query_focused = self.query.clone();
        let (title, description) = if searching {
            ("搜索结果", "在所有分类中查找。")
        } else {
            (self.page.label(), self.page.description())
        };
        div()
            .id("settings-panel")
            .w(px(T::SETTINGS_WIDTH))
            .max_w_full()
            .h(px(PANEL_HEIGHT))
            .max_h_full()
            .flex()
            .overflow_hidden()
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::panel()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .text_color(T::color(T::body()))
            .shadow_lg()
            .track_focus(&self.focus)
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if this.recording.is_some() {
                        this.key(&event.keystroke, cx);
                        cx.stop_propagation();
                        window.prevent_default();
                    } else if event.keystroke.key == "escape"
                        || matches!(
                            keymap::action(&event.keystroke, cx),
                            Some(Action::Close | Action::Settings)
                        )
                    {
                        cx.emit(SettingsAction::Close);
                        cx.stop_propagation();
                        window.prevent_default();
                    } else if !query_focused.focus_handle(cx).is_focused(window) {
                        // Settings never forward a shortcut to a terminal behind the panel.
                        cx.stop_propagation();
                    }
                }),
            )
            .on_key_down(|event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.platform {
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .w(px(NAV_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .bg(T::color(T::app()))
                    .border_r(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(
                        div()
                            .px(px(T::S16))
                            .pt(px(T::S20))
                            .pb(px(T::S12))
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child("设置"),
                    )
                    .child(
                        div()
                            .px(px(T::S12))
                            .pb(px(T::S12))
                            .child(self.query.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px(px(T::S8))
                            .flex()
                            .flex_col()
                            .gap(px(T::S2))
                            .children(
                                Page::ALL
                                    .into_iter()
                                    .map(|page| self.nav_item(page, searching, cx)),
                            ),
                    )
                    .child(
                        div()
                            .px(px(T::S16))
                            .py(px(T::S14))
                            .border_t(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                            .child(self.status()),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px(px(T::S20 + T::S8))
                            .pt(px(T::S20))
                            .pb(px(T::S4))
                            .flex()
                            .items_start()
                            .gap(px(T::S12))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(T::S4))
                                    .child(
                                        div()
                                            .text_size(px(T::TITLE))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(T::color(T::primary()))
                                            .child(title),
                                    )
                                    .child(label(description)),
                            )
                            .child(
                                button("settings-close", "✕")
                                    .aria_label("关闭设置")
                                    .w(px(T::ROW))
                                    .text_color(T::color(T::secondary()))
                                    .on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(SettingsAction::Close)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px(px(T::S20 + T::S8))
                            .pb(px(T::S20 + T::S8))
                            .child(body),
                    ),
            )
    }
}

impl Workbench {
    pub(crate) fn show_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = &self.settings {
            window.focus(&view.read(cx).focus.clone(), cx);
            return;
        }
        self.search.hide();
        let values = self.workspace.settings.clone();
        let busy = self.store.busy();
        let runtime = self.store.socket().parent().map(Path::to_path_buf);
        let view = cx.new(|cx| SettingsView::new(values, busy, runtime, cx));
        cx.subscribe(&view, |this, _, action, cx| match action {
            SettingsAction::Close => {
                this.settings = None;
                this.nav.pending_focus = true;
                cx.notify();
            }
            SettingsAction::Change(values) => {
                if !this.store.busy() {
                    this.edit(Edit::Settings(values.clone()), cx);
                }
            }
        })
        .detach();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        self.settings = Some(view);
        cx.notify();
    }
    /// Opens settings on one page, as the sidebar's 「提醒」 does for notifications.
    pub(super) fn show_settings_page(
        &mut self,
        page: Page,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_settings(window, cx);
        if let Some(view) = &self.settings {
            view.update(cx, |v, cx| {
                v.page = page;
                cx.notify();
            });
        }
    }
    pub(super) fn settings_overlay(
        &self,
        view: Entity<SettingsView>,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        gpui::deferred(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .flex()
                .justify_center()
                .items_center()
                .p(px(T::S20))
                .bg(gpui::rgba(T::scrim()))
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.settings = None;
                        this.nav.pending_focus = true;
                        cx.notify();
                    }),
                )
                .child(view),
        )
        .with_priority(2)
        .into_any_element()
    }
}

#[cfg(feature = "visual-test")]
impl Workbench {
    pub(crate) fn visual_settings_page_name(&self, cx: &gpui::App) -> &'static str {
        self.settings.as_ref().unwrap().read(cx).page.label()
    }
    pub(crate) fn visual_settings_toggle_sound(&mut self, cx: &mut Context<Self>) {
        let view = self.settings.clone().expect("settings open");
        view.update(cx, |v, cx| {
            v.change(cx, |values| values.sound = !values.sound)
        });
    }
    pub(crate) fn visual_settings_values(&self) -> Settings {
        self.workspace.settings.clone()
    }
    pub(crate) fn visual_settings_query(&self, cx: &gpui::App) -> Entity<Field> {
        self.settings.as_ref().unwrap().read(cx).query.clone()
    }
    /// The page labels in nav order, for a tour that shows each in turn.
    pub(crate) fn visual_settings_pages() -> Vec<&'static str> {
        Page::ALL.into_iter().map(Page::label).collect()
    }
    pub(crate) fn visual_settings_show(&mut self, label: &str, cx: &mut Context<Self>) {
        let view = self.settings.clone().expect("settings open");
        view.update(cx, |v, cx| {
            v.page = Page::ALL.into_iter().find(|p| p.label() == label).unwrap();
            cx.notify();
        });
    }
}
