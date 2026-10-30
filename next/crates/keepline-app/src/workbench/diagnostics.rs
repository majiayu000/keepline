//! An independently updated, disposable view of a short host measurement.
use super::*;
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedSender, unbounded},
};
use keepline_core::diagnostics::{AppUsage, Diagnostics};
use std::{
    io,
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) enum DiagnosticAction {
    Close,
    Terminal(String),
}

/// Dropping a view (or pressing Stop) shuts its socket, including a pending connect.
struct Sampling {
    socket: Arc<Mutex<Option<UnixStream>>>,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Sampling {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(stream) = self.socket.lock().unwrap().take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}
impl Sampling {
    fn start(path: PathBuf, tx: UnboundedSender<io::Result<Diagnostics>>) -> Self {
        let result = Self {
            socket: Arc::new(Mutex::new(None)),
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let socket = result.socket.clone();
        let cancelled = result.cancelled.clone();
        std::thread::spawn(move || {
            let read = || -> io::Result<()> {
                let mut stream = UnixStream::connect(path)?;
                {
                    let mut slot = socket.lock().unwrap();
                    if cancelled.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    *slot = Some(stream.try_clone()?);
                }
                stream.set_read_timeout(Some(Duration::from_secs(25)))?;
                stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                send(&mut stream, CONTROL, &Request::Hello { major: MAJOR })?;
                if !matches!(
                    receive::<Reply>(&mut stream)?,
                    Reply::Hello { major: MAJOR, .. }
                ) {
                    return Err(io::Error::other("诊断连接握手失败"));
                }
                send(&mut stream, CONTROL, &Request::Diagnose)?;
                loop {
                    let report = match receive::<Reply>(&mut stream)? {
                        Reply::Diagnostics(report) => report,
                        Reply::Error(error) => return Err(io::Error::other(error)),
                        _ => return Err(io::Error::other("诊断响应无效")),
                    };
                    let complete = report.complete;
                    if tx.unbounded_send(Ok(report)).is_err() || complete {
                        return Ok(());
                    }
                }
            };
            if let Err(error) = read() {
                if !cancelled.load(Ordering::Acquire) {
                    let _ = tx.unbounded_send(Err(error));
                }
            }
        });
        result
    }
}

pub(super) struct DiagnosticView {
    socket: PathBuf,
    sampling: Option<Sampling>,
    report: Option<Diagnostics>,
    error: Option<String>,
    copied: bool,
    terminals: HashMap<String, String>,
    focus: gpui::FocusHandle,
}
impl gpui::EventEmitter<DiagnosticAction> for DiagnosticView {}
impl DiagnosticView {
    fn new(socket: PathBuf, terminals: HashMap<String, String>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            socket,
            sampling: None,
            report: None,
            error: None,
            copied: false,
            terminals,
            focus: cx.focus_handle(),
        };
        view.start(cx);
        view
    }
    fn start(&mut self, cx: &mut Context<Self>) {
        self.report = None;
        self.error = None;
        self.copied = false;
        let (tx, mut rx) = unbounded();
        let sampling = Sampling::start(self.socket.clone(), tx);
        let cancelled = sampling.cancelled.clone();
        self.sampling = Some(sampling);
        cx.spawn(async move |this, cx| {
            while let Some(result) = rx.next().await {
                // A buffered result from a stopped run must not replace a newer run.
                if cancelled.load(Ordering::Acquire) {
                    break;
                }
                if this
                    .update(cx, |this, cx| {
                        match result {
                            Ok(report) => {
                                if report.complete {
                                    this.sampling = None;
                                }
                                this.report = Some(report);
                            }
                            Err(error) => {
                                this.error = Some(format!("采样未完成：{error}"));
                                this.sampling = None;
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(report) = &self.report {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(report.summary()));
            self.copied = true;
            cx.notify();
        }
    }
    fn app_row(&self, app: &AppUsage, gpu: bool, cx: &Context<Self>) -> gpui::Div {
        let value = if gpu {
            let share = app.gpu_share_percent.unwrap_or_default();
            if share > 0. && share < 0.1 {
                "<0.1%".into()
            } else {
                format!("{share:.1}%")
            }
        } else {
            format!("{:.1}%", app.cpu_percent)
        };
        let mut row = div()
            .py(px(T::S8))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .flex()
            .flex_col()
            .gap(px(T::S4))
            .child(
                div()
                    .flex()
                    .gap(px(T::S12))
                    .child(div().flex_1().min_w_0().truncate().child(app.name.clone()))
                    .child(
                        div()
                            .text_color(T::color(T::primary()))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    ),
            );
        if !gpu {
            row = row.child(label(format!(
                "区间峰值 {:.1}% · RSS {:.0} MiB · {} 个进程",
                app.cpu_peak_percent,
                app.rss_bytes as f64 / 1_048_576.,
                app.processes
            )));
        }
        for id in &app.terminals {
            if let Some(name) = self.terminals.get(id) {
                let id = id.clone();
                row = row.child(
                    button(
                        gpui::SharedString::from(format!("diag-{}-{gpu}-{id}", app.name)),
                        format!("打开终端 · {name} →"),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(DiagnosticAction::Terminal(id.clone()))
                    })),
                );
            }
        }
        row
    }
}
impl Render for DiagnosticView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.sampling.is_some();
        let mut body = div()
            .id("diagnostic-body")
            .overflow_y_scroll()
            .min_h_0()
            .flex_1()
            .p(px(T::S20))
            .flex()
            .flex_col()
            .gap(px(T::S16));
        if let Some(report) = &self.report {
            let percent =
                |v: Option<f64>| v.map_or_else(|| "不可读".into(), |p| format!("{p:.1}%"));
            let metric = |name: &str, value: String| {
                div()
                    .flex_1()
                    .p(px(T::S14))
                    .rounded(px(T::CARD_RADIUS))
                    .bg(T::color(T::raised()))
                    .child(label(name.to_owned()))
                    .child(
                        div()
                            .text_size(px(T::TITLE))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    )
            };
            body = body.child(
                div()
                    .flex()
                    .gap(px(T::S12))
                    .child(metric("整机 CPU · 采样平均", percent(report.cpu_percent)))
                    .child(metric("GPU 设备 · 最新活跃度", percent(report.gpu_percent)))
                    .child(metric("已采样", format!("{:.1} 秒", report.elapsed_secs))),
            );
            let mut cpu = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("CPU 应用排行"),
                )
                .child(label("平均占用 · 100% 表示一个核心"));
            for app in report.apps.iter().filter(|a| a.cpu_percent > 0.).take(12) {
                cpu = cpu.child(self.app_row(app, false, cx));
            }
            let mut gpu = div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("GPU 活动排行"),
                )
                .child(label("已观测活动的份额 · 不代表设备利用率"));
            let mut apps: Vec<_> = report
                .apps
                .iter()
                .filter(|a| a.gpu_share_percent.is_some())
                .collect();
            apps.sort_by(|a, b| {
                b.gpu_share_percent
                    .unwrap()
                    .total_cmp(&a.gpu_share_percent.unwrap())
            });
            if apps.is_empty() {
                gpu = gpu.child(label("没有可比较的 GPU 活动数据，不代表 GPU 空闲。"));
            }
            for app in apps.into_iter().take(10) {
                gpu = gpu.child(self.app_row(app, true, cx));
            }
            body = body
                .child(label(format!(
                    "{} · {} 个进程 · 开始于{}",
                    if running {
                        "采样中，每 7.5 秒更新"
                    } else if report.complete {
                        "采样已结束，不再刷新"
                    } else {
                        "采样已停止，以下为部分结果"
                    },
                    report.process_count,
                    ago(report.started_at, now())
                )))
                .child(
                    div()
                        .flex()
                        .gap(px(T::S20))
                        .items_start()
                        .child(cpu)
                        .child(gpu),
                );
            for note in &report.notes {
                body = body.child(label(note.clone()));
            }
        } else {
            body = body.child(div().py(px(T::S20)).child(if running {
                "正在读取本机资源，约 8 秒后显示第一组结果…"
            } else {
                "采样已停止，尚无可比较的数据。"
            }));
        }
        if let Some(error) = &self.error {
            body = body.child(
                div()
                    .text_color(Group::Error.colors().1)
                    .child(error.clone()),
            );
        }
        div()
            .id("diagnostic-panel")
            .w(px(T::CLEANUP_WIDTH))
            .max_w_full()
            .h(px(T::HERE_HEIGHT))
            .max_h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .shadow_lg()
            .track_focus(&self.focus)
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape"
                    || (event.keystroke.key == "w" && event.keystroke.modifiers.platform)
                {
                    cx.emit(DiagnosticAction::Close);
                } else if event.keystroke.key == "c" && event.keystroke.modifiers.platform {
                    this.copy(cx);
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .p(px(T::S20))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        div()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(T::TITLE))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("本机诊断"),
                            )
                            .child(label("看谁在占 CPU / GPU，再把证据交给 Agent 分析")),
                    )
                    .child(
                        button("diagnostic-close", "关闭 · Esc")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DiagnosticAction::Close))),
                    ),
            )
            .child(body)
            .child(
                div()
                    .p(px(T::S14))
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(label("仅本机读取 · 约 15 秒 · 结束后停止采样"))
                    .child(div().flex_1())
                    .when(self.report.is_some(), |d| {
                        d.child(
                            button(
                                "diagnostic-copy",
                                if self.copied {
                                    "已复制 ✓"
                                } else {
                                    "复制摘要给 Agent"
                                },
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                        )
                    })
                    .child(
                        button(
                            "diagnostic-run",
                            if running {
                                "停止采样"
                            } else {
                                "重新采样"
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.sampling.is_some() {
                                this.sampling = None;
                                cx.notify();
                            } else {
                                this.start(cx);
                            }
                        })),
                    ),
            )
    }
}

