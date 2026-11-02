//! 手机: the title bar's phone button and its panel (direction B in
//! `specs/extensions/design/phone-pairing.html`). The button shows once phones are on or
//! paired: green with a count while any is connected, so a phone typing here is never
//! out of sight. The panel lists the paired phones, what each connected one watches and
//! last did, disconnects one, changes its role or unpairs it, and pairs a new one with a
//! QR code. The relay's address and key live on the settings page 「手机与远程」.
use super::*;
use keepline_core::protocol::{RemoteChange, RemoteDevice, RemotePhone, RemoteState, RemoteStatus};

const PANEL_WIDTH: f32 = 360.;
/// One QR module, and the light margin around the code that scanners need.
const QR_MODULE: f32 = 4.;
const QR_QUIET: usize = 3;
/// The phone outline: its size, corner, edge, and the dot on its corner while connected.
const GLYPH_WIDTH: f32 = 9.;
const GLYPH_HEIGHT: f32 = 13.;
const GLYPH_RADIUS: f32 = 2.;
const GLYPH_EDGE: f32 = 1.5;
const GLYPH_DOT: f32 = 6.;
const GLYPH_DOT_OUT: f32 = -4.;
const ROLES: [(&str, &str); 3] = [
    ("view", "只看"),
    ("answer", "能答审批"),
    ("full", "完全控制"),
];

#[derive(Default)]
pub(super) struct Phones {
    pub(super) open: bool,
    /// The device whose role and unpair row is unfolded.
    more: Option<String>,
    pairing: Option<Pairing>,
    /// Why the last change was refused.
    note: Option<String>,
}

struct Pairing {
    name: Entity<Field>,
    role: &'static str,
    /// The link and its code, once the host paired the phone.
    link: Option<(String, Qr)>,
    busy: bool,
}

/// Dark modules, row by row.
struct Qr {
    width: usize,
    dark: Vec<bool>,
}

pub(super) fn role_label(role: &str) -> &'static str {
    ROLES
        .iter()
        .find(|(r, _)| *r == role)
        .map_or("", |(_, l)| *l)
}

fn ago(ms_or_s: u64, millis: bool) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let at = if millis { ms_or_s / 1000 } else { ms_or_s };
    match now.saturating_sub(at) {
        s if s < 10 => "刚刚".into(),
        s if s < 60 => format!("{s} 秒前"),
        s if s < 3600 => format!("{} 分钟前", s / 60),
        s if s < 86400 => format!("{} 小时前", s / 3600),
        s => format!("{} 天前", s / 86400),
    }
}

fn act_label(op: &str) -> &'static str {
    match op {
        "keys" => "按了键",
        "send" => "发了文字",
        "answer" => "答了审批",
        _ => "操作过",
    }
}

/// A small phone outline; filled with a dot while connected.
fn phone_glyph(color: gpui::Hsla, live: bool) -> gpui::Div {
    div()
        .relative()
        .w(px(GLYPH_WIDTH))
        .h(px(GLYPH_HEIGHT))
        .flex_shrink_0()
        .rounded(px(GLYPH_RADIUS))
        .border(px(GLYPH_EDGE))
        .border_color(color)
        .when(live, |d| {
            d.child(
                div()
                    .absolute()
                    .right(px(GLYPH_DOT_OUT))
                    .top(px(GLYPH_DOT_OUT))
                    .size(px(GLYPH_DOT))
                    .rounded_full()
                    .bg(color),
            )
        })
}

fn live_color() -> gpui::Hsla {
    Group::Running.colors().0
}

/// The connection in a few words, with the color of its dot.
pub(super) fn state_line(r: &RemoteStatus) -> (gpui::Hsla, String) {
    match r.live.state {
        RemoteState::Connected => (live_color(), "中继已连上".into()),
        RemoteState::Connecting if r.live.detail.is_empty() => {
            (T::color(T::secondary()), "正在连中继…".into())
        }
        RemoteState::Connecting => (T::color(T::secondary()), r.live.detail.clone()),
        RemoteState::Failed => (
            Group::Error.colors().0,
            format!("没连上：{}", r.live.detail),
        ),
        RemoteState::Off => (T::color(T::muted()), "已关闭".into()),
    }
}

