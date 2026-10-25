mod connection;
mod desktop;
mod field;
mod keyboard;
mod lifecycle;
mod pane;
mod screen;
mod terminal_input;
mod theme;
mod workbench;

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
        cx.bind_keys([gpui::KeyBinding::new("cmd-q",Quit,None)]);
        cx.set_menus([gpui::Menu::new("Keepline").items([gpui::MenuItem::action("菜单栏面板",ShowMenuPanel),gpui::MenuItem::action("退出 Keepline",Quit)])]);
        let wake_root=root.clone();
        cx.spawn(async move |cx| {
            while reopen.next().await.is_some() {
                cx.update(|cx|reopen_window(&wake_root,cx));
            }
        }).detach();
        if let Err(error)=register_symbols() {
            eprintln!("bundled symbol font: {error}"); cx.quit(); return;
        }
        let workspace=match initial {Ok(w)=>w,Err(e)=>{show_startup_error(e.to_string(),cx);return;}};
        let socket=root.join("workbench.sock");
        let mut desktop_events=desktop::start(&socket,workspace.clone(),cx);
        let desktop_root=root.clone();
        cx.spawn(async move |cx| {
            while let Some(route)=desktop_events.next().await {
                cx.update(|cx| {
                    if let Some(message)=route.strip_prefix("error:").or_else(||route.strip_prefix("notice:")) {cx.set_global(desktop::Status(message.into()));return;}
                    reopen_window(&desktop_root,cx);
                    if !route.is_empty() {
                        for handle in cx.windows() {
                            if let Some(handle)=handle.downcast::<workbench::Workbench>() {
                                let _=handle.update(cx,|view,window,cx|view.navigate_to(&route,window,cx));
                                break;
                            }
                        }
                    }
                });
            }
        }).detach();
        let bounds = Bounds::centered(
            None,
            size(
                px(theme::Theme::WINDOW_WIDTH),
                px(theme::Theme::WINDOW_HEIGHT),
            ),
            cx,
        );
        let result = cx.open_window(
            window_options(bounds),
            |window, cx| cx.new(|cx| workbench::Workbench::new(socket, workspace, window, cx)),
        );
        #[cfg(feature = "visual-test")]
        if let (Ok(handle), Some(directory)) = (&result, evidence.clone())
            && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_some() {
            let handle=*handle;
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                for name in ["single","nested","menu","eight"] {
                    handle.update(cx,|view,window,cx|{
                        if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {window.toggle_simple_fullscreen();}
                        window.resize(size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));window.bounds_changed(cx);
                        view.visual_layout(name,window,cx);
                    }).unwrap();
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                    let root=if name=="menu"{Some(handle.update(cx,|v,_,_|v.visual_bounds()).unwrap())}else{None};
                    cx.update_window(handle.into(),|_,window,app|{
                        window.refresh();window.draw(app).clear(app);
                        if let Some(bounds)=root {native_click(window,app,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right);}
                        assert_eq!(window.viewport_size(),size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));
                        screen::verify_native_geometry(window,app);
                        std::fs::create_dir_all(&directory).unwrap();
                        window.render_to_image().unwrap().save(directory.join(format!("{name}-native.png"))).unwrap();
                    }).unwrap();
                    if let Some(bounds)=root {
                        // Native event dispatch tests; simulated mouse, not human/CUA acceptance.
                        for expected in [1,4] {
                            let at=handle.update(cx,|v,_,_|v.visual_menu_point(2)).unwrap();
                            cx.update_window(handle.into(),|_,window,app|native_click(window,app,at,gpui::MouseButton::Left)).unwrap();
                            handle.update(cx,|v,_,_|v.visual_check_panes(expected)).unwrap();
                            if expected==1 {cx.update_window(handle.into(),|_,window,app|native_click(window,app,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right)).unwrap();}
                        }
                        cx.update_window(handle.into(),|_,window,app|{
                            use gpui::InputEvent;
                            let from=gpui::point(bounds.left()+(bounds.size.width-px(theme::Theme::S6))*0.58+px(theme::Theme::S6/2.),bounds.top()+px(30.));
                            let to=gpui::point(bounds.left()+bounds.size.width*0.67,from.y);
                            window.dispatch_event(gpui::MouseDownEvent{button:gpui::MouseButton::Left,position:from,modifiers:Default::default(),click_count:1,first_mouse:false}.to_platform_input(),app);
                            window.dispatch_event(gpui::MouseMoveEvent{position:to,pressed_button:Some(gpui::MouseButton::Left),modifiers:Default::default()}.to_platform_input(),app);
                            window.dispatch_event(gpui::MouseUpEvent{button:gpui::MouseButton::Left,position:to,modifiers:Default::default(),click_count:1}.to_platform_input(),app);
                            window.refresh();window.draw(app).clear(app);
                        }).unwrap();
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        handle.update(cx,|v,_,_|v.visual_check_saved_ratio()).unwrap();
                        eprintln!("native events PASS: right menu, zoom 4->1->4, divider drag ratio persisted");
                    }
                }
                if std::env::var_os("KL_NATIVE_ONLY").is_some() {
                    let bounds=handle.update(cx,|v,_,_|v.visual_bounds()).unwrap();
                    cx.update_window(handle.into(),|_,w,a|native_click(w,a,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right)).unwrap();
                    let at=handle.update(cx,|v,_,_|v.visual_menu_point(0)).unwrap();
                    cx.update_window(handle.into(),|_,w,a|native_click(w,a,at,gpui::MouseButton::Left)).unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle.update(cx,|v,w,a|{v.visual_assert_new_shell(9,w,a);v.visual_split_down(w,a);}).unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle.update(cx,|v,w,a|{v.visual_assert_new_shell(10,w,a);v.visual_close_focused(a);}).unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle.update(cx,|v,_,a|{v.visual_check_panes(9);v.visual_check_closed_processes();v.visual_close_focused(a);}).unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle.update(cx,|v,_,_|{v.visual_check_panes(8);v.visual_check_closed_processes();}).unwrap();
                    eprintln!("native actions PASS: right/down launch 8->9->10, shell cwd and focus, close 10->9->8 keeps PTYs alive");
                }
                cx.update(|cx|cx.quit());
            }).detach();
        }
        #[cfg(feature = "visual-test")]
        if let (Ok(handle), Some(directory)) = (&result, evidence.clone())
            && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_some()
        {
            let handle = *handle;
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                handle.update(cx, |_,window,_| {
                    if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {window.toggle_simple_fullscreen();}
                }).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                std::fs::create_dir_all(&directory).unwrap();
                let pages = std::env::var("KEEPLINE_PAGE_LIST").unwrap_or_else(|_| "Main,Overview,TodoNew,TodoDetail,TodoStart,Cleanup,Menubar".into());
                for name in pages.split(',').map(str::to_owned).collect::<Vec<_>>() {
                    let name = name.as_str();
                    handle.update(cx, |view,window,cx| {
                        window.resize(size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));
                        window.bounds_changed(cx);view.visual_page(name,window,cx);
                    }).unwrap();
                    // Starting an Agent and placing the task text takes a few quiet checks.
                    let settle = if name == "TodoStart" { 4 } else { 1 };
                    cx.background_executor().timer(Duration::from_secs(settle)).await;
                    cx.update_window(handle.into(), |_,window,app| {
                        assert_eq!(window.viewport_size(), size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));
                        window.refresh(); window.draw(app).clear(app);
                        let path = directory.join(format!("{}-native.png",name.to_lowercase()));
                        if name == "Menubar" {desktop::snapshot(&path);} else {window.render_to_image().unwrap().save(path).unwrap();}
                        eprintln!("{name}: native viewport {:?}, scale {}",window.viewport_size(),window.scale_factor());
                    }).unwrap();
                }
                cx.update(|cx|cx.quit());
            }).detach();
        }
        #[cfg(feature = "visual-test")]
        if let (Ok(handle), Some(directory)) = (&result, evidence.clone())
            && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_none()
            && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_none() {
            let handle = *handle;
            cx.spawn(async move |cx| {
                // Test-only single shots: native renderer output, no desktop capture permission.
                cx.background_executor().timer(Duration::from_secs(2)).await;
                handle
                    .update(cx, |_, window, _| {
                        if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {
                            window.toggle_simple_fullscreen();
                        }
                    })
                    .unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                for overview in [false, true] {
                    handle
                        .update(cx, |view, window, cx| {
                            window.resize(size(
                                px(theme::Theme::WINDOW_WIDTH),
                                px(theme::Theme::WINDOW_HEIGHT),
                            ));
                            window.bounds_changed(cx);
                            view.visual_view(overview, window, cx);
                        })
                        .unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle
                        .update(cx, |_, window, app| {
                            assert_eq!(
                                window.viewport_size(),
                                size(
                                    px(theme::Theme::WINDOW_WIDTH),
                                    px(theme::Theme::WINDOW_HEIGHT)
                                ),
                                "visual comparison viewport"
                            );
                            eprintln!(
                                "{}: viewport={:?}, scale={}, mono={}",
                                if overview { "Overview" } else { "Main" },
                                window.viewport_size(),
                                window.scale_factor(),
                                theme::Theme::mono(app).family
                            );
                            if !overview {
                                screen::verify_native_geometry(window, app);
                            }
                            std::fs::create_dir_all(&directory).unwrap();
                            window
                                .render_to_image()
                                .unwrap()
                                .save(directory.join(if overview {
                                    "overview-native.png"
                                } else {
                                    "main-native.png"
                                }))
                                .unwrap();
                        })
                        .unwrap();
                    if !overview {
                        let marked = handle
                            .update(cx, |view, window, cx| {
                                view.visual_marked_overlay(window, cx)
                            })
                            .unwrap();
                        // Render explicitly: an idle/occluded display link may not
                        // have presented the notification yet. Do not hold the root
                        // entity borrowed while draw renders it again.
                        cx.update_window(handle.into(), |_, window, app| {
                                window.refresh();
                                window.draw(app).clear(app);
                                window
                                    .render_to_image()
                                    .unwrap()
                                    .save(directory.join("ime-native.png"))
                                    .unwrap();
                                if let Some(marked) = marked {
                                    let scale = window.scale_factor();
                                    eprintln!("IME rendered underlines: {:?}", window.painted_underlines());
                                    assert!(
                                        window.painted_underlines().iter().any(|line| {
                                            let x = line.bounds.origin.x.0 / scale;
                                            let y = line.bounds.origin.y.0 / scale;
                                            (x - f32::from(marked.left())).abs() < 1.
                                                && y >= f32::from(marked.top())
                                                && y <= f32::from(marked.bottom())
                                        }),
                                        "marked underline must be painted at the IME range, not the pane bottom"
                                    );
                                }
                            })
                            .unwrap();
                    }
                }
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
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

#[cfg(feature = "visual-test")]
fn native_click(
    window: &mut gpui::Window,
    app: &mut App,
    position: gpui::Point<gpui::Pixels>,
    button: gpui::MouseButton,
) {
    use gpui::InputEvent;
    window.dispatch_event(
        gpui::MouseDownEvent {
            button,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        app,
    );
    window.dispatch_event(
        gpui::MouseUpEvent {
            button,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        app,
    );
    window.refresh();
    window.draw(app).clear(app);
}