impl Workbench {
    #[cfg(feature = "visual-test")]
    pub fn visual_open_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_diagnostics(window, cx);
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_diagnostic_report(&self, cx: &gpui::App) -> Option<Diagnostics> {
        self.diagnostics.as_ref()?.read(cx).report.clone()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_diagnostic_closed(&self) -> bool {
        self.diagnostics.is_none()
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_open_diagnostic_terminal(&mut self, cx: &mut Context<Self>) {
        let view = self.diagnostics.as_ref().unwrap().clone();
        let terminal = view
            .read(cx)
            .report
            .as_ref()
            .unwrap()
            .apps
            .iter()
            .flat_map(|app| &app.terminals)
            .next()
            .expect("no managed terminal in native report")
            .clone();
        view.update(cx, |_, cx| cx.emit(DiagnosticAction::Terminal(terminal)));
    }
    #[cfg(feature = "visual-test")]
    pub fn visual_copy_diagnostics(&mut self, cx: &mut Context<Self>) {
        let original = cx.read_from_clipboard();
        let view = self.diagnostics.as_ref().unwrap().clone();
        let expected = view.read(cx).report.as_ref().unwrap().summary();
        view.update(cx, |view, cx| view.copy(cx));
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        if let Some(original) = original {
            cx.write_to_clipboard(original);
        }
        assert_eq!(copied.as_deref(), Some(expected.as_str()));
    }

    pub(super) fn show_diagnostics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.hide();
        let terminals = self
            .workspace
            .terminals
            .iter()
            .map(|t| {
                let title = self
                    .workspace
                    .tabs
                    .iter()
                    .find(|tab| tab.layout.as_ref().is_some_and(|l| l.contains(&t.id)))
                    .map(|tab| tab.title.clone())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| home_relative(&t.cwd));
                (t.id.clone(), title)
            })
            .collect();
        let view = cx.new(|cx| DiagnosticView::new(self.store.socket().to_owned(), terminals, cx));
        cx.subscribe(&view, |this, _, action, cx| {
            this.diagnostics = None;
            this.nav.pending_focus = true;
            if let DiagnosticAction::Terminal(id) = action {
                if let Some(tab) = this
                    .workspace
                    .tabs
                    .iter()
                    .find(|tab| tab.layout.as_ref().is_some_and(|l| l.contains(id)))
                {
                    this.nav.space = tab.space.clone();
                    this.nav.tab = Some(tab.id.clone());
                    this.nav.focused = Some(id.clone());
                    this.nav.overview = false;
                    this.nav.task_detail = None;
                    this.nav.zoomed = None;
                    this.tasks.form = None;
                    this.overview.cleanup.clear();
                    this.sync_panes(cx);
                }
            }
            cx.notify();
        })
        .detach();
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        self.diagnostics = Some(view);
        cx.notify();
    }
    pub(super) fn diagnostic_overlay(
        &self,
        view: Entity<DiagnosticView>,
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
                        this.diagnostics = None;
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
