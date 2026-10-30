mod connection;
mod desktop;
#[cfg(feature = "visual-test")]
mod harness;
mod host;
mod keymap;
mod lifecycle;
mod model;
mod terminal;
mod ui;
mod workbench;

use ui::theme;

use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size,
};
use keepline_core::{model::*, protocol::*};
use std::{
    io,
    path::{Path, PathBuf},
};

fn ensure_host(root: &Path) -> io::Result<()> {
    if lifecycle::connected_host(&root.join("workbench.sock"))? {
        let Some(stale) = lifecycle::stale_build() else {
            return Ok(());
        };
        // A host that can hand its terminals over is updated at once, terminals and all.
        if stale.handoff {
            lifecycle::spawn_host(root, true)?;
            lifecycle::forget_stale_host();
            return Ok(());
        }
        // An older host with no terminal running is swapped for this app's own at once;
        // one with terminals waits for the user, since a restart ends them.
        let idle = matches!(
            rpc(&root.join("workbench.sock"), &Request::Workspace),
            Ok(Reply::Workspace(w)) if !w.terminals.iter().any(|t| t.state == ProcessState::Running)
        );
        if !idle {
            return Ok(());
        }
        lifecycle::stop_host(root, &stale)?;
        lifecycle::forget_stale_host();
    }
    lifecycle::spawn_host(root, false)
}

gpui::actions!(keepline, [Quit, ShowMenuPanel, ShowSettings]);
/// Keepline → 外观: one item per choice, the current one checked.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = keepline, no_json)]
pub(crate) struct ChooseAppearance(pub usize);
pub(crate) fn set_menus(cx: &mut App) {
    use ui::appearance::Appearance;
    let chosen = cx.try_global::<Appearance>().copied().unwrap_or_default();
    let looks = Appearance::ALL
        .iter()
        .enumerate()
        .map(|(i, a)| gpui::MenuItem::action(a.label(), ChooseAppearance(i)).checked(*a == chosen))
        .collect::<Vec<_>>();
    cx.set_menus([gpui::Menu::new("Keepline").items([
        gpui::MenuItem::action("设置…", ShowSettings),
        gpui::MenuItem::action("菜单栏面板", ShowMenuPanel),
        gpui::MenuItem::submenu(gpui::Menu::new("外观").items(looks)),
        gpui::MenuItem::separator(),
        gpui::MenuItem::action("退出 Keepline", Quit),
    ])]);
}

