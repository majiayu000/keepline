//! An independently updated, disposable view of a short host measurement.
use super::*;
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedSender, unbounded},
};
use keepline_core::diagnostics::{AppUsage, Diagnostics, mib};
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
    /// Workbench terminal id → the title the summary and the chips show.
    terminals: HashMap<String, String>,
    /// The app whose terminal chips are expanded.
    open: Option<String>,
    all_rows: bool,
    notes_open: bool,
    focus: gpui::FocusHandle,
}
impl gpui::EventEmitter<DiagnosticAction> for DiagnosticView {}

/// Rows the CPU ranking shows before「再显示」.
const FIRST_ROWS: usize = 10;
/// Apps that get their own chart color; the rest share one.
const COLORED: usize = 5;

/// A hover card: a bold first line, then quieter detail lines.
struct Tip(Vec<gpui::SharedString>);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut card = div()
            .max_w(px(T::TIP_WIDTH))
            .px(px(T::S10))
            .py(px(T::S8))
            .rounded(px(T::CONTROL_RADIUS))
            .bg(T::color(T::primary()))
            .text_color(T::color(T::popover()))
            .text_size(px(T::SMALL))
            .font_features(tabular())
            .shadow_lg();
        for (i, line) in self.0.iter().enumerate() {
            card = card.child(if i == 0 {
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(line.clone())
            } else {
                div().child(line.clone())
            });
        }
        card
    }
}
fn tip(lines: Vec<String>) -> impl Fn(&mut Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let lines: Vec<gpui::SharedString> = lines.into_iter().map(Into::into).collect();
    move |_, cx| cx.new(|_| Tip(lines.clone())).into()
}
/// Digits of equal width, so columns of numbers line up.
fn tabular() -> gpui::FontFeatures {
    gpui::FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}
