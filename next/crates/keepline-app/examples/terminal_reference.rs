//! Renders raw VT fixtures with Keepline's real terminal painter (screen.rs) to PNG,
//! for pixel comparison against Ghostty. Runs in its own process: no daemon, no
//! single-instance lock, so it never touches a running Keepline Workbench.
//!
//! cargo run -p keepline-app --features visual-test --example terminal_reference -- \
//!     --cols 100 --rows 30 --out out/keepline fixtures/*.vt
//! KL_REF_FONT=Menlo overrides Theme::mono when Ghostty cannot load the same family.
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
mod ui {
    pub(crate) use super::theme;
}

use gpui::{
    App, Bounds, Context, Font, IntoElement, Render, Window, WindowBounds, WindowOptions, canvas,
    div, font, prelude::*, px, size,
};
use keepline_core::terminal::Vt;
use std::{path::PathBuf, time::Duration};

struct Fixture {
    screen: screen::Screen,
    font: Font,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div().size_full().child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, cx| {
                    let (sc, font) =
                        entity.update(cx, |f, _| (std::mem::take(&mut f.screen), f.font.clone()));
                    let metrics = measure(&font, window);
                    screen::paint_cached(
                        &sc,
                        &font,
                        bounds,
                        metrics,
                        true,
                        &mut screen::LayoutCache::default(),
                        window,
                        cx,
                    );
                    entity.update(cx, |f, _| f.screen = sc);
                },
            )
            .size_full(),
        )
    }
}

fn measure(font: &Font, window: &Window) -> screen::CellMetrics {
    screen::CellMetrics::measure(font, window).expect("native font measurement")
}

fn pick_font(cx: &App) -> Font {
    match std::env::var("KL_REF_FONT") {
        Ok(family) => {
            let mut f = font(family);
            let terminal = theme::Theme::mono(cx);
            f.fallbacks = terminal.fallbacks;
            f.features = terminal.features;
            f
        }
        Err(_) => theme::Theme::mono(cx),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let cols: u16 = value("--cols").map_or(100, |v| v.parse().unwrap());
    let rows: u16 = value("--rows").map_or(30, |v| v.parse().unwrap());
    let out = PathBuf::from(value("--out").unwrap_or_else(|| "out/keepline".into()));
    let skip: Vec<&String> = ["--cols", "--rows", "--out"]
        .iter()
        .filter_map(|f| args.iter().position(|a| a == f).map(|i| &args[i + 1]))
        .collect();
    let fixtures: Vec<PathBuf> = args
        .iter()
        .filter(|a| !a.starts_with("--") && !skip.contains(a))
        .map(PathBuf::from)
        .collect();
    std::fs::create_dir_all(&out).unwrap();
    register_symbols();
    gpui_platform::application().run(move |cx: &mut App| {
        let font = pick_font(cx);
        // Size the window from the same metrics the painter uses, so the grid fills it exactly.
        let probe = cx
            .open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| Fixture {
                    screen: Default::default(),
                    font: font.clone(),
                })
            })
            .unwrap();
        let metrics = probe
            .update(cx, |_, window, _| measure(&font, window))
            .unwrap();
        probe
            .update(cx, |_, w, a| screen::verify_native_geometry(w, a))
            .unwrap();
        probe.update(cx, |_, w, _| w.remove_window()).unwrap();
        let grid = size(metrics.width * cols as f32, metrics.height * rows as f32);
        eprintln!(
            "keepline font={} size={} cell={:?}x{:?} ratio(h/w)={:.4} grid={grid:?}",
            font.family,
            screen::font_size(),
            metrics.width,
            metrics.height,
            f32::from(metrics.height) / f32::from(metrics.width)
        );
        cx.spawn(async move |cx| {
            for path in fixtures {
                let mut vt = Vt::new(cols, rows).unwrap();
                vt.feed(&std::fs::read(&path).unwrap());
                let sc = screen::capture(&vt).unwrap();
                let font = font.clone();
                let handle = cx
                    .update(|cx| {
                        let bounds = Bounds::new(gpui::point(px(0.), px(0.)), grid);
                        cx.open_window(
                            WindowOptions {
                                window_bounds: Some(WindowBounds::Windowed(bounds)),
                                titlebar: None,
                                focus: false,
                                ..Default::default()
                            },
                            |_, cx| cx.new(|_| Fixture { screen: sc, font }),
                        )
                    })
                    .unwrap();
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let name = path.file_stem().unwrap().to_string_lossy().into_owned();
                let file = out.join(format!("{name}.png"));
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh();
                    window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(&file).unwrap();
                    eprintln!(
                        "keepline {name}: scale={} -> {}",
                        window.scale_factor(),
                        file.display()
                    );
                    window.remove_window();
                })
                .unwrap();
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
}

/// Mirrors main.rs register_symbols: the bundled Nerd symbol face must be registered
/// with CoreText in this process for the same fallback cascade.
fn register_symbols() {
    use std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGDataProviderCreateWithData(
            info: *mut c_void,
            data: *const c_void,
            size: usize,
            release: Option<unsafe extern "C" fn(*mut c_void, *const c_void, usize)>,
        ) -> *const c_void;
        fn CGFontCreateWithDataProvider(provider: *const c_void) -> *const c_void;
        fn CGDataProviderRelease(provider: *const c_void);
    }
    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTFontManagerRegisterGraphicsFont(
            font: *const c_void,
            error: *mut *const c_void,
        ) -> bool;
    }
    let bytes = include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf");
    unsafe {
        let provider = CGDataProviderCreateWithData(
            std::ptr::null_mut(),
            bytes.as_ptr().cast(),
            bytes.len(),
            None,
        );
        let font = CGFontCreateWithDataProvider(provider);
        CGDataProviderRelease(provider);
        assert!(
            CTFontManagerRegisterGraphicsFont(font, std::ptr::null_mut()),
            "register bundled symbol font"
        );
    }
}