/// The top bar is the title bar: the traffic lights sit inside it, over the window's own gray.
fn window_options(bounds: Bounds<gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Keepline".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(
                px(theme::Theme::TRAFFIC_LIGHT_X),
                px(theme::Theme::TRAFFIC_LIGHT_Y),
            )),
        }),
        ..Default::default()
    }
}
fn load_workspace(root: &Path) -> io::Result<Workspace> {
    ensure_host(root)?;
    let socket = root.join("workbench.sock");
    let mut workspace = match rpc(&socket, &Request::Workspace)? {
        Reply::Workspace(w) => w,
        _ => return Err(io::Error::other("workspace response missing")),
    };
    if workspace.spaces.is_empty() {
        let cwd = default_space_dir()?.to_string_lossy().into_owned();
        if let Reply::Workspace(w) = rpc(
            &socket,
            &Request::Edit {
                revision: workspace.revision,
                edit: Edit::AddSpace(Space {
                    id: "default".into(),
                    name: "工作空间".into(),
                    cwd,
                }),
            },
        )? {
            workspace = w;
        }
    }
    Ok(workspace)
}
/// Finder and Dock start apps in "/", where nobody means to open a terminal: use the
/// home directory then, and the launch directory otherwise (`run.sh`, a shell).
fn default_space_dir() -> io::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(match std::env::var_os("HOME") {
        Some(home) if cwd == Path::new("/") => PathBuf::from(home),
        _ => cwd,
    })
}
fn reopen_window(root: &Path, cx: &mut App) {
    if let Some(handle) = cx.windows().first().copied() {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
        cx.activate(true);
        return;
    }
    match load_workspace(root) {
        Ok(workspace) => {
            let bounds = Bounds::centered(
                None,
                size(
                    px(theme::Theme::WINDOW_WIDTH),
                    px(theme::Theme::WINDOW_HEIGHT),
                ),
                cx,
            );
            if let Err(e) = cx.open_window(window_options(bounds), |window, cx| {
                cx.new(|cx| {
                    workbench::Workbench::new(root.join("workbench.sock"), workspace, window, cx)
                })
            }) {
                eprintln!("reopen: {e}");
            }
            cx.activate(true);
        }
        Err(e) => show_startup_error(e.to_string(), cx),
    }
}
struct StartupError {
    message: String,
    focus: gpui::FocusHandle,
}
impl gpui::Render for StartupError {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        use gpui::prelude::*;
        use theme::Theme as T;
        gpui::div()
            .size_full()
            .p(px(T::S16))
            .bg(T::color(T::app()))
            .text_color(T::color(T::primary()))
            .text_size(px(T::BODY_SIZE))
            .track_focus(&self.focus)
            .child(self.message.clone())
    }
}
fn show_startup_error(message: String, cx: &mut App) {
    eprintln!("Keepline: {message}");
    if let Err(e) = cx.open_window(WindowOptions::default(), |window, cx| {
        cx.new(|cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            StartupError { message, focus }
        })
    }) {
        eprintln!("error window: {e}");
    }
    cx.activate(true);
}
// Finder launches do not inherit run.sh. Enable the same event-driven idle policy
// before GPUI or any application thread is created. The retained back buffer redraws
// only what changed between frames, so a spinner repaints a few cells, not the window.
fn main() {
    // SAFETY: this is the first operation in main, before spawning threads.
    unsafe {
        std::env::set_var("GPUI_PAUSE_IDLE_DISPLAY_LINK", "1");
        std::env::set_var("GPUI_RETAINED_BACKBUFFER", "1");
    }
    record_panics();
    if let Err(error) = run() {
        eprintln!("Keepline: {error}");
        std::process::exit(1);
    }
}
/// The isolated runtime of a visual-evidence run, so every way out can stop its host.
#[cfg(feature = "visual-test")]
static EVIDENCE_RUNTIME: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
/// A visual-evidence run owns the host it started: the host outlives the app by design
/// (terminals survive a restart), so left alone it keeps the run's shells, and any load
/// a script typed into them, running with nobody attached. Ending the host closes its
/// PTYs and the shells with them.
#[cfg(feature = "visual-test")]
fn stop_evidence_host() {
    let Some(root) = EVIDENCE_RUNTIME.get() else {
        return;
    };
    let escaped: String = root
        .display()
        .to_string()
        .chars()
        .flat_map(|c| {
            let special = ".^$*+?()[]{}|\\".contains(c);
            special
                .then_some('\\')
                .into_iter()
                .chain(std::iter::once(c))
        })
        .collect();
    let _ = std::process::Command::new("/usr/bin/pkill")
        .args([
            "-TERM",
            "-f",
            &format!("keeplined serve --runtime {escaped}( |$)"),
        ])
        .status();
}
/// macOS crash reports keep only the stack, so every panic's message and place is also
/// appended to ~/Library/Logs/Keepline/crash.log. A failed visual-evidence check exits
/// with 101 instead of aborting, so a scripted run raises no "quit unexpectedly" dialog.
fn record_panics() {
    let log = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Logs/Keepline/crash.log"));
    let scripted = std::env::args().any(|arg| arg == "--visual-evidence");
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default(info);
        if let Some(log) = &log {
            use std::io::Write;
            let thread = std::thread::current();
            let entry = format!(
                "[{}] {} pid {} thread {}{}\n{info}\n{}\n\n",
                local_time(),
                std::env::args().collect::<Vec<_>>().join(" "),
                std::process::id(),
                thread.name().unwrap_or("unnamed"),
                if scripted { " (visual evidence)" } else { "" },
                std::backtrace::Backtrace::force_capture(),
            );
            let _ = std::fs::create_dir_all(log.parent().unwrap_or(Path::new("/")));
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log)
            {
                let _ = file.write_all(entry.as_bytes());
            }
        }
        if scripted {
            #[cfg(feature = "visual-test")]
            stop_evidence_host();
            let _ = io::Write::flush(&mut io::stderr());
            // SAFETY: ends the process at once, as the abort that follows a panic would.
            unsafe { libc::_exit(101) };
        }
    }));
}
fn local_time() -> String {
    let mut buf = [0u8; 32];
    // SAFETY: localtime_r and strftime write only into the locals handed to them.
    let len = unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm = std::mem::zeroed::<libc::tm>();
        libc::localtime_r(&now, &mut tm);
        libc::strftime(buf.as_mut_ptr().cast(), buf.len(), c"%F %T".as_ptr(), &tm)
    };
    String::from_utf8_lossy(&buf[..len]).into_owned()
}
fn run() -> io::Result<()> {
    // Declared CLIs first: menus, settings and commands all read the registry.
    keepline_core::agent::install();
    keepline_core::action::install();
    keepline_core::rule::install();
    let args: Vec<String> = std::env::args().collect();
    let root = if let Some(i) = args.iter().position(|a| a == "--runtime") {
        PathBuf::from(
            args.get(i + 1)
                .ok_or_else(|| io::Error::other("--runtime needs a directory"))?,
        )
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?)
            .join(".local/share/keepline-workbench")
    };
    let instance_dir = PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?,
    )
    .join(".local/share/keepline-client");
    // Visual evidence uses its explicitly isolated runtime without forwarding to or
    // replacing the user's running workbench.
    #[cfg(feature = "visual-test")]
    let instance_dir = if args.iter().any(|arg| arg == "--visual-evidence") {
        root.join("visual-client")
    } else {
        instance_dir
    };
    let (_instance, mut reopen) = match lifecycle::Instance::acquire(&instance_dir)? {
        lifecycle::Admission::Forwarded => return Ok(()),
        lifecycle::Admission::Primary(guard, events) => (guard, events),
    };
    let initial = load_workspace(&root);
    let icon_file = instance_dir.join("app-icon");
    #[cfg(feature = "visual-test")]
    let evidence = args
        .iter()
        .position(|a| a == "--visual-evidence")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    #[cfg(feature = "visual-test")]
    if evidence.is_some() {
        let _ = EVIDENCE_RUNTIME.set(root.clone());
    }
    let app = gpui_platform::application();
    let reopen_root = root.clone();
    app.on_reopen(move |cx| reopen_window(&reopen_root, cx));
    app.run(move |cx: &mut App| {
        use futures::StreamExt;
        cx.on_action(|_: &Quit, cx| cx.quit());
        #[cfg(feature = "visual-test")]
        cx.on_app_quit(|_| async { stop_evidence_host() }).detach();
        cx.on_action(|_: &ShowMenuPanel, _| desktop::show());
        cx.bind_keys([gpui::KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|choice: &ChooseAppearance, cx| {
            let Some(&appearance) = ui::appearance::Appearance::ALL.get(choice.0) else {
                return;
            };
            if let Err(error) = ui::appearance::choose(appearance, cx) {
                cx.set_global(desktop::Status(error));
            }
            set_menus(cx);
        });
        cx.on_action(|_: &ShowSettings, cx| {
            for handle in cx.windows() {
                if let Some(handle) = handle.downcast::<workbench::Workbench>() {
                    let _ = handle.update(cx, |view, window, cx| {
                        window.activate_window();
                        view.show_settings(window, cx);
                    });
                    break;
                }
            }
        });
        cx.set_global(ui::appearance::Appearance::load());
        cx.set_global(keymap::Keymap::load());
        ui::appearance::apply(cx);
        set_menus(cx);
        let wake_root = root.clone();
        cx.spawn(async move |cx| {
            while reopen.next().await.is_some() {
                cx.update(|cx| reopen_window(&wake_root, cx));
            }
        })
        .detach();
        let icon = ui::app_icon::AppIcon::load(&icon_file);
        desktop::set_dock_icon(icon.dock_png());
        cx.set_global(ui::app_icon::Chosen {
            icon,
            file: icon_file.clone(),
        });
        if let Err(error) = register_symbols() {
            eprintln!("bundled symbol font: {error}");
            cx.quit();
            return;
        }
        // Ghostty's default face ships inside Ghostty; without a bundled copy, machines
        // lacking an installed JetBrains Mono fall back to Menlo and look different.
        if let Err(error) = cx
            .text_system()
            .add_fonts(ui::theme::BUNDLED_MONO.map(Into::into).to_vec())
        {
            eprintln!("bundled terminal font: {error}");
        }
        let workspace = match initial {
            Ok(w) => w,
            Err(e) => {
                show_startup_error(e.to_string(), cx);
                return;
            }
        };
        let socket = root.join("workbench.sock");
        let mut desktop_events = desktop::start(&socket, workspace.clone(), cx);
        let desktop_root = root.clone();
        cx.spawn(async move |cx| {
            while let Some(route) = desktop_events.next().await {
                cx.update(|cx| {
                    if let Some(message) = route
                        .strip_prefix("error:")
                        .or_else(|| route.strip_prefix("notice:"))
                    {
                        cx.set_global(desktop::Status(message.into()));
                        return;
                    }
                    reopen_window(&desktop_root, cx);
                    if !route.is_empty() {
                        for handle in cx.windows() {
                            if let Some(handle) = handle.downcast::<workbench::Workbench>() {
                                let _ = handle.update(cx, |view, window, cx| {
                                    view.navigate_to(&route, window, cx)
                                });
                                break;
                            }
                        }
                    }
                });
            }
        })
        .detach();
        let bounds = Bounds::centered(
            None,
            size(
                px(theme::Theme::WINDOW_WIDTH),
                px(theme::Theme::WINDOW_HEIGHT),
            ),
            cx,
        );
        let result = cx.open_window(window_options(bounds), |window, cx| {
            cx.new(|cx| workbench::Workbench::new(socket, workspace, window, cx))
        });
        #[cfg(feature = "visual-test")]
        harness::start(result.as_ref().ok().copied(), evidence.clone(), cx);
        if let Err(e) = result {
            eprintln!("open workbench: {e}");
            cx.quit();
        }
        cx.activate(true);
    });
    Ok(())
}

/// Register only in this process. GPUI's memory font loader skips symbol-only
/// faces without an ASCII `m`; CoreText must see this face for its cascade.
fn register_symbols() -> Result<(), &'static str> {
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
        fn CGFontRelease(font: *const c_void);
    }
    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTFontManagerRegisterGraphicsFont(
            font: *const c_void,
            error: *mut *const c_void,
        ) -> bool;
    }
    let bytes = include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf");
    // Static bytes outlive the provider. CoreText retains its registered font.
    unsafe {
        let provider = CGDataProviderCreateWithData(
            std::ptr::null_mut(),
            bytes.as_ptr().cast(),
            bytes.len(),
            None,
        );
        if provider.is_null() {
            return Err("cannot create symbol font provider");
        }
        let font = CGFontCreateWithDataProvider(provider);
        CGDataProviderRelease(provider);
        if font.is_null() {
            return Err("cannot decode bundled symbol font");
        }
        let registered = CTFontManagerRegisterGraphicsFont(font, std::ptr::null_mut());
        CGFontRelease(font);
        if !registered {
            return Err("cannot register bundled symbol font");
        }
    }
    Ok(())
}