fn cores(percent: f64) -> String {
    if percent < 100. {
        format!("{:.2}", percent / 100.)
    } else {
        format!("{:.1}", percent / 100.)
    }
}
/// An axis end that is a round step past the largest value, never below `floor`.
fn axis_max(largest: f64, step: f64, floor: f64) -> f64 {
    ((largest / step).ceil() * step).max(floor)
}
fn heading(text: &'static str, aside: &'static str) -> gpui::Div {
    div()
        .flex()
        .items_baseline()
        .justify_between()
        .gap(px(T::S12))
        .mb(px(T::S8))
        .child(
            div()
                .text_color(T::color(T::primary()))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(text),
        )
        .child(
            div()
                .text_size(px(T::META))
                .text_color(T::color(T::muted()))
                .child(aside),
        )
}
fn metric(name: &'static str, value: String, unit: &'static str, hint: String) -> gpui::Div {
    div()
        .flex_1()
        .min_w_0()
        .px(px(T::S20))
        .py(px(T::S14))
        .flex()
        .flex_col()
        .gap(px(T::S4))
        .child(label(name))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(T::S6))
                .child(
                    div()
                        .text_size(px(T::BIG_NUMBER))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(T::color(T::primary()))
                        .child(value),
                )
                .child(div().text_color(T::color(T::secondary())).child(unit)),
        )
        .child(
            div()
                .text_size(px(T::SMALL))
                .text_color(T::color(T::muted()))
                .child(hint),
        )
}
/// An axis label centered on its tick; the two ends align inward so they stay inside.
fn tick(text: String, at: f32) -> gpui::Div {
    let label = div().absolute().top_0().whitespace_nowrap().child(text);
    if at <= 0.01 {
        label.left_0()
    } else if at >= 0.99 {
        label.right_0()
    } else {
        label
            .left(gpui::relative(at))
            .ml(px(-T::TICK_LABEL / 2.))
            .w(px(T::TICK_LABEL))
            .flex()
            .justify_center()
    }
}
/// Tick labels over a bar column: the grid shares the bar plot's x scale.
fn axis(max: f64, step: f64, reference: Option<f64>, label_width: f32) -> gpui::Div {
    let mut ticks = div().flex_1().relative().h(px(T::TICK_ROW));
    let mut v = 0.;
    while v <= max + 0.01 {
        let text = if Some(v) == reference {
            "1 核".to_owned()
        } else {
            format!("{v:.0}%")
        };
        let mut tick = tick(text, (v / max) as f32);
        if Some(v) == reference {
            tick = tick
                .text_color(T::color(T::secondary()))
                .font_weight(gpui::FontWeight::SEMIBOLD);
        }
        ticks = ticks.child(tick);
        v += step;
    }
    div()
        .flex()
        .gap(px(T::S10))
        .text_size(px(T::CHIP_SIZE))
        .text_color(T::color(T::muted()))
        .child(div().w(px(label_width)).flex_shrink_0())
        .child(ticks)
        .child(div().w(px(T::BAR_VALUE)).flex_shrink_0())
}
/// One bar on the shared scale, with vertical grid lines and an optional dashed reference.
fn plot(value: f64, max: f64, step: f64, reference: Option<f64>, color: u32) -> gpui::Div {
    let mut plot = div().flex_1().min_w_0().relative().h(px(T::ROW));
    let mut v = step;
    while v < max + 0.01 {
        let line = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(gpui::relative((v / max) as f32));
        plot = plot.child(if Some(v) == reference {
            line.border_l(px(T::BORDER_WIDTH))
                .border_dashed()
                .border_color(T::color(T::chart_ref()))
        } else {
            line.w(px(T::BORDER_WIDTH)).bg(T::color(T::chart_grid()))
        });
        v += step;
    }
    plot.child(
        div()
            .absolute()
            .left_0()
            .top(px((T::ROW - T::BAR) / 2.))
            .h(px(T::BAR))
            .min_w(px(T::S2))
            .w(gpui::relative((value / max).min(1.) as f32))
            .rounded_r(px(T::CHIP_RADIUS))
            .bg(T::color(color)),
    )
}
fn bar_row(
    id: gpui::SharedString,
    name: &str,
    extra: gpui::Div,
    plot: gpui::Div,
    value: String,
    label_width: f32,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(T::S10))
        .min_h(px(T::ROW))
        .border_b(px(T::BORDER_WIDTH))
        .border_color(T::color(T::border()))
        .hover(|s| s.bg(T::color(T::raised())))
        .child(
            div()
                .w(px(label_width))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(T::S6))
                .overflow_hidden()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(T::color(T::primary()))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(name.to_owned()),
                )
                .child(extra),
        )
        .child(plot)
        .child(
            div()
                .w(px(T::BAR_VALUE))
                .flex_shrink_0()
                .text_right()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(T::color(T::primary()))
                .child(value),
        )
}
fn chip(text: String) -> gpui::Div {
    div()
        .flex_shrink_0()
        .px(px(T::S6))
        .rounded(px(T::CHIP_RADIUS))
        .bg(T::color(T::chip()))
        .text_size(px(T::CHIP_SIZE))
        .text_color(T::color(T::secondary()))
        .child(text)
}

