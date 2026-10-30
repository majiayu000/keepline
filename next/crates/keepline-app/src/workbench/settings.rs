//! 设置: a floating panel over the workbench, opened from the sidebar, the Keepline menu
//! or ⌘,. The shortcuts page records a new chord in place; the appearance page is the
//! same choice as Keepline → 外观.
use super::*;
use crate::keymap::{self, Action, Chord, Keymap, Section};
use crate::ui::{
    appearance::Appearance,
    terminal_text::{self, TerminalText},
};

/// The chord button keeps one width, recording or not.
const KEY_WIDTH: f32 = T::HERE_LABEL + T::S20;
/// Room kept for 恢复 on every row.
const RESET_WIDTH: f32 = T::S20 * 2. + T::S12;
/// An outlined button: plain enough for a row, still plainly a button.
fn secondary(
    id: impl Into<gpui::ElementId>,
    text: impl Into<gpui::SharedString>,
) -> gpui::Stateful<gpui::Div> {
    button(id, text)
        .flex_shrink_0()
        .h(px(T::ROW - T::S6))
        .px(px(T::S8))
        .border(px(T::BORDER_WIDTH))
        .border_color(T::color(T::control()))
        .bg(T::color(T::button()))
        .text_color(T::color(T::primary()))
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Page {
    General,
    Look,
    Keys,
    Notifications,
    Plugins,
}
impl Page {
    const ALL: [Self; 5] = [
        Self::General,
        Self::Look,
        Self::Keys,
        Self::Notifications,
        Self::Plugins,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::General => "通用",
            Self::Look => "外观与终端",
            Self::Keys => "快捷键",
            Self::Notifications => "通知",
            Self::Plugins => "插件",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::General => "按你的习惯启动工作，整理会话。",
            Self::Look => "调整明暗和终端文字，修改后立即预览。",
            Self::Keys => "点击组合键重新录制，按 Esc 取消。",
            Self::Notifications => "在 Agent 需要你时接收提醒。",
            Self::Plugins => {
                "哪些插件和声明文件生效了，哪些没有、为什么。改动后重开 Keepline 生效。"
            }
        }
    }
}
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
    /// The action waiting for its new chord.
    pub(super) recording: Option<Action>,
    /// A recorded chord another action already has: it moves only when confirmed.
    pub(super) taken: Option<(Action, Chord, Action)>,
    /// Why the last recorded chord was refused, shown under its row.
    pub(super) refused: Option<(Action, &'static str)>,
}
impl gpui::EventEmitter<SettingsAction> for SettingsView {}
impl SettingsView {
    pub(super) fn new(values: Settings, busy: bool, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| {
            Field::new("搜索设置", "", cx)
                .compact()
                .placeholder("搜索设置…")
        });
        cx.observe(&query, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&query, |_, _, event, cx| {
            if matches!(event, crate::ui::field::FieldEvent::Cancel) {
                cx.emit(SettingsAction::Close);
            }
        })
        .detach();
        Self {
            focus: cx.focus_handle(),
            page: Page::General,
            values,
            busy,
            save_error: None,
            text_error: None,
            query,
            recording: None,
            taken: None,
            refused: None,
        }
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
    fn page_button(&self, page: Page, text: &'static str, cx: &Context<Self>) -> impl IntoElement {
        let on = self.page == page;
        button(text, text)
            .when(on, |b| {
                b.bg(T::color(T::raised()))
                    .text_color(T::color(T::primary()))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
            })
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
    fn key_row(&self, action: Action, map: &Keymap, cx: &Context<Self>) -> gpui::Div {
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
        let text = if recording {
            "按下组合键…".to_owned()
        } else if chords.is_empty() {
            "未设置".to_owned()
        } else {
            chords
                .iter()
                .map(Chord::display)
                .collect::<Vec<_>>()
                .join(" / ")
        };
        let note = |text: String, color: gpui::Hsla| {
            div()
                .flex_1()
                .text_size(px(T::META))
                .text_color(color)
                .child(text)
        };
        let mut row = div()
            .flex()
            .flex_col()
            .mx(px(-T::S8))
            .px(px(T::S8 - T::BORDER_WIDTH))
            .rounded(px(T::CONTROL_RADIUS))
            .border(px(T::BORDER_WIDTH))
            .border_color(gpui::transparent_black())
            .when(active, |d| {
                d.bg(T::color(T::raised()))
                    .border_color(T::color(T::focus()))
            })
            .child(
                div()
                    .h(px(T::ROW + T::S4))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(T::BODY_SIZE))
                            .child(action.label()),
                    )
                    // A fixed slot: the chord button never moves when 恢复 comes and goes.
                    .child(div().w(px(RESET_WIDTH)).flex().justify_end().when(
                        !map.is_default(action),
                        |d| {
                            d.child(secondary(format!("reset-{name}"), "恢复").on_click(
                                cx.listener(move |this, _, _, cx| this.reset(Some(action), cx)),
                            ))
                        },
                    ))
                    .child(
                        button(format!("key-{name}"), text)
                            .w(px(KEY_WIDTH))
                            .flex_shrink_0()
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(if recording {
                                T::focus()
                            } else {
                                T::control()
                            }))
                            .bg(T::color(T::button()))
                            .text_color(T::color(if chords.is_empty() || recording {
                                T::muted()
                            } else {
                                T::primary()
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                window.focus(&this.focus, cx);
                                this.record(action, cx);
                            })),
                    ),
            );
        if recording {
            row = row.child(div().pb(px(T::S8)).flex().child(note(
                "按下新的组合键，需包含 ⌘；Esc 取消".into(),
                T::color(T::secondary()),
            )));
        }
        if let Some(why) = refused {
            row = row.child(
                div()
                    .pb(px(T::S8))
                    .flex()
                    .child(note(why.into(), Group::Error.colors().1)),
            );
        }
        if let Some((_, chord, holder)) = taken {
            row = row.child(
                div()
                    .pb(px(T::S8))
                    .flex()
                    .items_center()
                    .gap(px(T::S8))
                    .child(note(
                        format!("{} 已被「{}」使用", chord.display(), holder.label()),
                        Group::Waiting.colors().1,
                    ))
                    .child(
                        button("take-chord", "仍然使用")
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
    fn tab_numbers_row(&self) -> gpui::Div {
        div()
            .h(px(T::ROW + T::S4))
            .flex()
            .items_center()
            .gap(px(T::S8))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_baseline()
                    .gap(px(T::S8))
                    .child(div().text_size(px(T::BODY_SIZE)).child("切换到第几个标签"))
                    .child(label("⌘9 为最后一个；固定，不可修改")),
            )
            .child(
                div()
                    .w(px(KEY_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()))
                    .child("⌘1 – ⌘9"),
            )
    }
    fn keys_page(&self, cx: &Context<Self>) -> gpui::Div {
        let map = keymap::current(cx);
        let mut page = div().flex().flex_col().gap(px(T::S14));
        for section in Section::ALL {
            let mut group = div()
                .flex()
                .flex_col()
                .child(label(section.label()).pb(px(T::S4)));
            for action in Keymap::actions().filter(|a| {
                a.section() == section && self.matches(Page::Keys, &a.label(), section.label(), cx)
            }) {
                group = group.child(self.key_row(action, &map, cx));
            }
            if section == Section::Tabs && self.matches(Page::Keys, "切换到第几个标签", "⌘1 ⌘9", cx)
            {
                group = group.child(self.tab_numbers_row());
            }
            let has_actions = Keymap::actions().any(|a| {
                a.section() == section && self.matches(Page::Keys, &a.label(), section.label(), cx)
            });
            if has_actions
                || (section == Section::Tabs
                    && self.matches(Page::Keys, "切换到第几个标签", "⌘1 ⌘9", cx))
            {
                page = page.child(group);
            }
        }
        page
    }
    fn matches(&self, page: Page, title: &str, note: &str, cx: &gpui::App) -> bool {
        let query = self.query.read(cx).text.trim().to_lowercase();
        query.is_empty()
            || format!("{} {title} {note}", page.label())
                .to_lowercase()
                .contains(&query)
    }
    fn row(&self, title: &str, note: &str, control: impl IntoElement) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(T::S16))
            .py(px(T::S14))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .child(
                        div()
                            .text_size(px(T::BODY_SIZE))
                            .text_color(T::color(T::primary()))
                            .child(title.to_owned()),
                    )
                    .child(label(note.to_owned())),
            )
            .child(div().flex_shrink_0().child(control))
    }
    fn choice(&self, id: String, text: String, selected: bool) -> gpui::Stateful<gpui::Div> {
        secondary(id, text).when(selected, |b| {
            b.bg(T::color(T::raised()))
                .border_color(T::color(T::accent()))
                .text_color(T::color(T::primary()))
        })
    }
    fn general_page(&self, cx: &Context<Self>) -> gpui::Div {
        let mut page = div().flex().flex_col();
        if self.matches(
            Page::General,
            "默认 Agent",
            "新建待办和未指定 Agent 的会话",
            cx,
        ) {
            let mut choices = div().flex().gap(px(T::S6));
            for agent in Agent::all() {
                choices = choices.child(
                    self.choice(
                        format!("agent-{}", agent.name()),
                        agent.name().into(),
                        self.values.default_agent == agent,
                    )
                    .when(!self.busy, |b| {
                        b.on_click(cx.listener(move |this, _, _, cx| {
                            let mut values = this.values.clone();
                            values.default_agent = agent;
                            cx.emit(SettingsAction::Change(values));
                        }))
                    }),
                );
            }
            page = page.child(self.row(
                "默认 Agent",
                "新建待办和未指定 Agent 的会话使用此选项",
                choices,
            ));
        }
        for (away, title, note, options, selected) in [
            (
                true,
                "回来时显示总览",
                "离开窗口达到这段时间后，返回时先看工作进展",
                &[5, 15, 30, 60][..],
                self.values.away_minutes,
            ),
            (
                false,
                "旧会话收起时间",
                "已关闭的会话超过这段时间后从侧栏收起，历史仍保留",
                &[1, 3, 7, 14][..],
                self.values.stale_days,
            ),
        ] {
            if !self.matches(Page::General, title, note, cx) {
                continue;
            }
            let mut choices = div().flex().gap(px(T::S4));
            // Show an existing custom value even when it is not one of the presets.
            let mut numbers = options.to_vec();
            if !numbers.contains(&selected) {
                numbers.push(selected);
            }
            for value in numbers {
                let text = format!("{value}{}", if away { " 分钟" } else { " 天" });
                choices = choices.child(
                    self.choice(format!("{away}-{value}"), text, value == selected)
                        .when(!self.busy, |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                let mut values = this.values.clone();
                                if away {
                                    values.away_minutes = value;
                                } else {
                                    values.stale_days = value;
                                }
                                cx.emit(SettingsAction::Change(values));
                            }))
                        }),
                );
            }
            page = page.child(self.row(title, note, choices));
        }
        page
    }
    fn set_text(&mut self, text: TerminalText, cx: &mut Context<Self>) {
        self.text_error = terminal_text::update(text).err();
        cx.refresh_windows();
        cx.notify();
    }
    fn look_page(&self, cx: &Context<Self>) -> gpui::Div {
        let chosen = cx.try_global::<Appearance>().copied().unwrap_or_default();
        let text = terminal_text::current();
        let mut page = div().flex().flex_col();
        if self.matches(Page::Look, "主题", "跟随系统 亮色 暗色 明暗", cx) {
            let mut choices = div().flex().gap(px(T::S6));
            for (i, look) in Appearance::ALL.into_iter().enumerate() {
                choices = choices.child(
                    self.choice(format!("look-{i}"), look.label().into(), look == chosen)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.text_error = crate::ui::appearance::choose(look, cx).err();
                            crate::set_menus(cx);
                            cx.notify();
                        })),
                );
            }
            page = page.child(self.row("主题", "窗口与终端一起切换；可跟随 macOS", choices));
        }
        if self.matches(Page::Look, "界面样式", "样式 标准 终端原生 tmux 等宽 style", cx) {
            use crate::ui::style::{self, Style};
            let chosen = style::current(cx);
            let mut choices = div().flex().gap(px(T::S6));
            for (i, look) in Style::ALL.into_iter().enumerate() {
                choices = choices.child(
                    self.choice(format!("style-{i}"), look.label().into(), look == chosen)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.text_error = style::choose(look, cx).err();
                            cx.notify();
                        })),
                );
            }
            page = page.child(self.row(
                "界面样式",
                "标准：现在的布局去掉重复；终端原生：外壳退成细线，等宽字，标签在底部",
                choices,
            ));
        }
        if self.matches(
            Page::Look,
            "终端字体",
            "字体 family font JetBrains Mono Menlo Monaco",
            cx,
        ) {
            let installed = cx.text_system().all_font_names();
            let mut choices = div()
                .flex()
                .flex_wrap()
                .gap(px(T::S6))
                .max_w(px(T::CARD_MIN));
            for family in ["", "Menlo", "Monaco", "SF Mono"]
                .into_iter()
                .filter(|f| f.is_empty() || installed.iter().any(|n| n == f))
            {
                choices = choices.child(
                    self.choice(
                        format!("font-{family}"),
                        if family.is_empty() {
                            "JetBrains Mono".into()
                        } else {
                            family.into()
                        },
                        text.family == family,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.family = family.into();
                        this.set_text(text, cx);
                    })),
                );
            }
            page = page.child(self.row(
                "终端字体",
                "选择等宽字体，中文和符号继续使用字形回退",
                choices,
            ));
        }
        if self.matches(Page::Look, "字号", "文字大小 font size 缩放", cx) {
            let controls = div()
                .flex()
                .items_center()
                .gap(px(T::S8))
                .child(
                    secondary("font-smaller", "−").on_click(cx.listener(|this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.size = (text.size - 1.).max(8.);
                        this.set_text(text, cx);
                    })),
                )
                .child(
                    div()
                        .w(px(RESET_WIDTH))
                        .text_center()
                        .child(format!("{} pt", text.size)),
                )
                .child(
                    secondary("font-larger", "+").on_click(cx.listener(|this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.size = (text.size + 1.).min(36.);
                        this.set_text(text, cx);
                    })),
                );
            page = page.child(self.row("字号", "与终端缩放快捷键共用，重开应用后仍保留", controls));
        }
        if self.matches(Page::Look, "行距", "行高 line height 紧凑 舒适 宽松", cx) {
            let mut choices = div().flex().gap(px(T::S6));
            for height in [1., 1.2, 1.4, 1.6] {
                choices = choices.child(
                    self.choice(
                        format!("line-{height}"),
                        format!("{height}×"),
                        (text.line_height - height).abs() < 0.01,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.line_height = height;
                        this.set_text(text, cx);
                    })),
                );
            }
            page = page.child(self.row(
                "行距",
                "相对于字体自然行高，文字始终在单元格内居中",
                choices,
            ));
        }
        if self.matches(Page::Look, "光标闪烁", "光标 闪烁 blink cursor", cx) {
            let mut choices = div().flex().gap(px(T::S6));
            for (blink, name) in [(true, "闪烁"), (false, "常亮")] {
                choices = choices.child(
                    self.choice(format!("blink-{blink}"), name.into(), text.cursor_blink == blink)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let mut text = terminal_text::current();
                            text.cursor_blink = blink;
                            this.set_text(text, cx);
                        })),
                );
            }
            page = page.child(self.row(
                "光标闪烁",
                "当前分屏的光标；程序要求常亮时照办，打字时保持亮着",
                choices,
            ));
        }
        if self.matches(Page::Look, "非当前分屏变暗", "分屏 变暗 焦点 dim split", cx) {
            let mut choices = div().flex().gap(px(T::S6));
            for (dim, name) in [(0., "不变暗"), (0.15, "轻"), (0.3, "明显")] {
                choices = choices.child(
                    self.choice(
                        format!("dim-{dim}"),
                        name.into(),
                        (text.unfocused_dim - dim).abs() < 0.01,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.unfocused_dim = dim;
                        this.set_text(text, cx);
                    })),
                );
            }
            page = page.child(self.row(
                "非当前分屏变暗",
                "一个标签里有多个分屏时，没有焦点的分屏淡一些",
                choices,
            ));
        }
        if self.matches(Page::Look, "非当前分屏的光标", "分屏 光标 空心 焦点 cursor split", cx) {
            use terminal_text::UnfocusedCursor;
            let mut choices = div().flex().gap(px(T::S6));
            for (shape, id, name) in [
                (UnfocusedCursor::Hidden, "hidden", "隐藏"),
                (UnfocusedCursor::Hollow, "hollow", "空心框"),
            ] {
                choices = choices.child(
                    self.choice(
                        format!("unfocused-cursor-{id}"),
                        name.into(),
                        text.unfocused_cursor == shape,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let mut text = terminal_text::current();
                        text.unfocused_cursor = shape;
                        this.set_text(text, cx);
                    })),
                );
            }
            page = page.child(self.row(
                "非当前分屏的光标",
                "隐藏时整个窗口只有一个光标；空心框能看出各分屏输入到哪",
                choices,
            ));
        }
        page.child(
            div()
                .mt(px(T::S20))
                .p(px(T::S16))
                .rounded(px(T::CONTROL_RADIUS))
                .bg(T::color(T::terminal_bg()))
                .border(px(T::BORDER_WIDTH))
                .border_color(T::color(T::control()))
                .child(label("终端预览 · Aa / 中文 / 0123").pb(px(T::S8)))
                .child(
                    gpui::canvas(
                        |_, _, _| (),
                        |bounds, _, window, cx| {
                            use crate::terminal::screen::{self, Run};
                            let font = T::terminal_mono(cx);
                            let Some(metrics) = screen::CellMetrics::measure(&font, window) else {
                                return;
                            };
                            let preview = screen::Screen {
                                background: T::terminal_bg(),
                                rows: [
                                    "~/workspace  main",
                                    "$ keepline",
                                    "准备继续你的工作  0123456789",
                                ]
                                .into_iter()
                                .map(|text| {
                                    vec![Run {
                                        col: 0,
                                        cols: text
                                            .chars()
                                            .map(|c| if c.is_ascii() { 1 } else { 2 })
                                            .sum(),
                                        text: text.into(),
                                        fg: T::terminal_fg(),
                                        bg: None,
                                        attrs: screen::Attrs::default(),
                                    }]
                                })
                                .collect(),
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
                    .h(px(text.size * text.line_height * 4.5)),
                ),
        )
        .child(
            div().mt(px(T::S12)).flex().justify_end().child(
                secondary("reset-text", "恢复终端文字默认值").on_click(
                    // Only the type: the cursor and split choices stay as they were.
                    cx.listener(|this, _, _, cx| {
                        let fresh = TerminalText::default();
                        let mut text = terminal_text::current();
                        text.family = fresh.family;
                        text.size = fresh.size;
                        text.line_height = fresh.line_height;
                        this.set_text(text, cx);
                    }),
                ),
            ),
        )
    }
    fn notifications_page(&self, cx: &Context<Self>) -> gpui::Div {
        let mut page = div().flex().flex_col();
        if self.matches(
            Page::Notifications,
            "桌面通知",
            "Agent 等待输入 错误 完成 macOS 提醒",
            cx,
        ) {
            page = page.child(self.row("桌面通知", "Agent 等待输入或出现错误时提醒你", self.choice("notifications-toggle".into(), if self.values.notifications { "已开启".into() } else { "已关闭".into() }, self.values.notifications)
                .when(!self.busy, |b| b.on_click(cx.listener(|this, _, _, cx| {
                    let mut values = this.values.clone(); values.notifications = !values.notifications; cx.emit(SettingsAction::Change(values));
                })))))
                .child(div().mt(px(T::S16)).child(label("首次开启时，macOS 会询问通知权限。若已拒绝，可到系统设置 → 通知 → Keepline 重新允许。")));
        }
        // Off until chosen, so an upgrade stays quiet; it sounds without notifications too.
        if self.matches(
            Page::Notifications,
            "提示音",
            "Agent 等待输入 错误 声音 不开通知也会响",
            cx,
        ) {
            page = page.child(
                self.row(
                    "提示音",
                    "Agent 等你或出错时响一声；不开桌面通知也会响",
                    self.choice(
                        "sound-toggle".into(),
                        if self.values.sound {
                            "已开启".into()
                        } else {
                            "已关闭".into()
                        },
                        self.values.sound,
                    )
                    .when(!self.busy, |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            let mut values = this.values.clone();
                            values.sound = !values.sound;
                            cx.emit(SettingsAction::Change(values));
                        }))
                    }),
                ),
            );
        }
        page
    }
    /// Every plugin directory and declaration file the app read at start, with its outcome.
    fn plugins_page(&self) -> gpui::Div {
        use keepline_core::{agent, plugin};
        let status = |ok: bool, text: String| {
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
        let mut page = div().flex().flex_col().child(label(format!(
            "放在 {where_}/plugins/<id>/plugin.json；Agent 声明也可以直接放在 {where_}/agents/*.json。"
        )));
        let found = plugin::report();
        if found.is_empty() {
            page = page.child(self.row(
                "还没有插件",
                "按上面的位置放一个插件目录，重开 Keepline 后会显示在这里。",
                div(),
            ));
        }
        for f in found {
            let name = f
                .dir
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            page = page.child(match &f.result {
                Ok(p) => self.row(
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
                    status(true, "已加载".into()),
                ),
                Err(e) => self.row(&name, e, status(false, "没有加载".into())),
            });
        }
        let actions = keepline_core::action::report();
        if !actions.is_empty() {
            page = page.child(div().mt(px(T::S16)).child(label("动作文件")));
            for a in actions {
                let file = a.file.display().to_string();
                page = page.child(match &a.result {
                    Ok(titles) => self.row(
                        &file,
                        &format!("加进右键菜单和 ⌘P：{titles}"),
                        status(true, "生效".into()),
                    ),
                    Err(e) => self.row(&file, e, status(false, "没有生效".into())),
                });
            }
        }
        let rules = keepline_core::rule::report();
        if !rules.is_empty() {
            page = page.child(div().mt(px(T::S16)).child(label("验证规则文件")));
            for r in rules {
                let file = r.file.display().to_string();
                page = page.child(match &r.result {
                    Ok(list) => self.row(
                        &file,
                        &format!("这些命令算跑过验证：{list}"),
                        status(true, "生效".into()),
                    ),
                    Err(e) => self.row(&file, e, status(false, "没有生效".into())),
                });
            }
        }
        let hooks = keepline_core::hook::report();
        if !hooks.is_empty() {
            page = page.child(div().mt(px(T::S16)).child(label("Hooks 文件")));
            for r in hooks {
                let file = r.file.display().to_string();
                page = page.child(match &r.result {
                    Ok(list) => self.row(
                        &file,
                        &format!("会话状态变化时由后台调用：{list}；结果写在后台日志里"),
                        status(true, "生效".into()),
                    ),
                    Err(e) => self.row(&file, e, status(false, "没有生效".into())),
                });
            }
        }
        let sidebars = keepline_core::sidebar::report();
        if !sidebars.is_empty() {
            page = page.child(div().mt(px(T::S16)).child(label("左栏文件")));
            for r in sidebars {
                let file = r.file.display().to_string();
                page = page.child(match &r.result {
                    Ok(list) => self.row(
                        &file,
                        &format!("左栏顶部多了：{list}"),
                        status(true, "生效".into()),
                    ),
                    Err(e) => self.row(&file, e, status(false, "没有生效".into())),
                });
            }
        }
        let declared = agent::report();
        if !declared.is_empty() {
            page = page.child(div().mt(px(T::S16)).child(label("Agent 声明文件")));
            for d in declared {
                let file = d.file.display().to_string();
                page = page.child(match &d.result {
                    Ok(key) => {
                        self.row(&file, &format!("注册为 {key}"), status(true, "生效".into()))
                    }
                    Err(e) => self.row(&file, e, status(false, "没有生效".into())),
                });
            }
        }
        page
    }
    fn page_matches(&self, page: Page, cx: &gpui::App) -> bool {
        match page {
            Page::General => {
                self.matches(page, "默认 Agent", "新建待办和未指定 Agent 的会话", cx)
                    || self.matches(
                        page,
                        "回来时显示总览",
                        "离开窗口达到这段时间后，返回时先看工作进展",
                        cx,
                    )
                    || self.matches(
                        page,
                        "旧会话收起时间",
                        "已关闭的会话超过这段时间后从侧栏收起，历史仍保留",
                        cx,
                    )
            }
            Page::Look => {
                self.matches(page, "主题", "跟随系统 亮色 暗色 明暗", cx)
                    || self.matches(
                        page,
                        "终端字体",
                        "字体 family font JetBrains Mono Menlo Monaco",
                        cx,
                    )
                    || self.matches(page, "字号", "文字大小 font size 缩放", cx)
                    || self.matches(page, "行距", "行高 line height 紧凑 舒适 宽松", cx)
                    || self.matches(page, "光标闪烁", "光标 闪烁 blink cursor", cx)
                    || self.matches(page, "非当前分屏变暗", "分屏 变暗 焦点 dim split", cx)
                    || self.matches(
                        page,
                        "非当前分屏的光标",
                        "分屏 光标 空心 焦点 cursor split",
                        cx,
                    )
            }
            Page::Keys => {
                Keymap::actions().any(|a| self.matches(page, &a.label(), a.section().label(), cx))
                    || self.matches(page, "切换到第几个标签", "⌘1 ⌘9", cx)
            }
            Page::Notifications => {
                self.matches(page, "桌面通知", "Agent 等待输入 错误 完成 macOS 提醒", cx)
            }
            Page::Plugins => self.matches(page, "插件", "plugin 扩展 声明 agent json", cx),
        }
    }
    fn body(&self, cx: &Context<Self>) -> gpui::Div {
        let searching = !self.query.read(cx).text.trim().is_empty();
        let mut body = div().flex().flex_col().gap(px(T::S20));
        let mut found = false;
        for page in Page::ALL {
            if (!searching && page != self.page) || (searching && !self.page_matches(page, cx)) {
                continue;
            }
            found = true;
            let content = match page {
                Page::General => self.general_page(cx),
                Page::Look => self.look_page(cx),
                Page::Keys => self.keys_page(cx),
                Page::Notifications => self.notifications_page(cx),
                Page::Plugins => self.plugins_page(),
            };
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(T::S8))
                    .child(
                        div()
                            .text_size(px(T::FORM_TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child(page.label()),
                    )
                    .when(!searching, |d| d.child(label(page.description())))
                    .child(content),
            );
        }
        if !found {
            body = body
                .child(div().py(px(T::S20)).child("没有找到相关设置"))
                .child(label("试试「字体」「通知」或「快捷键」"));
        }
        body
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.body(cx);
        let query_focused = self.query.clone();
        div()
            .id("settings-panel")
            .w(px(T::SETTINGS_WIDTH))
            .max_w_full()
            .h(px(T::HERE_HEIGHT + T::CLEANUP_ACTION_HEIGHT))
            .max_h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::popover()))
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
                    .p(px(T::S20))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(T::color(T::primary()))
                            .child("设置"),
                    )
                    .child(
                        button("settings-close", "关闭 · Esc")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsAction::Close))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(
                        div()
                            .w(px(T::SETTINGS_NAV))
                            .flex_shrink_0()
                            .p(px(T::S12))
                            .flex()
                            .flex_col()
                            .gap(px(T::S6))
                            .border_r(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::border()))
                            .children(
                                Page::ALL
                                    .into_iter()
                                    .map(|page| self.page_button(page, page.label(), cx)),
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
                                    .px(px(T::S20))
                                    .py(px(T::S12))
                                    .child(self.query.clone()),
                            )
                            .child(
                                div()
                                    .id("settings-body")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .px(px(T::S20))
                                    .pb(px(T::S20))
                                    .child(body),
                            ),
                    ),
            )
            .child(
                div()
                    .px(px(T::S20))
                    .py(px(T::S12))
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        div().flex_1().child(label(
                            self.save_error
                                .as_ref()
                                .or(self.text_error.as_ref())
                                .cloned()
                                .unwrap_or_else(|| {
                                    if self.busy {
                                        "正在保存…".into()
                                    } else {
                                        "更改自动保存 · 立即生效".into()
                                    }
                                }),
                        )),
                    )
                    .when(self.page == Page::Keys, |d| {
                        d.child(
                            secondary("settings-reset-all", "全部恢复默认")
                                .on_click(cx.listener(|this, _, _, cx| this.reset(None, cx))),
                        )
                    }),
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
        let view = cx.new(|cx| SettingsView::new(values, busy, cx));
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
            let mut values = v.values.clone();
            values.sound = !values.sound;
            cx.emit(SettingsAction::Change(values));
        });
    }
    pub(crate) fn visual_settings_values(&self) -> Settings {
        self.workspace.settings.clone()
    }
    pub(crate) fn visual_settings_query(&self, cx: &gpui::App) -> Entity<Field> {
        self.settings.as_ref().unwrap().read(cx).query.clone()
    }
}
