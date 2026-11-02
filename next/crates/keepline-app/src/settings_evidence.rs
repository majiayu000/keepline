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
        // Every page in light and dark, for looking at the layout; changes nothing saved.
        if phase == "tour" {
            for (scheme, look) in [("light", crate::ui::appearance::Appearance::Light), ("dark", crate::ui::appearance::Appearance::Dark)] {
                cx.update(|a| crate::ui::appearance::choose(look, a).unwrap());
                for (i, page) in workbench::Workbench::visual_settings_pages().into_iter().enumerate() {
                    handle.update(cx, |v, _, a| v.visual_settings_show(page, a)).unwrap();
                    wait(300).await;
                    shot(&format!("tour-{scheme}-{i}"), cx);
                }
            }
            eprintln!("settings tour PASS");
            cx.update(|a| a.quit());
            return;
        }
        // 钉住这一轮的提问: the row alone under its search, default (bar only) then all on.
        if phase == "pinned-ask" {
            for (name, choice) in [
                ("pinned-ask-default", crate::ui::pinned_ask::PinnedAsk::default()),
                ("pinned-ask-all", crate::ui::pinned_ask::PinnedAsk { bar: true, float: true, button: true }),
            ] {
                cx.update(|a| crate::ui::pinned_ask::choose(choice, a).unwrap());
                handle.update(cx, |v, _, a| {
                    let field = v.visual_settings_query(a);
                    field.update(a, |f, cx| { f.text = "钉住".into(); cx.notify(); });
                }).unwrap();
                wait(300).await;
                shot(name, cx);
            }
            let saved = std::fs::read_to_string(crate::ui::status_colors::config_dir().unwrap().join("pinned-ask")).unwrap();
            assert_eq!(saved, "bar,float,button");
            eprintln!("settings pinned-ask PASS");
            cx.update(|a| a.quit());
            return;
        }
        // 插件: install from the catalog with real clicks, confirm one that runs commands,
        // switch one of its actions off, uninstall, then pick a plugin's animation preset.
        // KEEPLINE_PLUGIN_CLICKS="x,y;…" in panel points, in this order: 安装 落印,
        // 安装 常用命令, 确认安装, its first action's switch, 卸载 落印, then (on 外观 with
        // the search 标记完成) the plugin preset 慢扫. Missing points stop after the shots.
        if phase == "plugins" {
            use keepline_core::{action, effect, plugin};
            let clicks: Vec<(f32, f32)> = std::env::var("KEEPLINE_PLUGIN_CLICKS").unwrap_or_default()
                .split(';').filter_map(|p| p.split_once(',')).map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).collect();
            let at = |i: usize| clicks.get(i).copied();
            handle.update(cx, |v, _, a| v.visual_settings_show("插件", a)).unwrap();
            wait(400).await;
            shot("plugins-start", cx);
            assert!(plugin::installed("slow-sweep").is_some(), "the scratch plugin loads");
            let Some(stamp) = at(0) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(stamp.0, stamp.1, cx); wait(1500).await;
            assert!(plugin::installed("complete-stamp").is_some(), "安装 installs and reloads");
            shot("plugins-installed", cx);
            let Some(actions) = at(1) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(actions.0, actions.1, cx); wait(400).await;
            assert!(plugin::installed("common-actions").is_none(), "commands wait for 确认安装");
            shot("plugins-confirm", cx);
            let Some(confirm) = at(2) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(confirm.0, confirm.1, cx); wait(1500).await;
            assert!(plugin::installed("common-actions").is_some());
            assert!(!action::all().is_empty(), "its actions load without a restart");
            shot("plugins-actions", cx);
            let Some(switch) = at(3) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(switch.0, switch.1, cx); wait(400).await;
            assert!(!action::all()[0].enabled(), "the switch turns its action off");
            shot("plugins-action-off", cx);
            let Some(remove) = at(4) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(remove.0, remove.1, cx); wait(600).await;
            assert!(plugin::installed("complete-stamp").is_none(), "卸载 removes it");
            shot("plugins-removed", cx);
            handle.update(cx, |v, _, a| {
                v.visual_settings_show("外观", a);
                let field = v.visual_settings_query(a);
                field.update(a, |f, cx| { f.text = "标记完成".into(); cx.notify(); });
            }).unwrap();
            wait(400).await;
            shot("plugins-look", cx);
            let Some(slow) = at(5) else { eprintln!("settings plugins SHOTS"); cx.update(|a| a.quit()); return; };
            click(slow.0, slow.1, cx); wait(400).await;
            assert_eq!(effect::picked(), Some(effect::Pick::Plugin("slow-sweep".into())));
            assert_eq!(effect::complete().duration, Duration::from_millis(3_400));
            shot("plugins-look-picked", cx);
            eprintln!("settings plugins PASS");
            cx.update(|a| a.quit());
            return;
        }
        // 标记完成动画: pick one with a real click, then 试播 on the shown pane, which must
        // stay open with nothing marked. KEEPLINE_COMPLETE_CLICKS="x,y;x,y" are the
        // panel points of 落印 and 试播 once the search leaves only that row.
        if phase == "complete" {
            click(106., 76., cx);
            cx.update_window(handle.into(), |_, w, a| {
                for key in ["e", "f", "f", "e", "c", "t", "s"] { w.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), a); }
            }).unwrap();
            wait(300).await;
            shot("complete-row", cx);
            let clicks: Vec<(f32, f32)> = std::env::var("KEEPLINE_COMPLETE_CLICKS").unwrap_or_default()
                .split(';').filter_map(|p| p.split_once(',')).map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).collect();
            if let [stamp, preview] = clicks[..] {
                click(stamp.0, stamp.1, cx); wait(300).await;
                assert_eq!(keepline_core::effect::picked(), Some(keepline_core::effect::Pick::Builtin(keepline_core::effect::Style::Stamp)));
                let saved = std::fs::read_to_string(crate::ui::status_colors::config_dir().unwrap().join("complete")).unwrap();
                assert_eq!(saved, "stamp");
                shot("complete-picked", cx);
                let pane = handle.update(cx, |v, _, _| v.visual_shown_pane()).unwrap();
                pane.update(cx, |p, a| p.visual_type("printf 'preview of the completion animation\\n'\r", a));
                wait(400).await;
                click(preview.0, preview.1, cx); wait(700).await;
                assert!(handle.update(cx, |v, w, a| v.visual_settings(w, a)).unwrap().is_none(), "试播 closes 设置");
                shot("complete-preview-mid", cx);
                wait(2600).await;
                shot("complete-preview-after", cx);
                assert!(handle.update(cx, |v, _, _| v.visual_shown_pane_alive()).unwrap(), "试播 keeps the pane");
            }
            eprintln!("settings complete PASS");
            cx.update(|a| a.quit());
            return;
        }
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
            click(578., 148., cx); wait(1000).await;
            assert_eq!(values(cx).default_agent, keepline_core::model::Agent::Claude);
            click(723., 253., cx); wait(1000).await;
            assert_eq!(values(cx).away_minutes, 15);
            click(814., 315., cx); wait(1000).await;
            assert_eq!(values(cx).stale_days, 7);
            shot("general-saved", cx);
        }
        click(60., 153., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "外观");
        shot("appearance", cx);
        if phase == "1" {
            click(792., 176., cx); wait(200).await;
            assert_eq!(crate::ui::theme::Theme::scheme(), crate::ui::theme::Scheme::Dark);
            click(60., 186., cx); wait(200).await;
            assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "终端");
            click(300., 506., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::current().family, "Menlo");
            click(874., 314., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::size(), 14.);
            click(778., 380., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::line_height(), 1.4);
            shot("appearance-dark-saved", cx);
            let blocked = crate::ui::terminal_text::path().unwrap().with_extension("json.tmp");
            std::fs::create_dir(&blocked).unwrap();
            // The taller preview at 14 pt / 1.4× moves the size row down.
            click(874., 344., cx); wait(200).await;
            assert_eq!(crate::ui::terminal_text::size(), 14., "failed save must not apply");
            shot("save-error", cx);
            std::fs::remove_dir(blocked).unwrap();
        }
        click(67., 220., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "快捷键");
        shot("shortcuts", cx);
        click(60., 255., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "通知");
        shot("notifications", cx);
        click(60., 289., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "插件");
        shot("plugins", cx);
        click(60., 323., cx); wait(200).await;
        assert_eq!(handle.update(cx, |v, _, a| v.visual_settings_page_name(a)).unwrap(), "关于");
        shot("about", cx);
        if phase == "inspect" {
            eprintln!("settings native inspection: seven category clicks and screenshots");
            cx.update(|a| a.quit());
            return;
        }
        // Native search input must receive characters without triggering workbench keys.
        click(106., 76., cx);
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