impl DiagnosticView {
    fn new(socket: PathBuf, terminals: HashMap<String, String>, cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            socket,
            sampling: None,
            report: None,
            error: None,
            copied: false,
            terminals,
            open: None,
            all_rows: false,
            notes_open: false,
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
    fn summary(&self) -> Option<String> {
        let report = self.report.as_ref()?;
        Some(report.summary(&clock(report.started_at), &self.terminals))
    }
    fn copy(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.summary() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
            self.copied = true;
            cx.notify();
        }
    }
    /// Where the whole machine's cores went: colored apps, the rest of the ranking,
    /// busy time no app accounts for, and idle.
    fn core_stack(&self, report: &Diagnostics) -> Option<gpui::Div> {
        let total = report.cpu_cores? as f64;
        let busy = report.cpu_percent? / 100. * total;
        let ranked: Vec<&AppUsage> = report.ranked().collect();
        let rest: f64 = ranked
            .iter()
            .skip(COLORED)
            .map(|a| a.cpu_percent)
            .sum::<f64>()
            / 100.;
        let unattributed = report.unattributed_cores()?;
        // (name, cores, color, hatched, detail)
        let mut segments: Vec<(String, f64, u32, bool, String)> = ranked
            .iter()
            .take(COLORED)
            .enumerate()
            .map(|(i, a)| {
                (
                    a.name.clone(),
                    a.cpu_percent / 100.,
                    T::series(i),
                    false,
                    format!("{} 个进程", a.processes),
                )
            })
            .collect();
        if ranked.len() > COLORED {
            segments.push((
                format!("排行其余 {} 项", ranked.len() - COLORED),
                rest,
                T::series_other(),
                false,
                String::new(),
            ));
        }
        segments.push((
            "未归属".into(),
            unattributed,
            T::hatch(),
            true,
            "排行之外或采样间隙里的短命进程".into(),
        ));
        let idle = (total - busy).max(0.);
        if idle >= 0.05 {
            segments.push(("空闲".into(), idle, T::chart_grid(), false, String::new()));
        }
        let mut stack = div().flex().gap(px(T::S2)).h(px(T::STACK_HEIGHT));
        let mut legend = div()
            .mt(px(T::S10))
            .grid()
            .grid_cols(4)
            .gap_x(px(T::S20))
            .gap_y(px(T::S6));
        let last = segments.len() - 1;
        for (i, (name, value, color, hatched, detail)) in segments.into_iter().enumerate() {
            let fill = |d: gpui::Div| {
                if hatched {
                    d.bg(gpui::pattern_slash(T::color(color), T::S2, T::S6))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(color))
                } else {
                    d.bg(T::color(color))
                }
            };
            let mut lines = vec![
                name.clone(),
                format!("{value:.2} 核 · 整机 {:.1}%", value / total * 100.),
            ];
            if !detail.is_empty() {
                lines.push(detail);
            }
            let mut segment = fill(div())
                .id(gpui::SharedString::from(format!("diag-core-{i}")))
                .h_full()
                .min_w(px(T::STEP_HEIGHT))
                .flex_grow(value as f32)
                .flex_basis(gpui::relative(0.))
                .tooltip(tip(lines));
            if i == 0 {
                segment = segment.rounded_l(px(T::CHIP_RADIUS));
            }
            if i == last {
                segment = segment.rounded_r(px(T::CHIP_RADIUS));
            }
            stack = stack.child(segment);
            legend = legend.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .min_w_0()
                    .text_size(px(T::SMALL))
                    .child(fill(
                        div().size(px(T::SWATCH)).flex_shrink_0().rounded(px(T::S2)),
                    ))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_color(T::color(T::body()))
                            .child(name),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(T::color(T::secondary()))
                            .child(format!("{value:.2} 核")),
                    ),
            );
        }
        let mut scale = div()
            .relative()
            .h(px(T::TICK_ROW))
            .mt(px(T::S4))
            .text_size(px(T::CHIP_SIZE))
            .text_color(T::color(T::muted()));
        for q in 0..=4 {
            let at = q as f32 / 4.;
            let text = format!(
                "{:.0}{}",
                total * at as f64,
                if q == 4 { " 核" } else { "" }
            );
            scale = scale.child(tick(text, at));
        }
        let mut section = div()
            .px(px(T::S20))
            .pt(px(T::S16))
            .pb(px(T::S16))
            .border_b(px(T::BORDER_WIDTH))
            .border_color(T::color(T::border()))
            .child(heading(
                "全部核心都在忙什么",
                "宽度 = 平均占用的核心数 · 悬停看明细",
            ))
            .child(stack)
            .child(scale)
            .child(legend);
        if unattributed >= 0.5 {
            section = section.child(
                div()
                    .mt(px(T::S12))
                    .px(px(T::S12))
                    .py(px(T::S8))
                    .rounded(px(T::CONTROL_RADIUS))
                    .bg(T::color(T::raised()))
                    .flex()
                    .gap(px(T::S8))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()))
                    .child(
                        div()
                            .flex_shrink_0()
                            .px(px(T::S6))
                            .rounded(px(T::CHIP_RADIUS))
                            .border(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::control()))
                            .text_size(px(T::CHIP_SIZE))
                            .child("推测"),
                    )
                    .child(div().flex_1().min_w_0().child(format!(
                        "约 {unattributed:.1} 核没有归到排行里的应用。可能来自排行之外的进程，或在两次采样之间出生又退出的短命进程（排行只统计前后都在的进程）。要确认，需要抓进程创建记录。"
                    ))),
            );
        }
        Some(section)
    }
    fn cpu_ranking(&self, report: &Diagnostics, cx: &Context<Self>) -> gpui::Div {
        let ranked: Vec<&AppUsage> = report.ranked().collect();
        let max = axis_max(ranked.first().map_or(0., |a| a.cpu_percent), 50., 100.);
        let step = if max <= 200. { 50. } else { 100. };
        let mut column = div()
            .flex_1()
            .min_w_0()
            .px(px(T::S20))
            .py(px(T::S16))
            .child(heading("CPU 占用排行", "平均 · 虚线 = 1 个核心"))
            .child(axis(max, step, Some(100.), T::BAR_LABEL));
        let shown = if self.all_rows {
            ranked.len()
        } else {
            ranked.len().min(FIRST_ROWS)
        };
        for (i, app) in ranked.iter().take(shown).enumerate() {
            let named: Vec<(String, String)> = app
                .terminals
                .iter()
                .filter_map(|id| self.terminals.get(id).map(|t| (id.clone(), t.clone())))
                .collect();
            let open = self.open.as_deref() == Some(app.name.as_str()) && !named.is_empty();
            let mut extra = div().flex().items_center().gap(px(T::S4)).flex_shrink_0();
            if app.processes > 1 {
                extra = extra.child(chip(format!("×{}", app.processes)));
            }
            if !named.is_empty() {
                let name = app.name.clone();
                extra = extra.child(
                    div()
                        .id(gpui::SharedString::from(format!("diag-terms-{}", app.name)))
                        .flex_shrink_0()
                        .px(px(T::S6))
                        .rounded(px(T::S8))
                        .border(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::control()))
                        .text_size(px(T::CHIP_SIZE))
                        .text_color(T::color(T::link()))
                        .cursor_pointer()
                        .hover(|s| s.bg(T::color(T::button_hover())))
                        .child(format!(
                            "{} 终端 {}",
                            named.len(),
                            if open { "▴" } else { "▾" }
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open =
                                (this.open.as_deref() != Some(name.as_str())).then(|| name.clone());
                            cx.notify();
                        })),
                );
            }
            let mut lines = vec![
                app.name.clone(),
                format!(
                    "平均 {:.1}% ≈ {} 核",
                    app.cpu_percent,
                    cores(app.cpu_percent)
                ),
            ];
            if app.cpu_peak_percent > app.cpu_percent + 0.05 {
                lines.push(format!("区间峰值 {:.1}%", app.cpu_peak_percent));
            }
            lines.push(format!(
                "RSS {} · {} 个进程",
                mib(app.rss_bytes),
                app.processes
            ));
            let color = if i < COLORED {
                T::series(i)
            } else {
                T::series_other()
            };
            let mut bars = plot(app.cpu_percent, max, step, Some(100.), color);
            if app.cpu_peak_percent > app.cpu_percent + 0.05 {
                bars = bars.child(
                    div()
                        .absolute()
                        .top(px((T::ROW - T::TICK_ROW) / 2.))
                        .h(px(T::TICK_ROW))
                        .w(px(T::S2))
                        .rounded(px(T::BORDER_WIDTH))
                        .bg(T::color(T::primary()))
                        .left(gpui::relative((app.cpu_peak_percent / max).min(1.) as f32)),
                );
            }
            column = column.child(
                bar_row(
                    format!("diag-cpu-{}", app.name).into(),
                    &app.name,
                    extra,
                    bars,
                    format!("{:.1}%", app.cpu_percent),
                    T::BAR_LABEL,
                )
                .tooltip(tip(lines)),
            );
            if open {
                let mut list = div().flex().flex_wrap().gap(px(T::S6)).py(px(T::S8));
                for (id, title) in named {
                    list = list.child(
                        button(
                            gpui::SharedString::from(format!("diag-term-{id}")),
                            format!("{title} →"),
                        )
                        .h(px(T::TICK_ROW + T::S6))
                        .px(px(T::S8))
                        .bg(T::color(T::button()))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DiagnosticAction::Terminal(id.clone()))
                        })),
                    );
                }
                column = column.child(
                    list.border_b(px(T::BORDER_WIDTH))
                        .border_color(T::color(T::border())),
                );
            }
        }
        if ranked.len() > FIRST_ROWS {
            column = column.child(
                button(
                    "diagnostic-more",
                    if self.all_rows {
                        "收起".to_owned()
                    } else {
                        format!("再显示 {} 项", ranked.len() - FIRST_ROWS)
                    },
                )
                .mt(px(T::S6))
                .px(px(T::S8))
                .text_color(T::color(T::link()))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.all_rows = !this.all_rows;
                    cx.notify();
                })),
            );
        }
        let one_interval = ranked
            .iter()
            .all(|a| a.cpu_peak_percent <= a.cpu_percent + 0.05);
        if one_interval && !ranked.is_empty() {
            column = column.child(
                div()
                    .mt(px(T::S10))
                    .text_size(px(T::META))
                    .text_color(T::color(T::muted()))
                    .child("只采到 1 个区间，峰值暂同平均；第 2 个区间后会出现峰值刻度。"),
            );
        }
        if ranked.is_empty() {
            column = column.child(label("这次没有测到 CPU 占用。"));
        }
        column
    }
    /// CPU on a linear x, resident memory on a log y: tells CPU hogs from memory hogs.
    fn scatter(&self, report: &Diagnostics) -> gpui::Div {
        const MB: f64 = 1_048_576.;
        let ranked: Vec<&AppUsage> = report.ranked().collect();
        let x_max = axis_max(ranked.first().map_or(0., |a| a.cpu_percent), 50., 100.);
        let x_step = if x_max <= 200. { 50. } else { 100. };
        let biggest = ranked
            .iter()
            .map(|a| a.rss_bytes as f64 / MB)
            .fold(8., f64::max);
        // Ticks every power of eight from 8 MB up past the largest app.
        let mut ticks = vec![8.];
        while *ticks.last().unwrap() < biggest {
            ticks.push(ticks.last().unwrap() * 8.);
        }
        // Headroom of one doubling above the largest app, not a whole empty tick band.
        let (lo, hi) = (4f64.log2(), (biggest * 2.).log2().max(16f64.log2()));
        ticks.retain(|mb| mb.log2() <= hi);
        let y = |mb: f64| (1. - (mb.max(4.).log2() - lo) / (hi - lo)) as f32;
        let x = |cpu: f64| (cpu / x_max).min(1.) as f32;
        // Label the three biggest CPU users and the two biggest memory users.
        let mut by_rss = ranked.clone();
        by_rss.sort_by_key(|a| std::cmp::Reverse(a.rss_bytes));
        let labelled: Vec<&str> = ranked
            .iter()
            .take(3)
            .chain(by_rss.iter().take(2))
            .map(|a| a.name.as_str())
            .collect();
        let mut area = div().flex_1().relative().h(px(T::SCATTER_HEIGHT));
        let mut gutter = div()
            .w(px(T::AXIS_GUTTER))
            .flex_shrink_0()
            .relative()
            .h(px(T::SCATTER_HEIGHT));
        for mb in &ticks {
            let at = y(*mb);
            area = area.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(gpui::relative(at))
                    .h(px(T::BORDER_WIDTH))
                    .bg(T::color(T::chart_grid())),
            );
            gutter = gutter.child(
                div()
                    .absolute()
                    .right(px(T::S6))
                    .top(gpui::relative(at))
                    .mt(px(-T::S8))
                    .whitespace_nowrap()
                    .child(if *mb >= 1024. {
                        format!("{:.0} GB", mb / 1024.)
                    } else {
                        format!("{mb:.0} MB")
                    }),
            );
        }
        let mut v = x_step;
        while v <= x_max + 0.01 {
            let line = div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(gpui::relative((v / x_max) as f32));
            area = area.child(if v == 100. {
                line.border_l(px(T::BORDER_WIDTH))
                    .border_dashed()
                    .border_color(T::color(T::chart_ref()))
            } else {
                line.w(px(T::BORDER_WIDTH)).bg(T::color(T::chart_grid()))
            });
            v += x_step;
        }
        for app in &ranked {
            let (px_, py_) = (x(app.cpu_percent), y(app.rss_bytes as f64 / MB));
            let named = labelled.contains(&app.name.as_str());
            area = area.child(
                div()
                    .id(gpui::SharedString::from(format!("diag-pt-{}", app.name)))
                    .absolute()
                    .left(gpui::relative(px_))
                    .top(gpui::relative(py_))
                    .ml(px(-T::POINT / 2.))
                    .mt(px(-T::POINT / 2.))
                    .size(px(T::POINT))
                    .rounded_full()
                    .border(px(T::S2))
                    .border_color(T::color(T::popover()))
                    .bg(T::color(if named { T::primary() } else { T::series(0) }))
                    .tooltip(tip(vec![
                        app.name.clone(),
                        format!("CPU {:.1}% · RSS {}", app.cpu_percent, mib(app.rss_bytes)),
                        format!("{} 个进程", app.processes),
                    ])),
            );
            if named {
                let text = div()
                    .absolute()
                    .top(gpui::relative(py_))
                    .mt(px(-T::S8))
                    .whitespace_nowrap()
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::body()))
                    .child(app.name.clone());
                area = area.child(if px_ > 0.6 {
                    text.right(gpui::relative(1. - px_)).mr(px(T::S8))
                } else {
                    text.left(gpui::relative(px_)).ml(px(T::S8))
                });
            }
        }
        let mut x_labels = div().flex_1().relative().h(px(T::TICK_ROW));
        let mut v = 0.;
        while v <= x_max + 0.01 {
            let text = if v == 100. {
                "1 核".to_owned()
            } else {
                format!("{v:.0}%")
            };
            x_labels = x_labels.child(tick(text, (v / x_max) as f32));
            v += x_step;
        }
        div()
            .child(heading("CPU × 内存", "纵轴为对数 · RSS 不等于物理内存"))
            .child(
                div()
                    .flex()
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .child(gutter)
                    .child(
                        area.border_l(px(T::BORDER_WIDTH))
                            .border_b(px(T::BORDER_WIDTH))
                            .border_color(T::color(T::control())),
                    ),
            )
            .child(
                div()
                    .flex()
                    .mt(px(T::S4))
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(T::color(T::muted()))
                    .child(div().w(px(T::AXIS_GUTTER)).flex_shrink_0())
                    .child(x_labels),
            )
    }
    fn gpu_ranking(&self, report: &Diagnostics) -> gpui::Div {
        let apps = report.gpu_ranked();
        let mut section = div()
            .mt(px(T::S20))
            .child(heading("GPU 活动份额", "已观测计数的占比"));
        if apps.is_empty() {
            return section.child(label("没有可比较的 GPU 活动数据，不代表 GPU 空闲。"));
        }
        let max = axis_max(apps[0].gpu_share_percent.unwrap_or_default(), 20., 20.);
        // The column is narrow: two intervals keep the labels apart.
        let step = max / 2.;
        let width = T::BAR_LABEL - T::S20;
        section = section.child(axis(max, step, None, width));
        for app in apps {
            let share = app.gpu_share_percent.unwrap_or_default();
            let value = if share > 0. && share < 0.1 {
                "<0.1%".into()
            } else {
                format!("{share:.1}%")
            };
            section = section.child(
                bar_row(
                    format!("diag-gpu-{}", app.name).into(),
                    &app.name,
                    div(),
                    plot(share, max, step, None, T::series(0)),
                    value,
                    width,
                )
                .tooltip(tip(vec![
                    app.name.clone(),
                    format!("已观测 GPU 活动的 {share:.1}%"),
                    "不是设备利用率".into(),
                ])),
            );
        }
        section.child(
            div()
                .mt(px(T::S10))
                .text_size(px(T::META))
                .text_color(T::color(T::muted()))
                .child("只统计驱动公开的计数增量；没有读数不代表占用为零。"),
        )
    }
}
impl Render for DiagnosticView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self.sampling.is_some();
        let mut body = div()
            .id("diagnostic-body")
            .overflow_y_scroll()
            .min_h_0()
            .flex_1();
        // Two sampling intervals; the first fills when the first report arrives.
        let done = self
            .report
            .as_ref()
            .map_or(0, |r| if r.complete { 2 } else { 1 });
        let mut steps = div().flex().gap(px(T::S2));
        for i in 0..2 {
            steps = steps.child(
                div()
                    .w(px(T::STEP_WIDTH))
                    .h(px(T::STEP_HEIGHT))
                    .rounded(px(T::S2))
                    .bg(T::color(if i < done {
                        T::secondary()
                    } else {
                        T::chart_grid()
                    })),
            );
        }
        let state = if running {
            "采样中"
        } else if self.report.as_ref().is_some_and(|r| r.complete) {
            "采样已结束"
        } else {
            "采样已停止"
        };
        let mut status = div()
            .flex()
            .items_center()
            .gap(px(T::S10))
            .text_size(px(T::SMALL))
            .text_color(T::color(T::secondary()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(T::S6))
                    .text_color(T::color(T::primary()))
                    .child(
                        div()
                            .size(px(T::DOT))
                            .rounded_full()
                            .bg(T::color(if running { T::primary() } else { T::muted() })),
                    )
                    .child(state),
            )
            .child(steps);
        if let Some(report) = &self.report {
            status = status
                .child(format!(
                    "已采 {:.1} 秒 · 区间 {done} / 2",
                    report.elapsed_secs
                ))
                .child(format!("· 开始于 {}", clock(report.started_at)));
        } else if running {
            status = status.child("约 8 秒后显示第一组结果…");
        }
        if let Some(report) = &self.report {
            let percent = |v: Option<f64>| v.map_or_else(|| "不可读".into(), |p| format!("{p:.1}"));
            let listed: f64 = report.ranked().map(|a| a.cpu_percent).sum::<f64>() / 100.;
            let busy_hint = match (report.cpu_cores, report.cpu_percent) {
                (Some(c), Some(p)) => format!("约 {:.1} / {c} 核在忙", p / 100. * c as f64),
                _ => "核心数不可读".into(),
            };
            body = body.child(
                div()
                    .flex()
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .child(metric(
                        "整机 CPU · 采样平均",
                        percent(report.cpu_percent),
                        "%",
                        busy_hint,
                    ))
                    .child(div().w(px(T::BORDER_WIDTH)).bg(T::color(T::border())))
                    .child(metric(
                        "GPU 设备 · 最新读数",
                        percent(report.gpu_percent),
                        "%",
                        "设备活跃度，不是各应用份额之和".into(),
                    ))
                    .child(div().w(px(T::BORDER_WIDTH)).bg(T::color(T::border())))
                    .child(metric(
                        "排行能解释的 CPU",
                        format!("{listed:.1}"),
                        "核",
                        format!(
                            "前 {} 项{} · 共 {} 个进程",
                            report.ranked().count(),
                            report
                                .cpu_cores
                                .map_or_else(String::new, |c| format!("，整机 {c} 核")),
                            report.process_count
                        ),
                    )),
            );
            if let Some(stack) = self.core_stack(report) {
                body = body.child(stack);
            }
            body = body.child(
                div()
                    .flex()
                    .items_start()
                    .child(self.cpu_ranking(report, cx))
                    .child(
                        div()
                            .w(px(T::BORDER_WIDTH))
                            .self_stretch()
                            .bg(T::color(T::border())),
                    )
                    .child(
                        div()
                            .w(gpui::relative(0.42))
                            .flex_shrink_0()
                            .px(px(T::S20))
                            .py(px(T::S16))
                            .child(self.scatter(report))
                            .child(self.gpu_ranking(report)),
                    ),
            );
        } else {
            body = body.child(div().p(px(T::S20)).child(if running {
                "正在读取本机资源，约 8 秒后显示第一组结果…"
            } else {
                "采样已停止，尚无可比较的数据。"
            }));
        }
        if let Some(error) = &self.error {
            body = body.child(
                div()
                    .px(px(T::S20))
                    .pb(px(T::S16))
                    .text_color(Group::Error.colors().1)
                    .child(error.clone()),
            );
        }
        let notes = self.report.as_ref().map_or(0, |r| r.notes.len());
        let mut panel = div()
            .id("diagnostic-panel")
            .w(px(T::DIAGNOSTIC_WIDTH))
            .max_w_full()
            .h(px(T::DIAGNOSTIC_HEIGHT))
            .max_h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(T::POPOVER_RADIUS))
            .bg(T::color(T::popover()))
            .border(px(T::BORDER_WIDTH))
            .border_color(T::color(T::control()))
            .font_features(tabular())
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
                    .px(px(T::S20))
                    .pt(px(T::S16))
                    .pb(px(T::S14))
                    .border_b(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .flex()
                    .items_center()
                    .gap(px(T::S12))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(T::S6))
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(T::S10))
                                    .child(
                                        div()
                                            .text_size(px(T::FORM_TITLE))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(T::color(T::primary()))
                                            .child("本机诊断"),
                                    )
                                    .child(label("看谁在占 CPU / GPU，再把证据交给 Agent")),
                            )
                            .child(status),
                    )
                    .child(
                        button("diagnostic-close", "关闭 · Esc")
                            .bg(T::color(T::button()))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DiagnosticAction::Close))),
                    ),
            )
            .child(body);
        if self.notes_open
            && let Some(report) = &self.report
        {
            {
                let mut list = div()
                    .px(px(T::S20))
                    .py(px(T::S10))
                    .border_t(px(T::BORDER_WIDTH))
                    .border_color(T::color(T::border()))
                    .bg(T::color(T::raised()))
                    .flex()
                    .flex_col()
                    .gap(px(T::S4))
                    .text_size(px(T::SMALL))
                    .text_color(T::color(T::secondary()));
                for note in &report.notes {
                    list = list.child(format!("· {note}"));
                }
                panel = panel.child(list);
            }
        }
        panel.child(
            div()
                .px(px(T::S20))
                .py(px(T::S12))
                .border_t(px(T::BORDER_WIDTH))
                .border_color(T::color(T::border()))
                .flex()
                .items_center()
                .gap(px(T::S10))
                .when(notes > 0, |d| {
                    d.child(
                        button(
                            "diagnostic-notes",
                            format!(
                                "ⓘ 读数说明 · {notes} 条 {}",
                                if self.notes_open { "▴" } else { "▾" }
                            ),
                        )
                        .px(px(T::S8))
                        .text_color(T::color(T::secondary()))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.notes_open = !this.notes_open;
                            cx.notify();
                        })),
                    )
                })
                .child(label("只在本机读取，结束后停止采样"))
                .child(div().flex_1())
                .child(
                    button(
                        "diagnostic-run",
                        if running {
                            "停止采样"
                        } else {
                            "重新采样"
                        },
                    )
                    .bg(T::color(T::button()))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.sampling.is_some() {
                            this.sampling = None;
                            cx.notify();
                        } else {
                            this.start(cx);
                        }
                    })),
                )
                .when(self.report.is_some(), |d| {
                    d.child(
                        button(
                            "diagnostic-copy",
                            if self.copied {
                                "已复制 ✓"
                            } else {
                                "复制摘要给 Agent · ⌘C"
                            },
                        )
                        .bg(T::color(T::accent()))
                        .text_color(T::color(T::on_accent()))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                    )
                }),
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
        let expected = view.read(cx).summary().unwrap();
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
