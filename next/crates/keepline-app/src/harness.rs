//! Native visual evidence: scripted layouts, pages and clicks that save what the real
//! renderer drew. Built only with the `visual-test` feature.
use super::*;
use crate::terminal::screen;
use gpui::WindowHandle;
use std::time::Duration;
#[path = "settings_evidence.rs"]
mod settings_evidence;

/// Runs the evidence script the environment asks for against the opened workbench.
pub fn start(
    handle: Option<WindowHandle<workbench::Workbench>>,
    evidence: Option<PathBuf>,
    cx: &mut App,
) {
    if settings_evidence::start(handle, evidence.clone(), cx) {
        return;
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_DIAGNOSTIC_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            pane.update(cx, |pane, cx| pane.visual_type("/bin/sh -c 'while :; do :; done'\r", cx));
            cx.background_executor().timer(Duration::from_secs(1)).await;
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh(); window.draw(app).clear(app);
                window.render_to_image().unwrap().save(directory.join("entry.png")).unwrap();
                let at = gpui::point(px(theme::Theme::SIDEBAR / 2.), window.viewport_size().height - px(theme::Theme::S8 + theme::Theme::ROW / 2.));
                native_click(window, app, at, gpui::MouseButton::Left);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(500)).await;
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh(); window.draw(app).clear(app);
                window.render_to_image().unwrap().save(directory.join("loading.png")).unwrap();
                window.dispatch_event(gpui::KeyDownEvent { keystroke: gpui::Keystroke::parse("escape").unwrap(), is_held: false, prefer_character_input: false }.to_platform_input(), app);
            }).unwrap();
            assert!(handle.update(cx, |view, _, _| view.visual_diagnostic_closed()).unwrap());
            cx.background_executor().timer(Duration::from_millis(300)).await;
            handle.update(cx, |view, window, cx| view.visual_open_diagnostics(window, cx)).unwrap();
            cx.background_executor().timer(Duration::from_secs(18)).await;
            let report = handle.update(cx, |view, _, cx| {
                let report = view.visual_diagnostic_report(cx).expect("missing native diagnostic report");
                assert!(report.complete && report.process_count > 0, "{report:?}");
                assert!(report.apps.iter().any(|app| app.cpu_percent > 0.));
                assert!(report.apps.iter().any(|app| !app.terminals.is_empty() && app.cpu_percent > 5.));
                view.visual_copy_diagnostics(cx);
                report
            }).unwrap();
            std::fs::write(directory.join("report.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh(); window.draw(app).clear(app);
                window.render_to_image().unwrap().save(directory.join("complete.png")).unwrap();
            }).unwrap();
            handle.update(cx, |view, _, cx| view.visual_open_diagnostic_terminal(cx)).unwrap();
            assert!(handle.update(cx, |view, _, _| view.visual_diagnostic_closed()).unwrap());
            pane.update(cx, |pane, cx| pane.visual_type("\x03", cx));
            eprintln!("diagnostic native PASS: loading, Escape cancellation, restart, real complete report, clipboard readback, managed terminal navigation");
            cx.update(|cx| cx.quit());
        }).detach();
        return;
    }
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
            pane.update(cx, |pane, cx| {
                pane.visual_type(&format!("cd '{dir}'\r"), cx)
            });
            wait(1500).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(2000).await;
            let cwd = handle.update(cx, |v, _, _| v.visual_shown_cwd()).unwrap();
            eprintln!("osc7: new tab cwd={cwd}, typed cd {dir}");
            assert_eq!(cwd, dir, "a new tab opens where the shell says it is");
            eprintln!(
                "osc7 native PASS: new tab took the shell's OSC 7 directory, symlink spelling kept"
            );
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
            eprintln!(
                "tab cwd native PASS: new tab opens in the directory the focused shell cd'd into"
            );
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // Two runs over one config directory: 1 rebinds 新建标签 in the settings panel,
    // 2 is the restart that must still have it, then restores the defaults.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && let Ok(phase) = std::env::var("KEEPLINE_KEYMAP_EVIDENCE")
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            std::fs::create_dir_all(&directory).unwrap();
            let file = crate::keymap::path().unwrap();
            assert!(
                std::env::var_os("XDG_CONFIG_HOME").is_some(),
                "runs against a scratch config, never ~/.config"
            );
            let ready = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, w, a| v.visual_pane_focused(w, a)).unwrap();
            wait(2000).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            // A launch under load can outlast any fixed wait: poll for the shell's focus.
            for _ in 0..100 {
                if ready(cx) {
                    break;
                }
                wait(100).await;
            }
            let press = |keys: &str, cx: &mut gpui::AsyncApp| {
                let keystroke = gpui::Keystroke::parse(keys).unwrap();
                cx.update_window(handle.into(), |_, w, a| {
                    w.dispatch_event(gpui::KeyDownEvent { keystroke, is_held: false, prefer_character_input: false }.to_platform_input(), a);
                })
                .unwrap();
            };
            let in_pane = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, w, a| v.visual_pane_focused(w, a)).unwrap();
            let tabs = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, _, _| v.visual_tab_row().0.len()).unwrap();
            let settings = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, w, a| v.visual_settings(w, a)).unwrap();
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                let path = directory.join(format!("{name}.png"));
                cx.update_window(handle.into(), |_, w, a| {
                    w.refresh();
                    w.draw(a).clear(a);
                    w.render_to_image().unwrap().save(path).unwrap();
                })
                .unwrap();
            };
            // A ⌘ chord typed into the terminal opens a tab, or provably does not.
            let opens_tab = async |keys: &str, cx: &mut gpui::AsyncApp| {
                assert!(in_pane(cx), "{keys} is pressed while typing in a terminal");
                let before = tabs(cx);
                press(keys, cx);
                // Opened means the new tab's shell holds input; a chord that opens
                // nothing is given the same 6s to prove it.
                for _ in 0..60 {
                    wait(100).await;
                    if tabs(cx) == before + 1 && in_pane(cx) {
                        return true;
                    }
                }
                false
            };
            let open_settings = async |cx: &mut gpui::AsyncApp| {
                press("cmd-,", cx);
                wait(400).await;
                let (recording, taken, focused) = settings(cx).expect("⌘, opens 设置");
                assert!(!recording && !taken && focused, "the panel takes the keys");
            };
            let close_settings = async |cx: &mut gpui::AsyncApp| {
                press("escape", cx);
                wait(500).await;
                assert!(settings(cx).is_none(), "Esc closes 设置");
                assert!(in_pane(cx), "input goes back to the terminal");
            };
            if phase == "1" {
                assert!(!file.exists(), "a fresh config starts on the defaults");
                assert!(opens_tab("cmd-t", cx).await, "default ⌘T opens a tab");
                open_settings(cx).await;
                cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Dark, a).unwrap());
                wait(300).await;
                shot("settings-dark", cx);
                cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Light, a).unwrap());
                wait(300).await;
                shot("settings-light", cx);
                // A taken chord waits for 仍然使用 instead of moving silently.
                handle.update(cx, |v, _, cx| v.visual_settings_record("split_down", cx)).unwrap();
                press("cmd-d", cx);
                wait(300).await;
                assert_eq!(settings(cx), Some((false, true, true)), "⌘D is 向右分屏's");
                shot("settings-conflict-light", cx);
                handle.update(cx, |v, _, cx| v.visual_settings_press("取消", cx)).unwrap();
                // A chord without ⌘ belongs to the program and is refused.
                handle.update(cx, |v, _, cx| v.visual_settings_record("new_tab", cx)).unwrap();
                press("ctrl-t", cx);
                wait(300).await;
                assert!(!file.exists(), "a refused chord changes nothing");
                handle.update(cx, |v, _, cx| v.visual_settings_record("new_tab", cx)).unwrap();
                wait(200).await;
                shot("settings-recording-light", cx);
                // A modifier alone does not end the recording; the full chord does.
                press("cmd-shift", cx);
                assert_eq!(settings(cx).map(|s| s.0), Some(true));
                press("cmd-shift-t", cx);
                wait(300).await;
                let saved = std::fs::read_to_string(&file).expect("keys.json written");
                eprintln!("keys.json after rebind: {}", saved.replace('\n', " "));
                assert!(saved.contains("new_tab") && saved.contains("cmd-shift-t"));
                cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Dark, a).unwrap());
                wait(300).await;
                shot("settings-rebound-dark", cx);
                close_settings(cx).await;
                assert!(!opens_tab("cmd-t", cx).await, "the old ⌘T no longer opens a tab");
                assert!(opens_tab("cmd-shift-t", cx).await, "the new ⌘⇧T does");
                eprintln!("keymap native PASS phase 1: default ⌘T, conflict held, ctrl refused, rebound to ⌘⇧T in 设置, old key off");
            } else {
                assert!(file.exists(), "phase 1 left keys.json");
                assert!(opens_tab("cmd-shift-t", cx).await, "after a restart ⌘⇧T still opens a tab");
                assert!(!opens_tab("cmd-t", cx).await, "and ⌘T still does not");
                open_settings(cx).await;
                handle.update(cx, |v, _, cx| v.visual_settings_press("全部恢复默认", cx)).unwrap();
                wait(300).await;
                assert!(!file.exists(), "全部恢复默认 removes keys.json");
                close_settings(cx).await;
                assert!(opens_tab("cmd-t", cx).await, "⌘T is back");
                assert!(!opens_tab("cmd-shift-t", cx).await, "⌘⇧T is free again");
                eprintln!("keymap native PASS phase 2: ⌘⇧T kept across restart, 全部恢复默认 restores ⌘T");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    if let (Some(handle), Some(_)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SHORTCUT_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            // A launch under load can outlast any fixed wait: poll for the state instead.
            macro_rules! until {
                ($cond:expr, $why:expr) => {{
                    let mut met = false;
                    for _ in 0..100 {
                        if $cond {
                            met = true;
                            break;
                        }
                        wait(100).await;
                    }
                    assert!(met, "{} within 10s", $why);
                }};
            }
            let focused = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, w, a| v.visual_pane_focused(w, a)).unwrap();
            wait(2000).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            until!(focused(cx), "the first tab's shell takes focus");
            let press = |keys: &str, cx: &mut gpui::AsyncApp| {
                let keystroke = gpui::Keystroke::parse(keys).unwrap();
                let focused = handle.update(cx, |v, w, a| v.visual_pane_focused(w, a)).unwrap();
                assert!(focused, "{keys} is pressed while typing in a terminal");
                cx.update_window(handle.into(), |_, w, a| {
                    w.dispatch_event(gpui::KeyDownEvent { keystroke, is_held: false, prefer_character_input: false }.to_platform_input(), a);
                })
                .unwrap();
            };
            let row = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, _, _| v.visual_tab_row()).unwrap();
            let (start, _, _) = row(cx);
            press("cmd-t", cx);
            until!(row(cx).0.len() == start.len() + 1 && focused(cx), "⌘T's tab takes focus");
            let (tabs, shown, panes) = row(cx);
            assert_eq!(tabs.len(), start.len() + 1, "⌘T opens a tab");
            assert_eq!(shown.as_ref(), tabs.last(), "the new tab shows");
            assert_eq!(panes, 1);
            press("cmd-d", cx);
            until!(row(cx).2 == 2 && focused(cx), "⌘D's pane takes focus");
            assert_eq!(row(cx).2, 2, "⌘D splits right");
            press("cmd-shift-d", cx);
            until!(row(cx).2 == 3 && focused(cx), "⌘⇧D's pane takes focus");
            assert_eq!(row(cx).2, 3, "⌘⇧D splits down");
            press("cmd-1", cx);
            wait(300).await;
            let (tabs, shown, _) = row(cx);
            assert_eq!(shown.as_ref(), tabs.first(), "⌘1 shows the first tab");
            press("cmd-shift-[", cx);
            wait(300).await;
            assert_eq!(row(cx).1.as_ref(), tabs.last(), "⌘⇧[ wraps to the last tab");
            press("cmd-shift-]", cx);
            wait(300).await;
            assert_eq!(row(cx).1.as_ref(), tabs.first(), "⌘⇧] wraps to the first tab");
            press("cmd-9", cx);
            wait(300).await;
            assert_eq!(row(cx).1.as_ref(), tabs.last(), "⌘9 shows the last tab");
            // ⌘] / ⌘[ walk the three panes in layout order, wrapping, with real focus.
            let focus = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, w, a| v.visual_pane_focus(w, a)).unwrap();
            let (order, first) = focus(cx);
            assert_eq!(order.len(), 3);
            let at = order.iter().position(|id| Some(id) == first.as_ref()).expect("a pane has focus");
            for step in 1..=3 {
                press("cmd-]", cx);
                wait(300).await;
                assert_eq!(focus(cx).1.as_ref(), Some(&order[(at + step) % 3]), "⌘] step {step}");
            }
            press("cmd-[", cx);
            wait(300).await;
            assert_eq!(focus(cx).1.as_ref(), Some(&order[(at + 2) % 3]), "⌘[ goes back, wrapping");
            press("cmd-[", cx);
            wait(300).await;
            assert_eq!(focus(cx).1.as_ref(), Some(&order[(at + 1) % 3]), "⌘[ again");
            // ⌘⌥ arrows go by where panes sit: order is a | (b over c).
            let (a, b, c) = (&order[0], &order[1], &order[2]);
            for (keys, want, why) in [
                ("cmd-alt-left", a, "⌘⌥← reaches the left column"),
                ("cmd-alt-left", a, "⌘⌥← at the left edge stays"),
                ("cmd-alt-right", b, "⌘⌥→ takes the first of two equal neighbours"),
                ("cmd-alt-down", c, "⌘⌥↓ goes below"),
                ("cmd-alt-up", b, "⌘⌥↑ goes back up"),
                ("cmd-alt-down", c, "⌘⌥↓ again"),
                ("cmd-alt-left", a, "⌘⌥← from the lower right pane"),
            ] {
                press(keys, cx);
                wait(300).await;
                assert_eq!(focus(cx).1.as_ref(), Some(want), "{why}");
            }
            eprintln!("shortcut native PASS: tabs {}->{}, ⌘T ⌘D ⌘⇧D ⌘1 ⌘⇧[ ⌘⇧] ⌘9, ⌘] ⌘[ cycle 3 panes with focus, ⌘⌥ arrows by position", start.len(), tabs.len());
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
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
            handle
                .update(cx, |v, w, a| v.visual_split_down(w, a))
                .unwrap();
            wait(1000).await;
            let (before, shown) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            handle
                .update(cx, |v, _, cx| v.visual_close_tab(cx))
                .unwrap();
            wait(500).await;
            cx.update_window(handle.into(), |_, w, a| shot("9-close-splits", w, a))
                .unwrap();
            let (asked, after) = handle
                .update(cx, |v, _, _| (v.visual_confirming_close(), v.visual_tabs()))
                .unwrap();
            assert!(asked, "closing a tab of two splits asks first");
            assert_eq!(after, (before, shown), "and keeps the tab until confirmed");
            handle
                .update(cx, |v, _, cx| v.visual_cancel_close(cx))
                .unwrap();
            handle
                .update(cx, |v, _, cx| v.visual_close_focused(cx))
                .unwrap();
            wait(1000).await;
            handle
                .update(cx, |v, _, cx| v.visual_close_tab(cx))
                .unwrap();
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
    // Scrolls a full shell back with the wheel, then types: the view returns to the
    // input line for a real key event and for committed text alike.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_FOLLOW_INPUT_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            pane.update(cx, |p, cx| p.visual_type("clear; seq 1 400\r", cx));
            for _ in 0..30 {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                if pane.update(cx, |p, _| p.visual_text()).contains("400") { break; }
            }
            let center = pane.read_with(cx, |p, _| p.visual_mouse_app().1).expect("pane laid out");
            let scroll_up = |window: &mut gpui::Window, app: &mut App| {
                window.dispatch_event(gpui::ScrollWheelEvent {
                    position: center,
                    delta: gpui::ScrollDelta::Lines(gpui::point(0., 60.)),
                    modifiers: Default::default(),
                    touch_phase: gpui::TouchPhase::Moved,
                }.to_platform_input(), app);
                window.refresh(); window.draw(app).clear(app);
            };
            for (name, key) in [("key", Some("left")), ("text", None)] {
                cx.update_window(handle.into(), |_, window, app| {
                    scroll_up(window, app);
                    window.render_to_image().unwrap().save(directory.join(format!("{name}-scrolled.png"))).unwrap();
                }).unwrap();
                assert!(!pane.read_with(cx, |p, _| p.visual_at_bottom()), "{name}: wheel did not leave the bottom");
                match key {
                    Some(key) => cx.update_window(handle.into(), |_, window, app| {
                        window.dispatch_event(gpui::KeyDownEvent { keystroke: gpui::Keystroke::parse(key).unwrap(), is_held: false, prefer_character_input: false }.to_platform_input(), app);
                    }).unwrap(),
                    None => pane.update(cx, |p, cx| p.visual_type("x", cx)),
                }
                cx.background_executor().timer(Duration::from_millis(300)).await;
                assert!(pane.read_with(cx, |p, _| p.visual_at_bottom()), "{name}: input did not return to the bottom");
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh(); window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(directory.join(format!("{name}-typed.png"))).unwrap();
                }).unwrap();
            }
            pane.update(cx, |p, cx| p.visual_type("\x15", cx));
            eprintln!("follow input native PASS: wheel leaves the bottom; a key event and typed text both return to the input line");
            cx.update(|cx| cx.quit());
        }).detach();
        return;
    }
    // Plain-text URLs in Agent prose: hovering one underlines it, as an OSC 8 link does.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_LINK_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            pane.update(cx, |pane, cx| {
                pane.visual_type(
                    "clear; printf '7. Hunter / AI 雷达\\n- 加了 Rust 标签页，已经上线到 3002 端口：http://100.113.31.23:3002/radar?s=rust\\n- 见 (https://en.wikipedia.org/wiki/Rust_(language))。\\n'\r",
                    cx,
                )
            });
            for _ in 0..30 {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                if pane.update(cx, |p, _| p.visual_text()).contains("3002 端口：http") {
                    break;
                }
            }
            for (name, prefix, cols) in [
                ("hover-radar.png", "http://100.113.31.23", 38),
                ("hover-wiki.png", "https://en.wikipedia.org", 45),
            ] {
                let (at, found) = pane
                    .update(cx, |pane, _| pane.visual_link(prefix))
                    .unwrap_or_else(|| panic!("no link for {prefix} in\n{}", pane.update(cx, |p, _| p.visual_text())));
                assert_eq!(found, cols, "{prefix}");
                cx.update_window(handle.into(), |_, window, app| {
                    window.dispatch_event(
                        gpui::MouseMoveEvent { position: at, pressed_button: None, modifiers: Default::default() }
                            .to_platform_input(),
                        app,
                    );
                    window.refresh(); window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(directory.join(name)).unwrap();
                }).unwrap();
            }
            pane.update(cx, |pane, cx| {
                pane.visual_type(
                    "clear; printf '8. 其他\\n- 把 3 个 Codex 的 skill 软链接到了 ~/.claude/skills。\\n- 改了 crates/keepline-app/Cargo.toml:3 和 full_v18.mp4\\n'\r",
                    cx,
                )
            });
            for _ in 0..30 {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                if pane.update(cx, |p, _| p.visual_text()).contains("和 full_v18") {
                    break;
                }
            }
            let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
            for (name, word, expect) in [
                ("hover-home.png", "~/.claude/skills", Some(home.join(".claude/skills"))),
                (
                    "hover-relative.png",
                    "crates/keepline-app/Cargo.toml:3",
                    Some(std::env::current_dir().unwrap().join("crates/keepline-app/Cargo.toml")),
                ),
                ("hover-missing.png", "full_v18.mp4", None),
            ] {
                let (at, _) = pane
                    .update(cx, |pane, _| pane.visual_path(word))
                    .unwrap_or_else(|| panic!("no path word {word}"));
                cx.update_window(handle.into(), |_, window, app| {
                    window.dispatch_event(
                        gpui::MouseMoveEvent { position: at, pressed_button: None, modifiers: Default::default() }
                            .to_platform_input(),
                        app,
                    );
                }).unwrap();
                // A relative word waits for the host to say where the shell is.
                cx.background_executor().timer(Duration::from_millis(500)).await;
                let (_, hovered) = pane.update(cx, |pane, _| pane.visual_path(word)).unwrap();
                assert_eq!(hovered, expect, "{word}");
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh(); window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(directory.join(name)).unwrap();
                }).unwrap();
            }
            eprintln!("link native PASS: plain-text URLs and existing paths underlined on hover, a missing file is not");
            cx.update(|cx| cx.quit());
        }).detach();
        return;
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
                let settle = match name {
                    "TodoStart" => 4,
                    // A login shell with a rich prompt takes longer before the command goes in.
                    "ActionRun" => 6,
                    _ => 1,
                };
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
