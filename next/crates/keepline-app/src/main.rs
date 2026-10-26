mod connection;
mod desktop;
#[cfg(feature = "visual-test")]
mod harness;
mod host;
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
use std::os::unix::{fs::DirBuilderExt, process::CommandExt};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn ensure_host(root: &Path) -> io::Result<()> {
    if lifecycle::connected_host(&root.join("workbench.sock"))? {
        return Ok(());
    }
    if !root.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root)?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("host.log"))?;
    let daemon = std::env::current_exe()?.with_file_name("keeplined");
    let mut command = Command::new(daemon);
    command
        .args(["serve", "--runtime"])
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    // SAFETY: setsid is async-signal-safe and detaches the host from this UI process.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if rpc(&root.join("workbench.sock"), &Request::Workspace).is_ok() {
            std::thread::spawn(move || {
                if let Err(e) = child.wait() {
                    eprintln!("host wait: {e}");
                }
            });
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "host exited {status}; see host.log"
            )));
        }
        if Instant::now() >= until {
            return Err(io::Error::other("后台启动超时，请检查 host.log"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

gpui::actions!(keepline, [Quit, ShowMenuPanel]);

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
            .bg(T::color(T::APP))
            .text_color(T::color(T::PRIMARY))
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
// before GPUI or any application thread is created.
fn main() {
    // SAFETY: this is the first operation in main, before spawning threads.
    unsafe { std::env::set_var("GPUI_PAUSE_IDLE_DISPLAY_LINK", "1") };
    if let Err(error) = run() {
        eprintln!("Keepline: {error}");
        std::process::exit(1);
    }
}
fn run() -> io::Result<()> {
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
    let app = gpui_platform::application();
    let reopen_root = root.clone();
    app.on_reopen(move |cx| reopen_window(&reopen_root, cx));
    app.run(move |cx: &mut App| {
        use futures::StreamExt;
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &ShowMenuPanel, _| desktop::show());
        cx.bind_keys([gpui::KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus([gpui::Menu::new("Keepline").items([
            gpui::MenuItem::action("菜单栏面板", ShowMenuPanel),
            gpui::MenuItem::action("退出 Keepline", Quit),
        ])]);
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
