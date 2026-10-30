//! Main-thread CPU of GPUI frames for the A14 resource loads, without a display link,
//! daemon or foreground window. Each frame feeds one pane the next R8/R5 10 Hz load frame
//! and calls Window::draw (layout, paint, scene); the other panes replay as cached views,
//! as in the workbench. GPU submission/present is not included.
//!
//! cargo run --release -p keepline-app --features visual-test --example paint_bench -- \
//!     --panes 4 --frames 2000
#[path = "../src/model/mod.rs"]
#[allow(dead_code)]
mod model;
#[path = "../src/terminal/screen.rs"]
#[allow(dead_code)]
mod screen;
#[path = "../src/ui/theme.rs"]
#[allow(dead_code)]
mod theme;
// The shared files name each other by their paths in the app crate.
#[allow(unused_imports)]
mod terminal {
    pub(crate) use super::screen;
}
#[path = "../src/ui/status_colors.rs"]
#[allow(dead_code)]
mod status_colors;
#[path = "../src/ui/terminal_text.rs"]
#[allow(dead_code)]
mod terminal_text;
mod ui {
    pub(crate) use super::terminal_text;
    pub(crate) use super::theme;
}

use gpui::{
    App, Bounds, Context, Entity, Font, IntoElement, Render, Window, WindowBounds, WindowOptions,
    canvas, div, prelude::*, px, size,
};
use keepline_core::terminal::Vt;

struct Pane {
    vt: Vt,
    screen: screen::Screen,
    capture: screen::Capture,
    cache: screen::LayoutCache,
    font: Font,
    width: usize,
    frame: u64,
}
impl Pane {
    // Same bytes as r8 load-one.py (60-char body) / r5 load.py (40-char body).
    fn step(&mut self) {
        self.frame += 1;
        let rows: Vec<String> = (0..20)
            .map(|row| {
                format!(
                    "{:08} row {row:02} {}",
                    self.frame,
                    &"0123456789abcdef".repeat(4)[..self.width]
                )
            })
            .collect();
        self.vt
            .feed(format!("\x1b[H{}", rows.join("\r\n")).as_bytes());
    }
}
impl Render for Pane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.capture.update(&self.vt, &mut self.screen).unwrap();
        let entity = cx.entity();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                entity.update(cx, |p, cx| {
                    let metrics = screen::CellMetrics::measure(&p.font, window).unwrap();
                    screen::paint_cached(
                        &p.screen,
                        &p.font,
                        bounds,
                        metrics,
                        screen::CursorPaint::Shown,
                        &mut p.cache,
                        window,
                        cx,
                    );
                })
            },
        )
        .size_full()
    }
}

struct Grid(Vec<Entity<Pane>>);
impl Render for Grid {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let cell = |p: &Entity<Pane>| {
            div()
                .flex_1()
                .min_h_0()
                .child(p.clone().cached(gpui::StyleRefinement {
                    size: gpui::SizeRefinement {
                        width: Some(gpui::relative(1.).into()),
                        height: Some(gpui::relative(1.).into()),
                    },
                    ..Default::default()
                }))
        };
        let column = |ps: &[Entity<Pane>]| {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(1.))
                .children(ps.iter().map(cell))
        };
        let half = self.0.len().div_ceil(2);
        div()
            .size_full()
            .flex()
            .gap(px(1.))
            .child(column(&self.0[..half]))
            .when(self.0.len() > 1, |d| d.child(column(&self.0[half..])))
    }
}

fn thread_cpu() -> f64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 + t.tv_nsec as f64 / 1e9
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str, default: usize| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .map_or(default, |v| v.parse().unwrap())
    };
    let panes = value("--panes", 1);
    let frames = value("--frames", 1000);
    gpui_platform::application().run(move |cx: &mut App| {
        let font = theme::Theme::mono(cx);
        // The R5/R8 Keepline terminal area: about 1190 x 710 points.
        let bounds = Bounds::new(gpui::point(px(0.), px(0.)), size(px(1190.), px(710.)));
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    focus: false,
                    ..Default::default()
                },
                |window, cx| {
                    let m = screen::CellMetrics::measure(&font, window).unwrap();
                    let (cols, rows) = if panes == 1 { (1, 1) } else { (2, 2) };
                    let c = ((bounds.size.width / m.width) as u16 / cols).max(2);
                    let r = ((bounds.size.height / m.height) as u16 / rows).max(1);
                    let width = if panes == 1 { 60 } else { 40 };
                    let entities = (0..panes)
                        .map(|_| {
                            cx.new(|_| {
                                let mut vt = Vt::new(c, r).unwrap();
                                vt.feed(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[HREADY\r\n");
                                Pane {
                                    vt,
                                    screen: Default::default(),
                                    capture: screen::Capture::new().unwrap(),
                                    cache: Default::default(),
                                    font: font.clone(),
                                    width,
                                    frame: 0,
                                }
                            })
                        })
                        .collect();
                    cx.new(|_| Grid(entities))
                },
            )
            .unwrap();
        let panes = handle.update(cx, |grid, _, _| grid.0.clone()).unwrap();
        // Draw through the window, not the root view: Window::draw renders Grid itself.
        cx.update_window(handle.into(), |_, window, app| {
                let mut run = |n: usize| {
                    for i in 0..n {
                        let pane = &panes[i % panes.len()];
                        pane.update(app, |p, cx| {
                            p.step();
                            cx.notify();
                        });
                        window.draw(app).clear(app);
                    }
                };
                run(50); // Warm glyph atlas, layout and ASCII caches.
                let start = thread_cpu();
                let wall = std::time::Instant::now();
                run(frames);
                let cpu = thread_cpu() - start;
                println!(
                    "{{\"panes\":{},\"frames\":{frames},\"main_thread_cpu_s\":{cpu:.4},\"cpu_us_per_frame\":{:.1},\"wall_s\":{:.3}}}",
                    panes.len(),
                    cpu / frames as f64 * 1e6,
                    wall.elapsed().as_secs_f64()
                );
                window.remove_window();
        })
        .unwrap();
        cx.quit();
    });
}
