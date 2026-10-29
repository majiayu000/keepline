//! Native visual evidence: scripted layouts, pages and clicks that save what the real
//! renderer drew. Built only with the `visual-test` feature.
use super::*;
use crate::terminal::screen;
use gpui::WindowHandle;
use std::time::Duration;

/// Runs the evidence script the environment asks for against the opened workbench.
pub fn start(
    handle: Option<WindowHandle<workbench::Workbench>>,
    evidence: Option<PathBuf>,
    cx: &mut App,
) {
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_some()
    {
        let handle = *handle;
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
                handle.update(cx,|v,_,a|{v.visual_check_panes(9);v.visual_check_closed_processes(1);v.visual_close_focused(a);}).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle.update(cx,|v,_,_|{v.visual_check_panes(8);v.visual_check_closed_processes(2);}).unwrap();
                eprintln!("native actions PASS: right/down launch 8->9->10, shell cwd and focus, close 10->9->8 stops the closed PTYs");
            }
            cx.update(|cx|cx.quit());
        }).detach();
    }
    // KEEPLINE_OSC7_EVIDENCE names a directory reached through a symlink: only the
    // shell's OSC 7 report keeps that spelling, the process directory resolves it.
    if let (Some(handle), Some(_), Some(dir)) = (
        &handle,
        evidence.clone(),
        std::env::var("KEEPLINE_OSC7_EVIDENCE").ok(),
    ) {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(2000).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(4000).await;
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            pane.update(cx, |pane, cx| pane.visual_type(&format!("cd '{dir}'\r"), cx));
            wait(1500).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(2000).await;
            let cwd = handle.update(cx, |v, _, _| v.visual_shown_cwd()).unwrap();
            eprintln!("osc7: new tab cwd={cwd}, typed cd {dir}");
            assert_eq!(cwd, dir, "a new tab opens where the shell says it is");
            eprintln!("osc7 native PASS: new tab took the shell's OSC 7 directory, symlink spelling kept");
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    if let (Some(handle), Some(_)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_TAB_CWD_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(2000).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(1500).await;
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            pane.update(cx, |pane, cx| pane.visual_type("cd /tmp\r", cx));
            wait(800).await;
            let (before, _) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(1500).await;
            let (after, _) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            let cwd = handle.update(cx, |v, _, _| v.visual_shown_cwd()).unwrap();
            eprintln!("tab cwd: tabs {before}->{after}, new tab cwd={cwd}");
            assert_eq!(after, before + 1, "a new tab opened");
            assert_eq!(
                std::fs::canonicalize(&cwd).unwrap(),
                std::fs::canonicalize("/tmp").unwrap(),
                "a new tab follows the focused shell's live directory"
            );
            eprintln!("tab cwd native PASS: new tab opens in the directory the focused shell cd'd into");
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // Two runs over one config directory: 1 rebinds 新建标签 in the settings panel,
    // 2 is the restart that must still have it, then restores the defaults.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_TAB_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(2000).await;
            for _ in 0..2 {
                handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
                wait(1500).await;
            }
            std::fs::create_dir_all(&directory).unwrap();
            let shot = |name: &str, window: &mut gpui::Window, app: &mut App| {
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join(format!("{name}.png")))
                    .unwrap();
            };
            let state = |cx: &mut gpui::AsyncApp| {
                handle
                    .update(cx, |v, w, a| v.visual_tab_state(w, a))
                    .unwrap()
            };
            // The first tab sits right after the sidebar, in the strip under the top bar.
            let tab = gpui::point(
                px(theme::Theme::SIDEBAR + 30.),
                px(theme::Theme::TOP + theme::Theme::TABS / 2.),
            );
            let hover = |at: gpui::Point<gpui::Pixels>| {
                gpui::MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input()
            };
            // Hovering shows the tab's card; the menu must not sit under it.
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(hover(tab), a);
            })
            .unwrap();
            wait(900).await;
            cx.update_window(handle.into(), |_, w, a| shot("0-tab-hover", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, tab, gpui::MouseButton::Right)
            })
            .unwrap();
            let (menu, renaming) = state(cx);
            assert!(menu && renaming.is_none(), "right click opens the tab menu");
            // Still over the tab with the menu open: the card stays away.
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(hover(tab + gpui::point(px(4.), px(0.))), a);
            })
            .unwrap();
            wait(900).await;
            cx.update_window(handle.into(), |_, w, a| shot("1-tab-menu", w, a))
                .unwrap();
            let at = handle
                .update(cx, |v, _, _| v.visual_tab_menu_point(0))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, at, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(300).await;
            let (menu, renaming) = state(cx);
            eprintln!("menu rename: menu={menu} field={renaming:?}");
            assert!(
                !menu && renaming.as_ref().is_some_and(|(_, focused)| *focused),
                "重命名 opens a focused field"
            );
            cx.update_window(handle.into(), |_, w, a| shot("2-rename-selected", w, a))
                .unwrap();
            let key = |key: &str| {
                gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input()
            };
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("backspace"), a);
            })
            .unwrap();
            wait(300).await;
            eprintln!("after backspace: {:?}", state(cx).1);
            cx.update_window(handle.into(), |_, w, a| shot("3-rename-caret", w, a))
                .unwrap();
            // A name longer than the tab stays on one line and slides to keep the caret.
            let long = "PR 1234 修复标签重命名时长名字换行被裁掉的问题";
            handle
                .update(cx, |v, _, cx| v.visual_rename_type(long, cx))
                .unwrap();
            wait(300).await;
            cx.update_window(handle.into(), |_, w, a| shot("3b-rename-long", w, a))
                .unwrap();
            for k in ["cmd-left", "alt-right", "shift-alt-right"] {
                cx.update_window(handle.into(), |_, w, a| {
                    w.dispatch_event(key(k), a);
                })
                .unwrap();
            }
            wait(300).await;
            cx.update_window(handle.into(), |_, w, a| shot("3c-rename-moved", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("escape"), a);
            })
            .unwrap();
            wait(300).await;
            assert!(state(cx).1.is_none(), "Esc cancels the rename");
            // A real double click: two presses, the second carrying click_count 2.
            cx.update_window(handle.into(), |_, w, a| {
                for count in [1, 2] {
                    w.dispatch_event(
                        gpui::MouseDownEvent {
                            button: gpui::MouseButton::Left,
                            position: tab,
                            modifiers: Default::default(),
                            click_count: count,
                            first_mouse: false,
                        }
                        .to_platform_input(),
                        a,
                    );
                    w.dispatch_event(
                        gpui::MouseUpEvent {
                            button: gpui::MouseButton::Left,
                            position: tab,
                            modifiers: Default::default(),
                            click_count: count,
                        }
                        .to_platform_input(),
                        a,
                    );
                }
            })
            .unwrap();
            wait(300).await;
            let (_, renaming) = state(cx);
            eprintln!("double click: field={renaming:?}");
            assert!(
                renaming.as_ref().is_some_and(|(_, focused)| *focused),
                "double click opens a focused field"
            );
            cx.update_window(handle.into(), |_, w, a| shot("4-double-click", w, a))
                .unwrap();
            // Clicking the terminal keeps the typed name. Focus events only reach an
            // active window, as the user's is when they click in it.
            cx.update_window(handle.into(), |_, w, _| w.activate_window())
                .unwrap();
            wait(500).await;
            handle
                .update(cx, |v, _, cx| v.visual_rename_type("部署 PR", cx))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                let middle = w.viewport_size();
                native_click(
                    w,
                    a,
                    gpui::point(middle.width / 2., middle.height / 2.),
                    gpui::MouseButton::Left,
                );
                // Focus listeners run once a frame is drawn, as the app does at once.
                w.refresh();
                w.draw(a).clear(a);
            })
            .unwrap();
            wait(300).await;
            let name = handle
                .update(cx, |v, _, cx| v.visual_first_tab_name(cx))
                .unwrap();
            let active = cx
                .update_window(handle.into(), |_, w, _| w.is_window_active())
                .unwrap();
            eprintln!(
                "after click away: active={active} field={:?} name={name}",
                state(cx).1
            );
            // macOS will not hand focus to a window started from a background shell.
            if active {
                assert!(state(cx).1.is_none(), "a click elsewhere ends the rename");
                assert_eq!(name, "部署 PR", "and keeps the typed name");
                cx.update_window(handle.into(), |_, w, a| shot("4b-blur-saved", w, a))
                    .unwrap();
            } else {
                eprintln!("click away not checked: the window is not active");
                cx.update_window(handle.into(), |_, w, a| {
                    w.dispatch_event(key("escape"), a);
                })
                .unwrap();
                wait(300).await;
            }
            // A 出错了 conversation whose resume fails: the second click returns to the
            // resume tab instead of opening another.
            let (before, _) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            handle
                .update(cx, |v, w, a| v.visual_open_failed_session(w, a))
                .unwrap();
            wait(3000).await;
            let (first, resumed) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            cx.update_window(handle.into(), |_, w, a| shot("5-failed-resume", w, a))
                .unwrap();
            for _ in 0..3 {
                handle
                    .update(cx, |v, w, a| v.visual_open_failed_session(w, a))
                    .unwrap();
                wait(1500).await;
            }
            let (again, current) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            eprintln!(
                "failed session tabs: before={before} first={first} after 3 more clicks={again}"
            );
            assert_eq!(first, before + 1, "the first click opens one resume tab");
            assert_eq!(
                (again, &current),
                (first, &resumed),
                "later clicks return to it"
            );
            cx.update_window(handle.into(), |_, w, a| shot("6-failed-reclick", w, a))
                .unwrap();
            // Closing the shown tab hands the view to a neighbour with a live shell; that
            // shell must be drawable at once, before the host confirms the close.
            let neighbour = handle
                .update(cx, |v, _, cx| v.visual_close_tab(cx))
                .unwrap();
            assert!(neighbour.is_some(), "a neighbouring tab takes over");
            cx.update_window(handle.into(), |_, w, a| shot("7-closed-tab", w, a))
                .unwrap();
            // A full-screen Agent such as Codex asks for mouse reports; right-click must
            // still open Keepline's pane menu instead of going to the program.
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            pane.update(cx, |p, cx| {
                p.visual_type("printf '\\033[?1000;1002;1006h'\r", cx)
            });
            wait(1500).await;
            let (tracking, center) = pane.read_with(cx, |p, _| p.visual_mouse_app());
            assert!(tracking, "the shell turned on mouse reports");
            let center = center.expect("painted pane");
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, center, gpui::MouseButton::Right)
            })
            .unwrap();
            let opened = handle
                .update(cx, |v, _, _| v.visual_pane_menu_open())
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                shot("8-right-click-mouse-app", w, a)
            })
            .unwrap();
            eprintln!("right click over a mouse-reporting program: menu={opened}");
            assert!(
                opened,
                "right-click opens the pane menu even when the program takes the mouse"
            );
            // A tab of several splits asks before its × takes them all down; a tab of one
            // pane closes at once.
            handle.update(cx, |v, w, a| v.visual_split_down(w, a)).unwrap();
            wait(1000).await;
            let (before, shown) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            handle.update(cx, |v, _, cx| v.visual_close_tab(cx)).unwrap();
            wait(500).await;
            cx.update_window(handle.into(), |_, w, a| shot("9-close-splits", w, a))
                .unwrap();
            let (asked, after) = handle
                .update(cx, |v, _, _| (v.visual_confirming_close(), v.visual_tabs()))
                .unwrap();
            assert!(asked, "closing a tab of two splits asks first");
            assert_eq!(after, (before, shown), "and keeps the tab until confirmed");
            handle.update(cx, |v, _, cx| v.visual_cancel_close(cx)).unwrap();
            handle.update(cx, |v, _, cx| v.visual_close_focused(cx)).unwrap();
            wait(1000).await;
            handle.update(cx, |v, _, cx| v.visual_close_tab(cx)).unwrap();
            wait(500).await;
            let (asked, (count, _)) = handle
                .update(cx, |v, _, _| (v.visual_confirming_close(), v.visual_tabs()))
                .unwrap();
            assert!(!asked, "a tab of one shell closes without asking");
            assert_eq!(count, before - 1, "and is gone");
            cx.update_window(handle.into(), |_, w, a| shot("10-closed-single", w, a))
                .unwrap();
            eprintln!("tab evidence PASS");
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    // Runs a real Codex in the shown pane, then scrolls and moves the mouse over it the
    // way a user does; reports whether any report reached Codex as typed text.
    if let Some(handle) = &handle
        && let Ok(command) = std::env::var("KEEPLINE_WHEEL_EVIDENCE")
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            cx.background_executor().timer(Duration::from_secs(3)).await;
            pane.update(cx, |p, cx| p.visual_type(&format!("{command}\r"), cx));
            cx.background_executor()
                .timer(Duration::from_secs(15))
                .await;
            let (tracking, center) = pane.read_with(cx, |p, _| p.visual_mouse_app());
            let center = center.expect("pane laid out");
            eprintln!("wheel evidence: tracking={tracking}");
            let rounds: usize = std::env::var("KEEPLINE_WHEEL_ROUNDS")
                .ok()
                .and_then(|r| r.parse().ok())
                .unwrap_or(40);
            let gap: u64 = std::env::var("KEEPLINE_WHEEL_GAP_MS")
                .ok()
                .and_then(|r| r.parse().ok())
                .unwrap_or(8);
            for i in 0..rounds {
                cx.update_window(handle.into(), |_, window, app| {
                    use gpui::InputEvent;
                    let at =
                        center + gpui::point(px((i % 20) as f32 * 7.), px((i % 7) as f32 * 9.));
                    window.dispatch_event(
                        gpui::MouseMoveEvent {
                            position: at,
                            pressed_button: None,
                            modifiers: Default::default(),
                        }
                        .to_platform_input(),
                        app,
                    );
                    window.dispatch_event(
                        gpui::ScrollWheelEvent {
                            position: at,
                            delta: gpui::ScrollDelta::Lines(gpui::point(
                                0.,
                                if i % 2 == 0 { -3. } else { 3. },
                            )),
                            modifiers: Default::default(),
                            touch_phase: gpui::TouchPhase::Moved,
                        }
                        .to_platform_input(),
                        app,
                    );
                })
                .unwrap();
                if gap > 0 {
                    cx.background_executor()
                        .timer(Duration::from_millis(gap))
                        .await;
                }
            }
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let text = pane.read_with(cx, |p, _| p.visual_text());
            eprintln!(
                "wheel evidence: pane error = {:?}",
                pane.read_with(cx, |p, _| p.visual_error())
            );
            let prompt = text
                .lines()
                .rfind(|l| l.trim_start().starts_with('›'))
                .unwrap_or("")
                .to_owned();
            eprintln!("wheel evidence: prompt line = {prompt:?}");
            eprintln!(
                "wheel evidence: stray report text = {}",
                ["<35", "<64", "<65", "[<"].iter().any(|m| text.contains(m))
            );
            // Leave Codex without submitting anything.
            pane.update(cx, |p, cx| p.visual_type("\x03\x03", cx));
            cx.background_executor().timer(Duration::from_secs(2)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    // Runs the top bar's 「重启后台」 for real, as confirming its dialog does.
    if let Some(handle) = &handle
        && std::env::var_os("KEEPLINE_RESTART_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            handle
                .update(cx, |view, _, cx| view.visual_restart_host(cx))
                .unwrap();
        })
        .detach();
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            handle
                .update(cx, |_, window, _| {
                    if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {
                        window.toggle_simple_fullscreen();
                    }
                })
                .unwrap();
            cx.background_executor().timer(Duration::from_secs(1)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pages = std::env::var("KEEPLINE_PAGE_LIST").unwrap_or_else(|_| {
                "Main,Overview,TodoNew,TodoDetail,TodoStart,Cleanup,Menubar".into()
            });
            for name in pages.split(',').map(str::to_owned).collect::<Vec<_>>() {
                let name = name.as_str();
                handle
                    .update(cx, |view, window, cx| {
                        window.resize(size(
                            px(theme::Theme::WINDOW_WIDTH),
                            px(theme::Theme::WINDOW_HEIGHT),
                        ));
                        window.bounds_changed(cx);
                        view.visual_page(name, window, cx);
                    })
                    .unwrap();
                if name == "HereTyped" {
                    use gpui::InputEvent;
                    // A plain keystroke into the terminal folds the card over it.
                    cx.update_window(handle.into(), |_, w, a| {
                        w.dispatch_event(
                            gpui::KeyDownEvent {
                                keystroke: gpui::Keystroke::parse("x").unwrap(),
                                is_held: false,
                                prefer_character_input: false,
                            }
                            .to_platform_input(),
                            a,
                        );
                    })
                    .unwrap();
                    let open = handle.update(cx, |v, _, _| v.visual_here_open()).unwrap();
                    assert!(!open, "typing into the pane folds its 你在这里 card");
                    eprintln!("HereTyped: card folded after a keystroke");
                }
                if name == "HereClickOut" {
                    use gpui::InputEvent;
                    // A press anywhere outside the card puts it away.
                    cx.update_window(handle.into(), |_, w, a| {
                        w.refresh();
                        w.draw(a).clear(a);
                        w.dispatch_event(
                            gpui::MouseDownEvent {
                                button: gpui::MouseButton::Left,
                                position: gpui::point(px(8.), px(theme::Theme::WINDOW_HEIGHT - 8.)),
                                modifiers: Default::default(),
                                click_count: 1,
                                first_mouse: false,
                            }
                            .to_platform_input(),
                            a,
                        );
                    })
                    .unwrap();
                    let open = handle.update(cx, |v, _, _| v.visual_here_open()).unwrap();
                    assert!(!open, "a click outside folds its 你在这里 card");
                    eprintln!("HereClickOut: card folded after an outside click");
                }
                // Starting an Agent and placing the task text takes a few quiet checks.
                let settle = if name == "TodoStart" { 4 } else { 1 };
                cx.background_executor()
                    .timer(Duration::from_secs(settle))
                    .await;
                cx.update_window(handle.into(), |_, window, app| {
                    assert_eq!(
                        window.viewport_size(),
                        size(
                            px(theme::Theme::WINDOW_WIDTH),
                            px(theme::Theme::WINDOW_HEIGHT)
                        )
                    );
                    window.refresh();
                    window.draw(app).clear(app);
                    let path = directory.join(format!("{}-native.png", name.to_lowercase()));
                    if name == "Menubar" {
                        desktop::snapshot(&path);
                    } else {
                        window.render_to_image().unwrap().save(path).unwrap();
                    }
                    eprintln!(
                        "{name}: native viewport {:?}, scale {}",
                        window.viewport_size(),
                        window.scale_factor()
                    );
                })
                .unwrap();
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_none()
        && std::env::var_os("KEEPLINE_TAB_EVIDENCE").is_none()
        && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_none()
        && std::env::var_os("KEEPLINE_OVERVIEW_EVIDENCE").is_none()
    {
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
    // The overview sits in the same window as the panes: one toggle in the top bar goes
    // in and back out, Esc and the tab strip return, the sidebar's spaces filter it.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_OVERVIEW_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(3000).await;
            std::fs::create_dir_all(&directory).unwrap();
            let shot = |name: &str, window: &mut gpui::Window, app: &mut App| {
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join(format!("{name}.png")))
                    .unwrap();
            };
            // Focus moves when a frame is drawn; idle frames are paused in this harness.
            let state = |cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, w, a| {
                    w.refresh();
                    w.draw(a).clear(a);
                })
                .unwrap();
                handle
                    .update(cx, |v, w, a| v.visual_overview_state(w, a))
                    .unwrap()
            };
            let click = |cx: &mut gpui::AsyncApp, at: gpui::Point<gpui::Pixels>| {
                cx.update_window(handle.into(), |_, w, a| {
                    native_click(w, a, at, gpui::MouseButton::Left)
                })
                .unwrap();
            };
            let escape = |cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, w, a| {
                    use gpui::InputEvent;
                    w.dispatch_event(
                        gpui::KeyDownEvent {
                            keystroke: gpui::Keystroke::parse("escape").unwrap(),
                            is_held: false,
                            prefer_character_input: false,
                        }
                        .to_platform_input(),
                        a,
                    );
                    w.refresh();
                    w.draw(a).clear(a);
                })
                .unwrap();
            };
            if state(cx).0 {
                escape(cx);
                wait(500).await;
            }
            // A live shell to come back to: the copied workspace's own terminals are gone.
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(2000).await;
            let start = state(cx);
            eprintln!("work view: {start:?}");
            assert!(!start.0, "starts on the panes");
            assert_eq!(start.3, "pane", "the new shell has the keyboard");
            cx.update_window(handle.into(), |_, w, a| shot("1-work", w, a))
                .unwrap();
            let toggle = handle
                .update(cx, |v, _, _| v.visual_toggle_point())
                .unwrap();
            let open = |cx: &mut gpui::AsyncApp| {
                click(cx, toggle);
                let s = state(cx);
                assert!(s.0, "the toggle opens the overview");
                assert_eq!(s.3, "overview", "the overview takes the keyboard");
                // Drawn in place: the toggle has not moved, so it can close it again.
                let again = handle
                    .update(cx, |v, _, _| v.visual_toggle_point())
                    .unwrap();
                assert_eq!(again, toggle, "the toggle does not move");
                s
            };
            open(cx);
            wait(1500).await;
            cx.update_window(handle.into(), |_, w, a| shot("2-overview", w, a))
                .unwrap();
            // The same spot closes it.
            click(cx, toggle);
            wait(800).await;
            let back = handle
                .update(cx, |v, _, _| v.visual_toggle_point())
                .unwrap();
            assert_eq!(back, toggle, "the toggle does not move on the way back");
            let s = state(cx);
            assert!(!s.0, "the same toggle returns");
            assert_eq!(s.1, start.1, "back on the tab that was shown");
            cx.update_window(handle.into(), |_, w, a| shot("3-back-by-toggle", w, a))
                .unwrap();
            assert_eq!(state(cx).3, "pane", "the pane has the keyboard again");
            // Esc returns.
            open(cx);
            escape(cx);
            wait(800).await;
            let s = state(cx);
            assert!(!s.0, "Esc returns");
            assert_eq!(s.3, "pane", "Esc hands the keyboard back to the pane");
            // A tab in the strip returns to it.
            open(cx);
            click(
                cx,
                gpui::point(
                    px(theme::Theme::SIDEBAR + 30.),
                    px(theme::Theme::TOP + theme::Theme::TABS / 2.),
                ),
            );
            wait(800).await;
            let s = state(cx);
            assert!(!s.0, "a tab returns");
            assert!(s.1.is_some(), "a tab is shown");
            // A space in the sidebar filters the overview and keeps it open.
            open(cx);
            let spaces = handle.update(cx, |v, _, _| v.visual_spaces()).unwrap();
            let mut filtered = None;
            for y in (0..12).map(|i| theme::Theme::TOP + 30. + i as f32 * 4.) {
                click(cx, gpui::point(px(60.), px(y)));
                let s = state(cx);
                if s.2.is_some() {
                    filtered = Some((y, s));
                    break;
                }
            }
            let (y, s) = filtered.expect("a sidebar space sets the filter");
            eprintln!("sidebar space at y={y}: {s:?}");
            assert!(s.0, "the overview stays open");
            assert_eq!(s.2.as_ref(), spaces.first(), "the first space filters it");
            wait(800).await;
            cx.update_window(handle.into(), |_, w, a| shot("4-space-filter", w, a))
                .unwrap();
            escape(cx);
            wait(500).await;
            assert!(!state(cx).0);
            eprintln!("overview evidence PASS");
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

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