impl Workbench {
    /// The title bar's phone button; absent until phones are on or one is paired.
    pub(super) fn phones_button(&self, cx: &Context<Self>) -> Option<gpui::Stateful<gpui::Div>> {
        let r = &self.store.recognition().remote;
        if !r.enabled && r.devices.is_empty() {
            return None;
        }
        let online = r.live.online.len();
        let color = match r.live.state {
            RemoteState::Connected if online > 0 => live_color(),
            RemoteState::Failed => Group::Error.colors().0,
            _ => T::color(T::secondary()),
        };
        let open = self.phones.open;
        Some(
            button("phones", "")
                .aria_label(if online > 0 {
                    format!("手机：{online} 台在线")
                } else {
                    "手机".into()
                })
                .gap(px(T::S6))
                .px(px(T::S8))
                .when(open, |d| d.bg(T::color(T::selection())))
                .child(phone_glyph(color, online > 0))
                .when(online > 0, |d| {
                    d.child(
                        div()
                            .font_features(T::tabular())
                            .text_color(color)
                            .child(online.to_string()),
                    )
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.phones.open = !this.phones.open;
                    this.phones.more = None;
                    this.phones.note = None;
                    if !this.phones.open {
                        this.phones.pairing = None;
                    }
                    cx.notify();
                })),
        )
    }

    fn close_phones(&mut self, cx: &mut Context<Self>) {
        self.phones = Phones::default();
        cx.notify();
    }

    /// Sends a change to the host; a refusal shows in the panel.
    fn phones_request(
        &mut self,
        request: Request,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, Reply, &mut Context<Self>) + 'static,
    ) {
        self.phones.note = None;
        let task = cx.background_spawn(self.store.send(request));
        cx.spawn(async move |this, cx| {
            let reply = task.await;
            let _ = this.update(cx, |this, cx| {
                let failed = match reply {
                    Ok(Reply::Error(e)) => Some(e),
                    Ok(reply) => {
                        done(this, reply, cx);
                        None
                    }
                    Err(e) => Some(e.to_string()),
                };
                if let Some(e) = failed {
                    this.phones.note = Some(e);
                    if let Some(p) = this.phones.pairing.as_mut() {
                        p.busy = false;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn change_device(&mut self, device: String, change: RemoteChange, cx: &mut Context<Self>) {
        self.phones.more = None;
        self.phones_request(Request::RemoteDevice { device, change }, cx, |_, _, _| ());
    }

    fn start_pairing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = cx.new(|cx| {
            Field::new("手机名字", "", cx)
                .compact()
                .placeholder("这台手机叫什么，比如 iPhone")
        });
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        window.focus(&name.focus_handle(cx), cx);
        self.phones.pairing = Some(Pairing {
            name,
            role: "full",
            link: None,
            busy: false,
        });
        self.phones.more = None;
        cx.notify();
    }

    fn pair(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.phones.pairing.as_mut().filter(|p| !p.busy) else {
            return;
        };
        p.busy = true;
        let request = Request::RemotePair {
            name: p.name.read(cx).text.trim().to_owned(),
            role: p.role.to_owned(),
        };
        self.phones_request(request, cx, |this, reply, _| {
            let Some(p) = this.phones.pairing.as_mut() else {
                return;
            };
            p.busy = false;
            if let Reply::Text(link) = reply {
                match qrcode::QrCode::new(link.as_bytes()) {
                    Ok(code) => {
                        let dark = code
                            .to_colors()
                            .into_iter()
                            .map(|c| c == qrcode::Color::Dark)
                            .collect();
                        p.link = Some((
                            link,
                            Qr {
                                width: code.width(),
                                dark,
                            },
                        ));
                    }
                    Err(e) => this.phones.note = Some(e.to_string()),
                }
            }
        });
    }

    /// A terminal as people know it: its tab's title.
    fn terminal_name(&self, terminal: &str) -> String {
        self.workspace
            .tabs
            .iter()
            .find(|t| {
                self.workspace
                    .tab_terminals(&t.id)
                    .iter()
                    .any(|id| id == terminal)
            })
            .map(|t| t.title.clone())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| terminal.chars().take(8).collect())
    }

    fn device_row(
        &self,
        d: &RemoteDevice,
        phone: Option<&RemotePhone>,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let color = if phone.is_some() {
            live_color()
        } else {
            T::color(T::secondary())
        };
        let detail = match phone {
            Some(p) => {
                let mut parts = vec![];
                if let Some(t) = p.watching.first() {
                    parts.push(format!("在看「{}」", self.terminal_name(t)));
                } else {
                    parts.push("在线".into());
                }
                if let Some(a) = &p.last {
                    parts.push(format!("{}{}", ago(a.at, true), act_label(&a.op)));
                }
                parts.join(" · ")
            }
            None => d.last_seen.map_or("还没连过".into(), |at| {
                format!("{}连过", ago(at, false))
            }),
        };
        let id = d.id.clone();
        let unfolded = self.phones.more.as_deref() == Some(d.id.as_str());
        let row = div()
            .flex()
            .items_center()
            .gap(px(T::S10))
            .px(px(T::S8))
            .py(px(T::S8))
            .rounded(px(T::CONTROL_RADIUS))
            .bg(T::color(T::raised()))
            .child(phone_glyph(color, phone.is_some()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(T::S2))
                    .child(
                        div()
                            .text_size(px(T::SMALL))
                            .text_color(T::color(T::primary()))
                            .child(format!("{} · {}", d.name, role_label(&d.role))),
                    )
                    .child(label(detail).truncate()),
            )
            .when(phone.is_some(), |r| {
                let id = id.clone();
                r.child(
                    button(format!("kick-{id}"), "断开")
                        .h(px(T::PILL_HEIGHT))
                        .px(px(T::S8))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::control()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_device(id.clone(), RemoteChange::Kick, cx)
                        })),
                )
            })
            .child({
                let id = id.clone();
                button(format!("more-{id}"), "…")
                    .aria_label("改权限或取消配对")
                    .h(px(T::PILL_HEIGHT))
                    .px(px(T::S6))
                    .text_color(T::color(T::secondary()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.phones.more = if this.phones.more.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                        cx.notify();
                    }))
            });
        let mut block = div().flex().flex_col().gap(px(T::S4)).child(row);
        if unfolded {
            let mut roles = div().flex().items_center().gap(px(T::S4));
            for (role, text) in ROLES {
                let on = d.role == role;
                let id = id.clone();
                roles = roles.child(
                    button(format!("role-{id}-{role}"), text)
                        .h(px(T::PILL_HEIGHT))
                        .px(px(T::S8))
                        .text_size(px(T::META))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(if on { T::focus() } else { T::control() }))
                        .when(on, |b| b.bg(T::color(T::selection())))
                        .when(!on, |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                this.change_device(id.clone(), RemoteChange::Role(role.into()), cx)
                            }))
                        }),
                );
            }
            block = block.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(T::S8))
                    .child(roles)
                    .child(
                        button(format!("revoke-{id}"), "取消配对")
                            .h(px(T::PILL_HEIGHT))
                            .px(px(T::S8))
                            .text_size(px(T::META))
                            .text_color(Group::Error.colors().1)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_device(id.clone(), RemoteChange::Revoke, cx)
                            })),
                    ),
            );
        }
        block.into_any_element()
    }

    fn pairing_view(&self, p: &Pairing, cx: &Context<Self>) -> gpui::Div {
        if let Some((link, qr)) = &p.link {
            let side = (qr.width + QR_QUIET * 2) as f32 * QR_MODULE;
            let mut code = div()
                .w(px(side))
                .h(px(side))
                .p(px(QR_QUIET as f32 * QR_MODULE))
                .bg(gpui::white())
                .rounded(px(T::CONTROL_RADIUS))
                .flex()
                .flex_col();
            // Runs of dark modules, so a row is a few boxes rather than one per module.
            for row in qr.dark.chunks(qr.width) {
                let mut line = div().h(px(QR_MODULE)).flex();
                let mut x = 0;
                while x < row.len() {
                    let run = row[x..].iter().take_while(|&&d| d == row[x]).count();
                    line = line.child(
                        div()
                            .w(px(run as f32 * QR_MODULE))
                            .h_full()
                            .when(row[x], |d| d.bg(gpui::black())),
                    );
                    x += run;
                }
                code = code.child(line);
            }
            let link = link.clone();
            return div()
                .flex()
                .flex_col()
                .gap(px(T::S10))
                .child(div().flex().justify_center().child(code))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(T::S4))
                        .text_size(px(T::SMALL))
                        .child(format!(
                            "用 Keepline 手机 App 扫这个码：「{}」 · {}",
                            p.name.read(cx).text.trim(),
                            role_label(p.role)
                        ))
                        .child(label(
                            "这个码只给这一台手机，别截图发给别人；关掉后不再显示。",
                        )),
                )
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(
                            button("copy-link", self.copy_label("copy-link", "复制链接"))
                                .border(px(T::BORDER_WIDTH))
                                .border_color(T::color(T::control()))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.copy_text("copy-link", link.clone(), cx)
                                })),
                        )
                        .child(button("pair-done", "完成").on_click(cx.listener(
                            |this, _, _, cx| {
                                this.phones.pairing = None;
                                cx.notify();
                            },
                        ))),
                );
        }
        let mut roles = div().flex().items_center().gap(px(T::S4));
        for (role, text) in ROLES {
            let on = p.role == role;
            roles = roles.child(
                button(format!("pair-role-{role}"), text)
                    .h(px(T::PILL_HEIGHT))
                    .px(px(T::S8))
                    .text_size(px(T::META))
                    .border(px(T::BORDER_WIDTH))
                    .border_color(T::color(if on { T::focus() } else { T::control() }))
                    .when(on, |b| b.bg(T::color(T::selection())))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(p) = this.phones.pairing.as_mut() {
                            p.role = role;
                        }
                        cx.notify();
                    })),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(T::S10))
            .child(div().text_size(px(T::SMALL)).child("配对新手机"))
            .child(p.name.clone())
            .child(roles)
            .child(
                div()
                    .flex()
                    .justify_between()
                    .child(
                        button("pair-cancel", "取消").on_click(cx.listener(|this, _, _, cx| {
                            this.phones.pairing = None;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "pair-go",
                            if p.busy {
                                "正在生成…"
                            } else {
                                "生成二维码"
                            },
                        )
                        .bg(T::color(T::accent()))
                        .text_color(T::color(T::on_accent()))
                        .on_click(cx.listener(|this, _, _, cx| this.pair(cx))),
                    ),
            )
    }

    /// The panel under the phone button; a click outside closes it.
    pub(super) fn phones_panel(&self, window: &Window, cx: &Context<Self>) -> gpui::AnyElement {
        let r = &self.store.recognition().remote;
        let (dot, state) = state_line(r);
        let mut body = div().flex().flex_col().gap(px(T::S8));
        if let Some(p) = &self.phones.pairing {
            body = body.child(self.pairing_view(p, cx));
        } else {
            if r.devices.is_empty() {
                body = body.child(label("还没有配对的手机"));
            }
            for d in &r.devices {
                let phone = r.live.online.iter().find(|p| p.device == d.id);
                body = body.child(self.device_row(d, phone, cx));
            }
            let can_pair = !r.relay.is_empty();
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(px(T::S4))
                    .child(
                        button("pair-new", "配对新手机…")
                            .when(can_pair, |b| {
                                b.bg(T::color(T::accent()))
                                    .text_color(T::color(T::on_accent()))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_pairing(window, cx)
                                    }))
                            })
                            .when(!can_pair, |b| b.opacity(T::DISABLED_OPACITY)),
                    )
                    .child(
                        button("phones-settings", "设置与记录")
                            .text_color(T::color(T::secondary()))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_phones(cx);
                                this.show_settings_page(settings::Page::Phones, window, cx);
                            })),
                    ),
            );
            if !can_pair {
                body = body.child(label("先在「设置与记录」里填中继地址"));
            }
        }
        if let Some(note) = &self.phones.note {
            body = body.child(label(note.clone()).text_color(Group::Error.colors().1));
        }
        let viewport = window.viewport_size();
        gpui::deferred(
            div()
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| this.close_phones(cx)),
                )
                .child(
                    div()
                        .id("phones-panel")
                        .absolute()
                        .top(px(T::TOP))
                        .left((viewport.width - px(PANEL_WIDTH + T::S8)).max(px(0.)))
                        .w(px(PANEL_WIDTH))
                        .p(px(T::S10))
                        .flex()
                        .flex_col()
                        .gap(px(T::S10))
                        .bg(T::color(T::popover()))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::control()))
                        .rounded(px(T::POPOVER_RADIUS))
                        .shadow_lg()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(T::SMALL))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child("手机"),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(T::S6))
                                        .min_w_0()
                                        .child(div().size(px(T::DOT)).rounded_full().bg(dot))
                                        .child(label(state).truncate()),
                                ),
                        )
                        .child(body),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

#[cfg(feature = "visual-test")]
impl Workbench {
    pub(crate) fn visual_remote(&self) -> RemoteStatus {
        self.store.recognition().remote.clone()
    }
    pub(crate) fn visual_phones(
        &mut self,
        open: bool,
        more: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.settings = None;
        self.phones = Phones {
            open,
            more,
            ..Default::default()
        };
        cx.notify();
    }
    pub(crate) fn visual_phones_pair(
        &mut self,
        name: &str,
        role: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.phones.open = true;
        self.start_pairing(window, cx);
        if let Some(p) = self.phones.pairing.as_mut() {
            p.role = role;
            p.name.update(cx, |f, cx| {
                f.set_text(name);
                cx.notify();
            });
        }
        self.pair(cx);
    }
    pub(crate) fn visual_phones_paired(&self) -> bool {
        self.phones
            .pairing
            .as_ref()
            .is_some_and(|p| p.link.is_some())
    }
    pub(crate) fn visual_phones_kick(&mut self, device: &str, cx: &mut Context<Self>) {
        self.change_device(device.into(), RemoteChange::Kick, cx);
    }
    pub(crate) fn visual_phones_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_phones(cx);
        self.show_settings_page(settings::Page::Phones, window, cx);
    }
}
