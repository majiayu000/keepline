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
    // Reconnect: a host killed outright shows the strip after its grace period, and
    // 「重新连接」 starts a host the status link then resubscribes to.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_RECONNECT_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh(); window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(directory.join(name)).unwrap();
                }).unwrap();
            };
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            assert!(!handle.update(cx, |view, _, _| view.visual_cut_off()).unwrap());
            shot("connected.png", cx);
            let socket = handle.update(cx, |view, _, _| view.visual_socket()).unwrap();
            let runtime = socket.parent().unwrap().display().to_string();
            let killed = std::process::Command::new("/usr/bin/pkill")
                .args(["-9", "-f", &format!("keeplined serve --runtime {runtime}")])
                .status().unwrap();
            assert!(killed.success(), "no host to kill");
            cx.background_executor().timer(Duration::from_secs(1)).await;
            assert!(!handle.update(cx, |view, _, _| view.visual_cut_off()).unwrap(), "strip before the grace period");
            cx.background_executor().timer(Duration::from_secs(3)).await;
            assert!(handle.update(cx, |view, _, _| view.visual_cut_off()).unwrap(), "no strip after the host died");
            shot("cut-off.png", cx);
            handle.update(cx, |view, _, cx| view.visual_reconnect(cx)).unwrap();
            cx.background_executor().timer(Duration::from_millis(200)).await;
            shot("reconnecting.png", cx);
            let mut back = false;
            for _ in 0..40 {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                if !handle.update(cx, |view, _, _| view.visual_cut_off()).unwrap() { back = true; break; }
            }
            assert!(back, "strip still shown after 重新连接");
            cx.background_executor().timer(Duration::from_millis(500)).await;
            shot("reconnected.png", cx);
            eprintln!("reconnect native PASS: no strip within grace, strip after host kill, 重新连接 restarts host, strip clears");
            cx.update(|cx| cx.quit());
        }).detach();
        return;
    }
    // Scrollback: the thumb shows when scrolled up, a drag held past the top edge scrolls
    // and copies across screens, search highlights and steps, the thumb drags the view.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SCROLLBACK_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            pane.update(cx, |p, cx| p.visual_type("clear; seq 1 300\r", cx));
            for _ in 0..50 {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                if pane.update(cx, |p, _| p.visual_text()).lines().any(|l| l.trim() == "300") { break; }
            }
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                let path = directory.join(name);
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh(); window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(path).unwrap();
                }).unwrap();
            };
            let b = pane.read_with(cx, |p, _| p.visual_bounds()).expect("laid out");
            let mv = |window: &mut gpui::Window, app: &mut App, at: gpui::Point<gpui::Pixels>, pressed: bool| {
                window.dispatch_event(gpui::MouseMoveEvent { position: at, pressed_button: pressed.then_some(gpui::MouseButton::Left), modifiers: Default::default() }.to_platform_input(), app);
            };
            // Up the history: the thumb appears.
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::ScrollWheelEvent { position: b.center(), delta: gpui::ScrollDelta::Lines(gpui::point(0., 40.)), modifiers: Default::default(), touch_phase: gpui::TouchPhase::Moved }.to_platform_input(), app);
            }).unwrap();
            shot("1-scrolled.png", cx);
            let (top, total, len) = pane.read_with(cx, |p, _| p.visual_viewport());
            eprintln!("scrolled viewport {top}/{total}/{len}");
            assert!(top + len < total, "scrolled into history");
            // Press in the middle, hold the drag past the top edge a while, release.
            let start = gpui::point(b.left() + px(60.), b.center().y);
            let outside = gpui::point(start.x, b.top() - px(30.));
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::MouseDownEvent { button: gpui::MouseButton::Left, position: start, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), app);
                mv(window, app, start - gpui::point(px(0.), px(40.)), true);
                mv(window, app, outside, true);
            }).unwrap();
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::MouseUpEvent { button: gpui::MouseButton::Left, position: outside, modifiers: Default::default(), click_count: 1 }.to_platform_input(), app);
            }).unwrap();
            shot("2-dragged-past-top.png", cx);
            let after = pane.read_with(cx, |p, _| p.visual_viewport()).0;
            assert!(after < top, "holding the drag past the top scrolled up: {after} vs {top}");
            pane.update(cx, |p, cx| p.copy(cx));
            let copied = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text())).unwrap_or_default();
            let lines: Vec<&str> = copied.lines().collect();
            eprintln!("copied {} lines: {:?} .. {:?}", lines.len(), lines.first(), lines.last());
            assert!(lines.len() > len, "copy reaches past one screen: {} lines, screen {len}", lines.len());
            // Consecutive numbers: nothing skipped or doubled across the screens it crossed.
            let nums: Vec<u32> = lines.iter().filter_map(|l| l.trim().parse().ok()).collect();
            assert!(nums.windows(2).all(|w| w[1] == w[0] + 1), "copied text runs on: {nums:?}");
            // Search: newest match first, then step back.
            handle.update(cx, |_, window, cx| pane.update(cx, |p, cx| p.visual_search("25", window, cx))).unwrap();
            cx.background_executor().timer(Duration::from_millis(500)).await;
            shot("3-search.png", cx);
            let (current, rows) = pane.read_with(cx, |p, _| p.visual_matches());
            eprintln!("search: current {current:?} of {} matches", rows.len());
            assert!(rows.len() >= 10 && current == Some(rows.len() - 1));
            pane.update(cx, |p, cx| p.visual_step_match(-1, cx));
            cx.background_executor().timer(Duration::from_millis(200)).await;
            let (current2, _) = pane.read_with(cx, |p, _| p.visual_matches());
            assert_eq!(current2, Some(rows.len() - 2));
            shot("4-previous-match.png", cx);
            let (vtop, _, vlen) = pane.read_with(cx, |p, _| p.visual_viewport());
            assert!((vtop..vtop + vlen).contains(&rows[rows.len() - 2]), "the current match is in view");
            // Step forward wraps past the newest back to the oldest.
            pane.update(cx, |p, cx| { p.visual_step_match(1, cx); p.visual_step_match(1, cx); });
            assert_eq!(pane.read_with(cx, |p, _| p.visual_matches()).0, Some(0), "wraps to the first");
            // Thumb drag to the very top.
            let (_, total, _) = pane.read_with(cx, |p, _| p.visual_viewport());
            let grab = gpui::point(b.right() - px(4.), b.bottom() - px(10.));
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::MouseDownEvent { button: gpui::MouseButton::Left, position: grab, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), app);
                mv(window, app, gpui::point(grab.x, b.top() + px(1.)), true);
                window.dispatch_event(gpui::MouseUpEvent { button: gpui::MouseButton::Left, position: gpui::point(grab.x, b.top() + px(1.)), modifiers: Default::default(), click_count: 1 }.to_platform_input(), app);
            }).unwrap();
            shot("5-thumb-top.png", cx);
            let top_now = pane.read_with(cx, |p, _| p.visual_viewport()).0;
            assert!(top_now < total / 10, "thumb dragged to the top: {top_now} of {total}");
            eprintln!("scrollback native PASS: thumb shows and drags, a drag past the edge scrolls and copies {} lines, search highlights and steps", lines.len());
            cx.update(|cx| cx.quit());
        }).detach();
        return;
    }
    // ⌘Home / ⌘End and the 「回到底部」 chip move through scrollback; a program's OSC 52
    // copy lands on the clipboard.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SCROLL_KEYS_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            cx.update(|cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into())));
            // "osc52 复制" in base64, written by the program as OSC 52.
            pane.update(cx, |p, cx| p.visual_type("clear; seq 1 300; printf '\\033]52;c;b3NjNTIg5aSN5Yi2\\a'\r", cx));
            for _ in 0..50 {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                if pane.update(cx, |p, _| p.visual_text()).lines().any(|l| l.trim() == "300") { break; }
            }
            cx.background_executor().timer(Duration::from_millis(500)).await;
            let shot = |name: &str, window: &mut gpui::Window, app: &mut App| {
                window.refresh(); window.draw(app).clear(app);
                window.render_to_image().unwrap().save(directory.join(format!("{name}.png"))).unwrap();
            };
            cx.update_window(handle.into(), |_, window, app| shot("0-osc52", window, app)).unwrap();
            let clip = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
            assert_eq!(clip.as_deref(), Some("osc52 复制"), "OSC 52 reached the clipboard");
            let key = |k: &str, window: &mut gpui::Window, app: &mut App| {
                window.dispatch_event(gpui::KeyDownEvent { keystroke: gpui::Keystroke::parse(k).unwrap(), is_held: false, prefer_character_input: false }.to_platform_input(), app);
            };
            cx.update_window(handle.into(), |_, window, app| { key("cmd-home", window, app); shot("1-cmd-home", window, app); }).unwrap();
            assert!(!pane.read_with(cx, |p, _| p.visual_at_bottom()), "⌘Home left the bottom");
            assert!(pane.read_with(cx, |p, _| p.visual_text_shown()).lines().next().is_some_and(|l| l.trim() == "1"), "⌘Home shows the top");
            cx.update_window(handle.into(), |_, window, app| key("cmd-end", window, app)).unwrap();
            assert!(pane.read_with(cx, |p, _| p.visual_at_bottom()), "⌘End returned to the bottom");
            cx.update_window(handle.into(), |_, window, app| key("cmd-pageup", window, app)).unwrap();
            assert!(!pane.read_with(cx, |p, _| p.visual_at_bottom()), "⌘PageUp left the bottom");
            // The chip floats at the pane's bottom right; clicking it goes back down.
            let chip = pane.read_with(cx, |p, _| p.visual_bounds()).map(|b| b.bottom_right() - gpui::point(px(50.), px(16.))).expect("laid out");
            cx.update_window(handle.into(), |_, window, app| {
                shot("2-chip", window, app);
                window.dispatch_event(gpui::MouseDownEvent { button: gpui::MouseButton::Left, position: chip, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), app);
                window.dispatch_event(gpui::MouseUpEvent { button: gpui::MouseButton::Left, position: chip, modifiers: Default::default(), click_count: 1 }.to_platform_input(), app);
                shot("3-chip-clicked", window, app);
            }).unwrap();
            assert!(pane.read_with(cx, |p, _| p.visual_at_bottom()), "the chip returned to the bottom");
            eprintln!("scroll keys native PASS: OSC 52 copies; ⌘Home/⌘End/⌘PageUp move the view; the chip returns to the bottom");
            cx.update(|cx| cx.quit());
        }).detach();
        return;
    }
    // Drags a selection over shell output, lets new output push it up, scrolls the view
    // back, then lets the text leave the screen for history: the highlight stays on the
    // same text throughout and copies it, not the rows it left.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SELECTION_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pane = handle.update(cx, |view, _, _| view.visual_shown_pane()).unwrap();
            pane.update(cx, |p, cx| p.visual_type("clear; seq 1 400; sleep 4; seq 401 405; sleep 3; seq 406 460\r", cx));
            // A fresh zsh can take seconds to start; the echoed command line is not output.
            for _ in 0..150 {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                if pane.update(cx, |p, _| p.visual_text()).lines().any(|l| l.trim() == "400") { break; }
            }
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let shot = |name: &str, window: &mut gpui::Window, app: &mut App| {
                window.refresh(); window.draw(app).clear(app);
                window.render_to_image().unwrap().save(directory.join(format!("{name}.png"))).unwrap();
            };
            let (from, to) = pane.read_with(cx, |p, _| (p.visual_cell(5, 0), p.visual_cell(7, 1)));
            let (from, to) = (from.expect("pane laid out"), to.expect("pane laid out"));
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::MouseDownEvent { button: gpui::MouseButton::Left, position: from, modifiers: Default::default(), click_count: 1, first_mouse: false }.to_platform_input(), app);
                window.dispatch_event(gpui::MouseMoveEvent { position: to, pressed_button: Some(gpui::MouseButton::Left), modifiers: Default::default() }.to_platform_input(), app);
                window.dispatch_event(gpui::MouseUpEvent { button: gpui::MouseButton::Left, position: to, modifiers: Default::default(), click_count: 1 }.to_platform_input(), app);
                shot("1-selected", window, app);
            }).unwrap();
            let (rows, text) = pane.read_with(cx, |p, _| p.visual_selection());
            let text = text.expect("drag selected nothing");
            assert_eq!(rows, [5, 6, 7], "selection drawn on the dragged rows");
            assert_eq!(text.lines().count(), 3, "{text:?}");
            // Copy on select put it on the clipboard already.
            let clip = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
            assert_eq!(clip.as_deref(), Some(text.as_str()), "copy on select");
            // New output pushes the selected text up, partly into history.
            for _ in 0..40 {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                if pane.update(cx, |p, _| p.visual_text()).lines().any(|l| l.trim() == "405") { break; }
            }
            cx.background_executor().timer(Duration::from_millis(300)).await;
            cx.update_window(handle.into(), |_, window, app| shot("2-output-pushed", window, app)).unwrap();
            // The highlight sits on the rows that show the selected text, wherever they are now.
            let on_text = |((rows, copied), drawn): ((Vec<u16>, Option<String>), Vec<String>), step: &str| {
                assert_eq!(copied.as_deref(), Some(text.as_str()), "{step}: copied text changed");
                let lines: Vec<&str> = text.lines().collect();
                assert!(!drawn.is_empty() && drawn.len() <= lines.len(), "{step}: {rows:?} {drawn:?}");
                // Rows scrolled out of view drop from the top; the end stays in view.
                let shown = &lines[lines.len() - drawn.len()..];
                for (i, (row, line)) in drawn.iter().zip(shown).enumerate() {
                    let ok = if i + 1 == drawn.len() { row.starts_with(line) } else { row == line };
                    assert!(ok, "{step}: highlight on {drawn:?}, selected {lines:?}");
                }
                rows
            };
            let pushed = on_text(pane.read_with(cx, |p, _| p.visual_selection_drawn()), "output");
            assert!(*pushed.last().unwrap() < 7, "output moved the text but not the selection: {pushed:?}");
            // The wheel scrolls the view back; the highlight comes down with its text.
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::ScrollWheelEvent { position: from, delta: gpui::ScrollDelta::Lines(gpui::point(0., 3.)), modifiers: Default::default(), touch_phase: gpui::TouchPhase::Moved }.to_platform_input(), app);
                shot("3-scrolled", window, app);
            }).unwrap();
            let scrolled = on_text(pane.read_with(cx, |p, _| p.visual_selection_drawn()), "wheel");
            assert!(scrolled.last() > pushed.last(), "the wheel moved the text but not the selection: {pushed:?} -> {scrolled:?}");
            pane.update(cx, |p, cx| p.copy(cx));
            let copied = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
            assert_eq!(copied.as_deref(), Some(text.as_str()), "copy after scrolling");
            // Back to the bottom; more output carries the text off screen into history.
            cx.update_window(handle.into(), |_, window, app| {
                window.dispatch_event(gpui::ScrollWheelEvent { position: from, delta: gpui::ScrollDelta::Lines(gpui::point(0., -200.)), modifiers: Default::default(), touch_phase: gpui::TouchPhase::Moved }.to_platform_input(), app);
            }).unwrap();
            for _ in 0..40 {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                if pane.update(cx, |p, _| p.visual_text()).lines().any(|l| l.trim() == "460") { break; }
            }
            cx.background_executor().timer(Duration::from_millis(300)).await;
            cx.update_window(handle.into(), |_, window, app| shot("4-in-history", window, app)).unwrap();
            let ((gone, kept), _) = pane.read_with(cx, |p, _| p.visual_selection_drawn());
            assert!(gone.is_empty(), "the selected text left the screen, its highlight too: {gone:?}");
            assert_eq!(kept.as_deref(), Some(text.as_str()), "selection kept in history");
            cx.update(|cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(String::new())));
            pane.update(cx, |p, cx| p.copy(cx));
            let copied = cx.update(|cx| cx.read_from_clipboard().and_then(|c| c.text()));
            assert_eq!(copied.as_deref(), Some(text.as_str()), "copy from history");
            eprintln!("selection native PASS: {text:?} stays highlighted on its text after output and wheel scrolling, and copies the same from history");
            cx.update(|cx| cx.quit());
        }).detach();
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
    // Pane decorations: a shell in a repo with one uncommitted file shows the example
    // plugin's 改动 1 in its title bar and the last commit in the hover card; after `cd`
    // out of the repo both go; back in, a second file shows as 改动 2 once it is due.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && let Ok(repo) = std::env::var("KEEPLINE_DECORATION_EVIDENCE")
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh();
                    window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(directory.join(name)).unwrap();
                })
                .unwrap();
            };
            let lines = |cx: &mut gpui::AsyncApp| {
                handle.update(cx, |v, _, cx| v.visual_decorations(cx)).unwrap()
            };
            // Asks until the lines settle on `want`: the first look starts the run.
            async fn until<F: std::future::Future>(
                want: impl Fn(&(Vec<String>, Vec<String>)) -> bool,
                lines: impl Fn(&mut gpui::AsyncApp) -> (Vec<String>, Vec<String>),
                wait: impl Fn(u64) -> F,
                secs: u64,
                cx: &mut gpui::AsyncApp,
            ) -> (Vec<String>, Vec<String>) {
                let mut seen = lines(cx);
                for _ in 0..secs * 4 {
                    if want(&seen) {
                        break;
                    }
                    wait(250).await;
                    seen = lines(cx);
                }
                seen
            }
            std::fs::create_dir_all(&directory).unwrap();
            wait(2000).await;
            handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
            wait(1500).await;
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            pane.update(cx, |pane, cx| pane.visual_type(&format!("cd '{repo}'\r"), cx));
            let seen = until(|l| !l.0.is_empty() && !l.1.is_empty(), lines, &wait, 5, cx).await;
            eprintln!("in the repo: {seen:?}");
            assert_eq!(seen.0, ["改动 1"]);
            assert!(seen.1[0].starts_with("最近提交 ") && seen.1[0].ends_with("：初始提交"));
            shot("decorated.png", cx);
            pane.update(cx, |pane, cx| pane.visual_type("cd /tmp\r", cx));
            let seen = until(|l| l.0.is_empty() && l.1.is_empty(), lines, &wait, 10, cx).await;
            let cwd = pane.update(cx, |p, _| p.cwd().map(|c| c.display().to_string()));
            eprintln!("in /tmp ({cwd:?}): {seen:?}");
            assert!(seen.0.is_empty() && seen.1.is_empty(), "outside a repo nothing shows");
            shot("outside.png", cx);
            std::fs::write(std::path::Path::new(&repo).join("second.txt"), "").unwrap();
            pane.update(cx, |pane, cx| pane.visual_type(&format!("cd '{repo}'\r"), cx));
            // The cached 改动 1 shows at once and stays until the run due after 10 s.
            let seen = until(|l| l.0 == ["改动 2"], lines, &wait, 15, cx).await;
            eprintln!("back, a file later: {seen:?}");
            assert_eq!(seen.0, ["改动 2"]);
            shot("refreshed.png", cx);
            eprintln!(
                "decoration native PASS: title and hover lines from the plugin, gone outside a repo, refreshed when due"
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
    // A space is renamed in place from its right-click menu, Esc keeps the old name,
    // and a space whose folder is gone can still be renamed.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SPACE_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(2000).await;
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
                    .update(cx, |v, w, a| v.visual_space_state(w, a))
                    .unwrap()
            };
            let key = |key: &str| {
                gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input()
            };
            handle
                .update(cx, |v, _, cx| v.visual_lose_space_dir(cx))
                .unwrap();
            wait(800).await;
            let (_, _, spaces) = state(cx);
            eprintln!("spaces: {spaces:?}");
            let before = spaces[0].0.clone();
            cx.update_window(handle.into(), |_, w, a| shot("0-space-missing", w, a))
                .unwrap();
            // The first space row sits under the preset switch and the 空间 heading.
            let row = gpui::point(
                px(theme::Theme::S20 + 20.),
                px(std::env::var("KL_SPACE_ROW_Y").ok().and_then(|v| v.parse().ok()).unwrap_or(132.)),
            );
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, row, gpui::MouseButton::Right)
            })
            .unwrap();
            let (menu, renaming, _) = state(cx);
            assert!(
                menu && renaming.is_none(),
                "right click opens the space menu"
            );
            cx.update_window(handle.into(), |_, w, a| shot("1-space-menu", w, a))
                .unwrap();
            let at = handle
                .update(cx, |v, _, _| v.visual_space_menu_point(0))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, at, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(300).await;
            let (menu, renaming, _) = state(cx);
            eprintln!("menu rename: menu={menu} field={renaming:?}");
            assert!(
                !menu && renaming == Some((before.clone(), true)),
                "重命名 opens a focused field holding the name"
            );
            cx.update_window(handle.into(), |_, w, a| shot("2-space-renaming", w, a))
                .unwrap();
            // Esc puts the old name back.
            handle
                .update(cx, |v, _, cx| v.visual_space_rename_type("不该留下", cx))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("escape"), a);
            })
            .unwrap();
            wait(800).await;
            let (_, renaming, spaces) = state(cx);
            assert!(renaming.is_none(), "Esc ends the rename");
            assert_eq!(spaces[0].0, before, "and keeps the old name");
            // Enter keeps the new one, with the folder still missing.
            handle
                .update(cx, |v, w, cx| v.visual_rename_first_space(w, cx))
                .unwrap();
            handle
                .update(cx, |v, _, cx| {
                    v.visual_space_rename_type("改名后的空间", cx)
                })
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| shot("3-space-typed", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("enter"), a);
            })
            .unwrap();
            wait(1500).await;
            let (_, renaming, spaces) = state(cx);
            eprintln!("after enter: {spaces:?}");
            assert!(renaming.is_none(), "Enter ends the rename");
            assert_eq!(spaces[0].0, "改名后的空间", "and saves the name");
            assert_eq!(
                spaces[0].1, "/nonexistent/keepline-space-evidence",
                "without touching the missing folder"
            );
            cx.update_window(handle.into(), |_, w, a| shot("4-space-renamed", w, a))
                .unwrap();
            eprintln!("space evidence PASS");
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // Tab order by dragging and by the tab menu, a pane moved to another tab, and a
    // space removed after confirming, all against a real host.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_LAYOUT_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |ms: u64| executor.timer(Duration::from_millis(ms));
            wait(2000).await;
            for _ in 0..2 {
                handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
                wait(1500).await;
            }
            handle
                .update(cx, |v, _, cx| v.visual_name_tabs(&["alpha", "beta", "gamma"], cx))
                .unwrap();
            wait(800).await;
            handle
                .update(cx, |v, _, cx| v.visual_add_space("other", "/tmp", cx))
                .unwrap();
            wait(800).await;
            std::fs::create_dir_all(&directory).unwrap();
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                let path = directory.join(name);
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh();
                    window.draw(app).clear(app);
                    window.render_to_image().unwrap().save(path).unwrap();
                })
                .unwrap();
            };
            let titles =
                |cx: &mut gpui::AsyncApp| handle.update(cx, |v, _, _| v.visual_tab_titles()).unwrap();
            shot("1-before.png", cx);
            assert_eq!(titles(cx), ["alpha", "beta", "gamma"]);
            let y = px(theme::Theme::TOP + theme::Theme::TABS / 2.);
            let x = |at: f32| px(theme::Theme::SIDEBAR + at);
            let env = |name: &str, default: f32| {
                std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
            };
            let (from, to) = (env("KL_DRAG_FROM", 30.), env("KL_DRAG_TO", 240.));
            cx.update_window(handle.into(), |_, window, app| {
                let at = gpui::point(x(from), y);
                window.dispatch_event(
                    gpui::MouseDownEvent {
                        button: gpui::MouseButton::Left,
                        position: at,
                        modifiers: Default::default(),
                        click_count: 1,
                        first_mouse: false,
                    }
                    .to_platform_input(),
                    app,
                );
                for i in 1..=12 {
                    let p = gpui::point(x(from + (to - from) * i as f32 / 12.), y);
                    window.dispatch_event(
                        gpui::MouseMoveEvent {
                            position: p,
                            pressed_button: Some(gpui::MouseButton::Left),
                            modifiers: Default::default(),
                        }
                        .to_platform_input(),
                        app,
                    );
                }
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join("2-dragging.png"))
                    .unwrap();
                window.dispatch_event(
                    gpui::MouseUpEvent {
                        button: gpui::MouseButton::Left,
                        position: gpui::point(x(to), y),
                        modifiers: Default::default(),
                        click_count: 1,
                    }
                    .to_platform_input(),
                    app,
                );
            })
            .unwrap();
            wait(600).await;
            shot("3-dragged.png", cx);
            eprintln!("after drag: {:?}", titles(cx));
            assert_eq!(
                titles(cx),
                ["beta", "alpha", "gamma"],
                "dragged onto gamma: the tab lands before the one dropped on"
            );
            // The tab menu: right-click the first tab (beta), 向右移.
            cx.update_window(handle.into(), |_, window, app| {
                native_click(window, app, gpui::point(x(from), y), gpui::MouseButton::Right);
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join("4-tab-menu.png"))
                    .unwrap();
            })
            .unwrap();
            let item = handle.update(cx, |v, _, _| v.visual_tab_menu_point(1)).unwrap();
            cx.update_window(handle.into(), |_, window, app| {
                native_click(window, app, item, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(600).await;
            assert_eq!(titles(cx), ["alpha", "beta", "gamma"], "向右移 swaps with the next tab");
            // Move gamma's pane into alpha: gamma closes with its last pane.
            handle
                .update(cx, |v, window, cx| v.visual_page_tab("gamma", window, cx))
                .unwrap();
            wait(600).await;
            handle
                .update(cx, |v, _, cx| v.visual_move_pane("alpha", cx))
                .unwrap();
            wait(800).await;
            shot("5-moved.png", cx);
            assert_eq!(titles(cx), ["alpha", "beta"]);
            assert_eq!(handle.update(cx, |v, _, _| v.visual_tab_layout("alpha")).unwrap(), 2);
            // Remove the other space from its right-click menu: the first press asks,
            // the second removes it.
            handle
                .update(cx, |v, _, cx| v.visual_open_space_menu("other", cx))
                .unwrap();
            wait(300).await;
            let remove = handle
                .update(cx, |v, _, _| v.visual_space_menu_point(2))
                .unwrap();
            // Clicks hit what was last drawn: paint the menu before pressing it.
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh();
                window.draw(app).clear(app);
                native_click(window, app, remove, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(300).await;
            shot("6-remove-confirm.png", cx);
            cx.update_window(handle.into(), |_, window, app| {
                native_click(window, app, remove, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(800).await;
            shot("7-removed.png", cx);
            let spaces = handle.update(cx, |v, _, _| v.visual_spaces()).unwrap();
            assert!(!spaces.iter().any(|s| s == "other"), "{spaces:?}");
            eprintln!(
                "layout native PASS: drag reorders tabs, 向右移 moves one step, a pane moves to another tab, a space is removed after confirming"
            );
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // Peek: a session waiting at an approval in a hidden tab floats over the shown one,
    // is answered from the peek's title bar, and its terminal keeps its own size.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_PEEK_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |ms| executor.timer(Duration::from_millis(ms));
            wait(1500).await;
            std::fs::create_dir_all(&directory).unwrap();
            let socket = handle.update(cx, |v, _, _| v.visual_socket()).unwrap();
            let root = socket.parent().unwrap().to_owned();
            let call = |r: Request| rpc(&socket, &r).unwrap();
            let workspace = || match call(Request::Workspace) { Reply::Workspace(w) => w, x => panic!("{x:?}") };
            let w = workspace();
            let space = match w.spaces.first() {
                Some(s) => s.id.clone(),
                None => {
                    call(Request::Edit { revision: w.revision, edit: Edit::AddSpace(Space { id: "s".into(), name: "Tests".into(), cwd: "/tmp".into() }) });
                    "s".into()
                }
            };
            // A Claude stand-in: its transcript names the session, then it asks.
            let logs = PathBuf::from(std::env::var_os("CLAUDE_CONFIG_DIR").expect("scratch CLAUDE_CONFIG_DIR")).join("projects/-tmp");
            std::fs::create_dir_all(&logs).unwrap();
            let claude = root.join("claude");
            let _ = std::os::unix::fs::symlink("/bin/sh", &claude);
            let id = format!("{:08x}-0000-4000-8000-000000000001", std::process::id());
            let seed = root.join("seed");
            let row = serde_json::json!({"type":"user","sessionId":id,"uuid":"u0","cwd":"/tmp","entrypoint":"cli","userType":"external","timestamp":keepline_core::content::now(),"message":{"content":"发布前检查"}});
            std::fs::write(&seed, format!("{row}\n")).unwrap();
            let script = format!(
                "cat '{}' > '{}'; printf '%s\\n' '⏺ 测试都过了，准备推送到远端。' '' '⏺ Bash(git push origin feat/release)' '' 'Bash command' '  git push origin feat/release' 'Do you want to proceed?' '❯ 1. Yes' \"  2. Yes, and don't ask again for git push\" '  3. No, and tell Claude what to do differently (esc)'; read -r answer; printf '\\033[2J\\033[Hanswer=[%s] size=%s\\n' \"$answer\" \"$(stty size)\"; read -r end; echo \"end=[$end]\"; sleep 600",
                seed.display(), logs.join(format!("{id}.jsonl")).display()
            );
            let launch = |tid: &str, argv: Vec<String>| {
                call(Request::Launch { id: tid.into(), space: space.clone(), argv, cwd: "/tmp".into(), cols: 100, rows: 30, scheme: Default::default() });
                let w = workspace();
                call(Request::Edit { revision: w.revision, edit: Edit::PutTab { tab: Tab { id: format!("tab-{tid}"), space: space.clone(), default_task: None, title: tid.into(), layout: Some(Layout::Leaf(tid.into())) } } });
            };
            launch("agent", vec!["/bin/sh".into(), "-c".into(), format!("exec '{}' -c \"$0\" fixture --session-id {id}", claude.display()), script]);
            launch("shell", vec!["/bin/sh".into(), "-c".into(), "printf '~/keepline $ cargo test\\n'; sleep 600".into()]);
            wait(500).await;
            handle.update(cx, |v, _, a| v.visual_peek_show_tab("shell", a)).unwrap();
            // The session binds and the screen rule sees the approval within a few seconds.
            let mut seen = None;
            for _ in 0..120 {
                wait(500).await;
                let opened = handle.update(cx, |v, w, a| v.visual_peek_row("agent", w, a)).unwrap();
                seen = handle.update(cx, |v, _, _| v.visual_peek_state()).unwrap();
                if opened && seen.as_ref().is_some_and(|(_, a, _)| a.is_some()) { break; }
            }
            let (terminal, approval, _) = seen.expect("peek never opened");
            assert_eq!(terminal, "agent");
            assert_eq!(approval.as_deref(), Some("Do you want to proceed?"), "approval not offered");
            assert!(handle.update(cx, |v, w, a| v.visual_peek_focused(w, a)).unwrap(), "peek not focused");
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, w, a| {
                    w.refresh(); w.draw(a).clear(a);
                    w.render_to_image().unwrap().save(directory.join(name)).unwrap();
                }).unwrap();
            };
            wait(300).await;
            shot("1-peek-light.png", cx);
            cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Dark, a).unwrap());
            wait(300).await;
            shot("1-peek-dark.png", cx);
            cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Light, a).unwrap());
            let screen = || match call(Request::ReadScreen { id: "agent".into(), rows: 30 }) { Reply::Text(t) => t, x => panic!("{x:?}") };
            handle.update(cx, |v, _, a| v.visual_peek_answer(true, a)).unwrap();
            let mut answered = false;
            for _ in 0..50 {
                wait(100).await;
                if screen().contains("answer=[]") { answered = true; break; }
            }
            assert!(answered, "允许一次 never reached the Agent: {}", screen());
            // The peek never resized the terminal it looked at.
            assert!(screen().contains("size=30 100"), "peek resized the terminal: {}", screen());
            wait(500).await;
            shot("2-answered.png", cx);
            // The same answer again finds no approval and types nothing.
            let w = workspace();
            let stale = rpc(&socket, &Request::AnswerApproval { id: "agent".into(), question: "Do you want to proceed?".into(), allow: true });
            assert!(stale.is_err(), "answered twice: {stale:?} rev {}", w.revision);
            // Esc closes the peek and is not typed into the Agent.
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_keystroke(gpui::Keystroke::parse("escape").unwrap(), a);
            }).unwrap();
            wait(300).await;
            assert!(handle.update(cx, |v, _, _| v.visual_peek_state()).unwrap().is_none(), "Esc did not close the peek");
            call(Request::SendText { id: "agent".into(), text: "done".into(), submit: true });
            let mut ended = String::new();
            for _ in 0..50 {
                wait(100).await;
                ended = screen();
                if ended.contains("end=[") { break; }
            }
            assert!(ended.contains("end=[done]"), "Esc reached the Agent: {ended}");
            // Opening the peeked terminal's own tab ends the peek, so the tab's pane owns it.
            handle.update(cx, |v, w, a| v.visual_peek_row("agent", w, a)).unwrap();
            assert!(handle.update(cx, |v, _, _| v.visual_peek_state()).unwrap().is_some());
            handle.update(cx, |v, _, a| v.visual_peek_show_tab("agent", a)).unwrap();
            assert!(handle.update(cx, |v, _, _| v.visual_peek_state()).unwrap().is_none(), "peek kept over its own tab");
            wait(800).await;
            shot("3-own-tab.png", cx);
            eprintln!("peek native PASS: hidden approval peeked, answered once, size kept, Esc closes without reaching the Agent");
            // 置顶: the row's dot shows on hover, a real click pins the terminal, the pin is
            // saved, and it comes off once the terminal ends.
            let row = gpui::point(px(100.), px(292.));
            let dot = gpui::point(px(theme::Theme::SIDEBAR - 22.), px(292.));
            cx.update_window(handle.into(), |_, w, a| {
                use gpui::InputEvent;
                w.dispatch_event(gpui::MouseMoveEvent { position: row, pressed_button: None, modifiers: Default::default() }.to_platform_input(), a);
                w.refresh(); w.draw(a).clear(a);
            }).unwrap();
            wait(200).await;
            shot("4-hover.png", cx);
            cx.update_window(handle.into(), |_, w, a| native_click(w, a, dot, gpui::MouseButton::Left)).unwrap();
            wait(300).await;
            assert_eq!(handle.update(cx, |v, _, _| v.visual_pins()).unwrap(), vec!["agent".to_owned()], "the dot did not pin");
            assert!(handle.update(cx, |v, _, _| v.visual_peek_state()).unwrap().is_none(), "the dot's click also reached the row");
            let saved = std::fs::read_to_string(crate::ui::status_colors::config_dir().unwrap().join("sidebar")).unwrap();
            assert!(saved.lines().any(|l| l == "pin agent"), "pin not saved: {saved:?}");
            shot("5-pinned.png", cx);
            call(Request::Stop { id: "agent".into() });
            let mut gone = false;
            for _ in 0..50 {
                wait(200).await;
                if handle.update(cx, |v, _, _| v.visual_pins()).unwrap().is_empty() { gone = true; break; }
            }
            assert!(gone, "pin outlived its terminal");
            let saved = std::fs::read_to_string(crate::ui::status_colors::config_dir().unwrap().join("sidebar")).unwrap();
            assert!(!saved.contains("pin "), "ended pin still saved: {saved:?}");
            shot("6-unpinned.png", cx);
            eprintln!("pin native PASS: hover dot, click pins and saves, ended terminal unpins");
            // 在编辑器里看滚动内容: the pane menu opens the terminal's text with $EDITOR in a
            // new tab; the stand-in editor (KEEPLINE_TEST_EDITOR) copies what it was given.
            if let Some(edited) = std::env::var_os("KEEPLINE_EDITED") {
                let edited = PathBuf::from(edited);
                handle.update(cx, |v, _, a| v.visual_peek_show_tab("shell", a)).unwrap();
                wait(500).await;
                let at = gpui::point(px(theme::Theme::SIDEBAR + 200.), px(theme::Theme::TOP + theme::Theme::TABS + theme::Theme::PANE_HEADER + 120.));
                cx.update_window(handle.into(), |_, w, a| { w.refresh(); w.draw(a).clear(a); native_click(w, a, at, gpui::MouseButton::Right) }).unwrap();
                wait(300).await;
                shot("7-menu.png", cx);
                let item = handle.update(cx, |v, _, _| v.visual_menu_point(7)).unwrap();
                cx.update_window(handle.into(), |_, w, a| native_click(w, a, item, gpui::MouseButton::Left)).unwrap();
                let mut text = String::new();
                for _ in 0..100 {
                    wait(200).await;
                    if let Ok(t) = std::fs::read_to_string(&edited) { text = t; break; }
                }
                if text.is_empty() { shot("8-editor-failed.png", cx); }
                assert!(text.contains("~/keepline $ cargo test"), "editor got: {text:?}");
                let saved = root.join("scrollback/shell.txt");
                let mode = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&saved).unwrap().permissions()) & 0o777;
                assert_eq!(mode, 0o600, "scrollback file readable by others");
                wait(500).await;
                shot("8-editor.png", cx);
                eprintln!("scrollback native PASS: menu opens the terminal text with $EDITOR, file private");
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // The sidebar shows the space's branch; the pane menu explains the pane's state.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_WHY_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            std::fs::create_dir_all(&directory).unwrap();
            // Near the top of the shown pane so the menu opens where it was asked,
            // not pushed up to fit; clicks hit what was last drawn.
            // Below the pane header and the 「停在 …」 line under it, inside the screen.
            let at = gpui::point(
                px(theme::Theme::SIDEBAR + 120.),
                px(theme::Theme::TOP + theme::Theme::TABS + theme::Theme::PANE_HEADER + 120.),
            );
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh();
                window.draw(app).clear(app);
                native_click(window, app, at, gpui::MouseButton::Right)
            })
            .unwrap();
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            // 为什么是这个状态, after 在编辑器里看滚动内容.
            let item = handle.update(cx, |v, _, _| v.visual_menu_point(9)).unwrap();
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join("1-menu.png"))
                    .unwrap();
                native_click(window, app, item, gpui::MouseButton::Left);
            })
            .unwrap();
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            assert!(
                handle.update(cx, |v, _, _| v.visual_menu_why()).unwrap(),
                "为什么是这个状态 turns the menu into the explanation"
            );
            cx.update_window(handle.into(), |_, window, app| {
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join("2-why.png"))
                    .unwrap();
            })
            .unwrap();
            eprintln!("why native PASS: the pane menu explains the state");
            cx.update(|cx| cx.quit());
        })
        .detach();
        return;
    }
    // The sidebar's 「提醒」 opens settings on 通知, where 提示音 turns on and is saved.
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_REMINDER_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let wait = move |ms: u64| executor.timer(Duration::from_millis(ms));
            wait(2000).await;
            std::fs::create_dir_all(&directory).unwrap();
            let shot = |name: &str, cx: &mut gpui::AsyncApp| {
                cx.update_window(handle.into(), |_, window, app| {
                    window.refresh();
                    window.draw(app).clear(app);
                    window
                        .render_to_image()
                        .unwrap()
                        .save(directory.join(format!("{name}.png")))
                        .unwrap();
                })
                .unwrap();
            };
            // 「提醒」 sits in the sidebar footer, left of 设置.
            let x = std::env::var("KL_REMINDER_X")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(theme::Theme::SIDEBAR - 72.);
            cx.update_window(handle.into(), |_, window, app| {
                let at = gpui::point(
                    px(x),
                    window.viewport_size().height
                        - px(theme::Theme::S8 + theme::Theme::ROW / 2.),
                );
                native_click(window, app, at, gpui::MouseButton::Left);
            })
            .unwrap();
            wait(400).await;
            assert_eq!(
                handle
                    .update(cx, |v, _, a| v.visual_settings_page_name(a))
                    .unwrap(),
                "通知",
                "「提醒」 opens settings on 通知"
            );
            shot("1-reminders", cx);
            assert!(!handle.update(cx, |v, _, _| v.visual_settings_values()).unwrap().sound);
            handle
                .update(cx, |v, _, cx| v.visual_settings_toggle_sound(cx))
                .unwrap();
            wait(1000).await;
            assert!(
                handle.update(cx, |v, _, _| v.visual_settings_values()).unwrap().sound,
                "提示音 is saved by the host"
            );
            shot("2-sound-on", cx);
            eprintln!("reminder native PASS: 提醒 opens 通知, 提示音 saved");
            cx.update(|cx| cx.quit());
        })
        .detach();
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
