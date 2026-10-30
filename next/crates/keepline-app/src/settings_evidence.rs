//! Isolated native settings acceptance. Uses GPUI event dispatch and real PNG output.
use super::*;

pub(super) fn start(
    handle: Option<WindowHandle<workbench::Workbench>>,
    evidence: Option<PathBuf>,
    cx: &mut App,
) -> bool {
    let Ok(phase) = std::env::var("KEEPLINE_SETTINGS_EVIDENCE") else {
        return false;
    };
    let (Some(handle), Some(directory)) = (handle, evidence) else {
        return false;
    };
    assert!(
        std::env::var_os("XDG_CONFIG_HOME").is_some(),
        "scratch config required"
    );
    cx.spawn(async move |cx| {
        let executor = cx.background_executor().clone();
        let wait = move |ms| executor.timer(Duration::from_millis(ms));
        wait(2000).await;
        std::fs::create_dir_all(&directory).unwrap();
        handle.update(cx, |v, w, a| v.show_settings(w, a)).unwrap();
        wait(300).await;
        let shot = |name: &str, cx: &mut gpui::AsyncApp| {
            cx.update_window(handle.into(), |_, w, a| {
                w.refresh(); w.draw(a).clear(a);
                w.render_to_image().unwrap().save(directory.join(format!("{name}.png"))).unwrap();
            }).unwrap();
        };
        // Coordinates are relative to the visible settings panel, in logical pixels.
        let click = |x: f32, y: f32, cx: &mut gpui::AsyncApp| {
            cx.update_window(handle.into(), |_, w, a| {
                let origin = gpui::point((w.viewport_size().width - px(theme::Theme::SETTINGS_WIDTH)) / 2., (w.viewport_size().height - px(664.)) / 2.);
                native_click(w, a, origin + gpui::point(px(x), px(y)), gpui::MouseButton::Left);
            }).unwrap();
        };
        let values = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, _, _| v.visual_settings_values()).unwrap();
        if phase == "2" {
            let saved = values(cx);
            assert_eq!(saved.default_agent, keepline_core::model::Agent::Claude);
            assert_eq!(saved.away_minutes, 15);
            assert_eq!(saved.stale_days, 7);
            let text = crate::ui::terminal_text::current();
            assert_eq!(text.family, "Menlo"); assert_eq!(text.size, 14.); assert_eq!(text.line_height, 1.4);
            assert_eq!(crate::ui::theme::Theme::scheme(), crate::ui::theme::Scheme::Dark);
        }
        shot("general", cx);
        if phase == "1" {
            click(875., 213., cx); wait(1000).await;
            assert_eq!(values(cx).default_agent, keepline_core::model::Agent::Claude);
            click(760., 280., cx); wait(1000).await;
            assert_eq!(values(cx).away_minutes, 15);
            click(850., 347., cx); wait(1000).await;
            assert_eq!(values(cx).stale_days, 7);
            shot("general-saved", cx);
        }
        click(70., 128., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "外观与终端");
        shot("appearance", cx);
        if phase == "1" {
            click(900., 213., cx); wait(200).await;
            assert_eq!(crate::ui::theme::Theme::scheme(), crate::ui::theme::Scheme::Dark);
            click(822., 280., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::current().family, "Menlo");
            click(906., 347., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::size(), 14.);
            click(853., 413., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::line_height(), 1.4);
            shot("appearance-dark-saved", cx);
            let blocked = crate::ui::terminal_text::path().unwrap().with_extension("json.tmp");
            std::fs::create_dir(&blocked).unwrap();
            click(906., 347., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::size(), 14., "failed save must not apply");
            shot("save-error", cx);
            std::fs::remove_dir(blocked).unwrap();
        }
        click(70., 164., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "快捷键");
        shot("shortcuts", cx);
        click(70., 200., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "通知");
        shot("notifications", cx);
        if phase == "inspect" {
            eprintln!("settings native inspection: four category clicks and screenshots");
            cx.update(|a| a.quit());
            return;
        }
        // Native search input must receive characters without triggering workbench keys.
        click(280., 98., cx);
        cx.update_window(handle.into(), |_, w, a| {
            for key in ["f", "o", "n", "t"] { w.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), a); }
        }).unwrap();
        wait(200).await;
        let query = handle.update(cx, |v, _, a| v.visual_settings_query(a)).unwrap();
        assert_eq!(cx.read_entity(&query, |f, _| f.text.clone()), "font");
        shot("search", cx);
        // An unmatched search has an explicit empty state, then native Escape closes.
        cx.update_window(handle.into(), |_, w, a| {
            w.dispatch_keystroke(gpui::Keystroke::parse("cmd-a").unwrap(), a);
            for key in ["z", "z", "z", "z"] { w.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), a); }
        }).unwrap();
        wait(200).await;
        shot("search-empty", cx);
        cx.update_window(handle.into(), |_, w, a| { w.dispatch_keystroke(gpui::Keystroke::parse("escape").unwrap(), a); }).unwrap();
        assert!(handle.update(cx, |v, w, a| v.visual_settings(w, a)).unwrap().is_none());
        if phase == "2" {
            let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
            pane.update(cx, |p, a| p.visual_type("printf '\\033[2J\\033[HSettings typography: Menlo 14pt / 1.4x\\n中文输入与等宽文字 0123456789\\n'\r", a));
            wait(500).await;
            shot("terminal-after-restart", cx);
        }
        eprintln!("settings native PASS phase {phase}: category clicks, real saves/restart, failed write unchanged, search text and Escape");
        cx.update(|a| a.quit());
    }).detach();
    true
}
