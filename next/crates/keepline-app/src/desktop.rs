//! Event-driven macOS status item and notification delivery; no timers or process probes.
use crate::{connection::RecognitionConnection, ui::theme::Theme as T};
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded},
};
use gpui::{App, AppContext, Global};
use keepline_core::{
    content::{Inbox, now, short},
    model::*,
    protocol::*,
};
use serde_json::json;
use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
    sync::OnceLock,
};

#[derive(Default)]
pub struct Status(pub String);
impl Global for Status {}
const MENUBAR_FAILED: &str = "菜单栏连接失败：";
/// The route the panel's 查看诊断 sends; session ids never take this form.
pub const DIAGNOSTICS: &str = "diagnostics:";
/// The terminal in the focused pane of the active window, if any.
#[derive(Default)]
pub struct Watching(pub Option<String>);
impl Global for Watching {}
static EVENTS: OnceLock<UnboundedSender<String>> = OnceLock::new();
unsafe extern "C" {
    fn kl_desktop_route_applied(
        session: *const c_char,
        terminal: *const c_char,
        tab: *const c_char,
    );
    fn kl_desktop_install(callback: extern "C" fn(*const c_char));
    fn kl_desktop_update(json: *const c_char);
    fn kl_notifications_enable();
    fn kl_desktop_show();
    fn kl_request_attention();
    fn kl_set_dock_icon(png: *const u8, length: usize);
    fn kl_desktop_notify(
        session: *const c_char,
        title: *const c_char,
        body: *const c_char,
        sound: i32,
    );
    fn kl_play_sound();
}
pub fn set_dock_icon(png: &[u8]) {
    unsafe { kl_set_dock_icon(png.as_ptr(), png.len()) }
}
pub fn route_applied(session: &str, terminal: &str, tab: &str) {
    unsafe {
        kl_desktop_route_applied(
            cstring(session).as_ptr(),
            cstring(terminal).as_ptr(),
            cstring(tab).as_ptr(),
        );
    }
}
extern "C" fn event(value: *const c_char) {
    if value.is_null() {
        return;
    }
    // Native strings live through the callback; copy before returning to AppKit.
    let value = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned();
    if let Some(tx) = EVENTS.get() {
        let _ = tx.unbounded_send(value);
    }
}
fn cstring(value: &str) -> CString {
    CString::new(value.replace('\0', " ")).expect("removed NUL")
}
fn fact(w: &Workspace, s: &AgentSession) -> Option<(String, String)> {
    let c = w.content.get(&s.id)?;
    if c.hidden_for(s).is_some() {
        return None;
    }
    let group = c.group(s, w, now());
    let body = match group {
        Inbox::Waiting => c.waiting.clone(),
        Inbox::Error => c.error.clone(),
        _ => return None,
    };
    // One dialog is one event however many signals report it or refine its text; an error
    // is keyed by its text.
    let event = match group {
        Inbox::Waiting => format!("episode {}", c.waiting_episode),
        _ => body.clone(),
    };
    Some((
        format!("{:?}:{}:{}:{}", group, c.turn_key, c.turn_started, event),
        format!("{} · {}", group.label(), short(&body, 180)),
    ))
}
fn entered(
    before: &Workspace,
    after: &Workspace,
    watching: Option<&str>,
) -> Vec<(String, String, String, String)> {
    if !after.settings.notifications && !after.settings.sound {
        return vec![];
    }
    after
        .agent_sessions
        .iter()
        .filter(|s| s.managed() && (watching.is_none() || s.terminal.as_deref() != watching))
        .filter_map(|s| {
            let (key, body) = fact(after, s)?;
            let old = before
                .agent_sessions
                .iter()
                .find(|p| p.id == s.id)
                .and_then(|s| fact(before, s));
            if old.as_ref().is_some_and(|(old, _)| old == &key) {
                return None;
            }
            let title = short(&after.session_title(s), 72);
            // Name the project first: several panes often run the same kind of task.
            let title = match space_name(after, s) {
                Some(space) => format!("{space} · {title}"),
                None => title,
            };
            Some((s.id.clone(), key, title, body))
        })
        .collect()
}
/// The project a session's terminal belongs to, as notifications and the panel name it.
fn space_name(w: &Workspace, s: &AgentSession) -> Option<String> {
    w.terminals
        .iter()
        .find(|t| Some(&t.id) == s.terminal.as_ref())
        .and_then(|t| w.spaces.iter().find(|p| p.id == t.space))
        .map(|p| p.name.clone())
}
/// What the menu bar knows about its own link to the host: the panel says when its
/// numbers stopped being live instead of showing old ones as current.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Link {
    phase: RecognitionPhase,
    /// The last refresh's failure, while the host cannot be reached.
    error: Option<String>,
    /// When the current state began: connected since, or, while the host cannot be
    /// reached, the last time the panel's workspace was fresh.
    since: u64,
}
impl Link {
    fn fresh(phase: RecognitionPhase) -> Self {
        Self {
            phase,
            error: None,
            since: now(),
        }
    }
    fn phase(&self) -> &'static str {
        match (&self.error, self.phase) {
            (None, RecognitionPhase::Starting) => "starting",
            (None, _) => "healthy",
            (Some(_), RecognitionPhase::Recovering) => "recovering",
            (Some(_), _) => "interrupted",
        }
    }
}
/// `13:04`, the minute the panel's data dates from.
fn minute(at: u64) -> String {
    let mut clock = crate::model::time::clock(at);
    clock.truncate(5);
    clock
}
/// Rows the panel lists; the rest are counted under them.
const SHOWN: usize = 5;
fn panel(w: &Workspace, link: &Link) -> String {
    let mut rows = Vec::new();
    let mut review = 0;
    let mut running = 0;
    let mut stale = 0;
    let mut waiting = 0;
    let mut errors = 0;
    let at = now();
    for s in &w.agent_sessions {
        let Some(c) = w.content.get(&s.id) else {
            continue;
        };
        if c.hidden_for(s).is_some() {
            continue;
        }
        let group = c.group(s, w, at);
        if !s.managed() && matches!(group, Inbox::Waiting | Inbox::Error) {
            continue;
        }
        match group {
            Inbox::Review => review += 1,
            Inbox::Running => running += 1,
            Inbox::Stale => stale += 1,
            Inbox::Waiting => waiting += 1,
            Inbox::Error => errors += 1,
            _ => (),
        };
        if matches!(group, Inbox::Waiting | Inbox::Error) {
            let minutes = at.saturating_sub(c.updated) / 60;
            let meta = match space_name(w, s) {
                Some(space) => format!("{space} · {}", s.runtime.name()),
                None => s.runtime.name().to_owned(),
            };
            let waits = group == Inbox::Waiting;
            rows.push((
                (!waits, std::cmp::Reverse(c.updated)),
                json!({
                    "id": s.id,
                    "title": short(&w.session_title(s), 24),
                    "summary": short(if waits { &c.waiting } else { &c.error }, 40),
                    "age": if minutes < 60 {
                        format!("{minutes} 分钟")
                    } else {
                        format!("{} 小时", minutes / 60)
                    },
                    "kind": if waits { "wait" } else { "error" },
                    "meta": meta,
                }),
            ));
        }
    }
    // Waiting first, then the newest.
    rows.sort_by_key(|r| r.0);
    let total = rows.len();
    let offline = link.error.is_some();
    let stopped = minute(link.since);
    let (heading, when) = if offline {
        ("连不上后台".to_owned(), format!("数据停在 {stopped}"))
    } else if total == 0 {
        ("没有需要你的".to_owned(), "刚刚更新".to_owned())
    } else {
        (format!("{total} 项需要你"), "刚刚更新".to_owned())
    };
    let (section, section_note) = if offline {
        (format!("{stopped} 时需要你"), "可能已经变了")
    } else {
        ("需要你".to_owned(), "等你在前，按时间")
    };
    // The empty view says what happens next instead of leaving a blank.
    let empty = (!offline && total == 0).then(|| {
        let who = if running > 0 {
            format!("在跑的 {running} 个停下来要你时")
        } else {
            "有 Agent 停下来要你时".to_owned()
        };
        let (what, note) = match (w.settings.notifications, w.settings.sound) {
            (true, _) => ("会发系统通知", "通知已开 · 你正看着的画面不会再弹"),
            (false, true) => ("会响提示音", "系统通知没开 · 在设置的通知页打开"),
            (false, false) => ("会出现在这里", "通知没开 · 在设置的通知页打开"),
        };
        json!({"title": format!("{who}，{what}"), "note": note})
    });
    let (foot, foot_count, foot_note) = if offline {
        ("重连后自动刷新", None, "通知暂停")
    } else {
        ("很久没动", Some(stale), "外部会话在鸟瞰里看")
    };
    json!({
        "badge": if offline || total == 0 { "K".into() } else { format!("K {total}") },
        "heading": heading,
        "when": when,
        "needs": total,
        "counts": {
            "waiting": waiting, "errors": errors, "review": review,
            "running": running, "stale": stale,
        },
        "section": section,
        "section_note": section_note,
        "rows": rows.into_iter().take(SHOWN).map(|r| r.1).collect::<Vec<_>>(),
        "more": total.saturating_sub(SHOWN),
        "empty": empty,
        "foot": foot,
        "foot_count": foot_count,
        "foot_note": foot_note,
        "notifications": w.settings.notifications,
        // The subscription retries on its own and does not count its attempts.
        "connection": {"phase": link.phase(), "error": link.error, "since": link.since},
        "tokens": {
            "width": T::MENUBAR_WIDTH, "height": T::MENUBAR_HEIGHT, "gap": T::S12,
            "font": T::BODY_SIZE, "title": T::MENU_TITLE, "count": T::MENU_COUNT,
            "small": T::SMALL, "meta": T::META, "chip": T::CHIP_SIZE,
            "button": T::MENU_BUTTON, "radius": T::S8,
            "light": T::MENU_LIGHT, "dark": T::MENU_DARK,
        },
    })
    .to_string()
}
pub fn show() {
    unsafe {
        kl_desktop_show();
    }
}
pub fn request_attention() {
    unsafe {
        kl_request_attention();
    }
}
pub fn start(socket: &Path, initial: Workspace, cx: &mut App) -> UnboundedReceiver<String> {
    let (tx, events) = unbounded();
    let _ = EVENTS.set(tx);
    cx.set_global(Status::default());
    // All AppKit entry points run on the GPUI/main thread.
    unsafe {
        kl_desktop_install(event);
        kl_desktop_update(
            cstring(&panel(&initial, &Link::fresh(RecognitionPhase::Starting))).as_ptr(),
        );
    }
    let (status_tx, mut status_rx) = unbounded();
    let connection = RecognitionConnection::new(socket, status_tx);
    let socket = socket.to_owned();
    cx.spawn(async move |cx| {
        let _connection = connection;
        let mut before = initial;
        let mut ready = false;
        let mut link = Link::fresh(RecognitionPhase::Starting);
        let mut previous_panel = panel(&before, &link);
        let mut fresh = now();
        while let Some(status) = status_rx.next().await {
            let socket_read = socket.clone();
            let result = cx
                .background_spawn(async move { rpc(&socket_read, &Request::Workspace) })
                .await;
            link.phase = status.phase;
            if let Err(e) = &result {
                // Keep the last numbers on screen, marked with the minute they date from.
                if link.error.is_none() {
                    link.since = fresh;
                }
                link.error = Some(e.to_string());
                let value = panel(&before, &link);
                if previous_panel != value {
                    cx.update(|_| unsafe {
                        kl_desktop_update(cstring(&value).as_ptr());
                    });
                    previous_panel = value;
                }
            }
            let after = match result {
                Ok(Reply::Workspace(w)) => {
                    // A host restart breaks one refresh; clear its note once the next one works.
                    cx.update(|cx| {
                        if cx.global::<Status>().0.starts_with(MENUBAR_FAILED) {
                            cx.set_global(Status::default());
                        }
                    });
                    w
                }
                Err(e) => {
                    cx.update(|cx| cx.set_global(Status(format!("{MENUBAR_FAILED}{e}"))));
                    continue;
                }
                _ => continue,
            };
            fresh = now();
            if link.error.take().is_some() {
                link.since = fresh;
            }
            let value = panel(&after, &link);
            if previous_panel != value {
                cx.update(|_| unsafe {
                    kl_desktop_update(cstring(&value).as_ptr());
                });
                previous_panel = value;
            }
            if !before.settings.notifications && after.settings.notifications {
                cx.update(|_| unsafe { kl_notifications_enable() });
            }
            if ready {
                let watching =
                    cx.update(|cx| cx.try_global::<Watching>().and_then(|w| w.0.clone()));
                for (session, key, title, body) in entered(&before, &after, watching.as_deref()) {
                    let socket_claim = socket.clone();
                    let claim_session = session.clone();
                    match cx
                        .background_spawn(async move {
                            rpc(
                                &socket_claim,
                                &Request::ClaimNotification {
                                    session: claim_session,
                                    key,
                                },
                            )
                        })
                        .await
                    {
                        Ok(Reply::NotificationClaimed(true)) => {
                            let settings = &after.settings;
                            cx.update(|_| unsafe {
                                if settings.notifications {
                                    kl_desktop_notify(
                                        cstring(&session).as_ptr(),
                                        cstring(&title).as_ptr(),
                                        cstring(&body).as_ptr(),
                                        i32::from(settings.sound),
                                    );
                                } else {
                                    kl_play_sound();
                                }
                            });
                        }
                        Err(e) => {
                            cx.update(|cx| cx.set_global(Status(format!("通知去重记录失败：{e}"))));
                        }
                        _ => (),
                    }
                }
            }
            ready |= status.phase == RecognitionPhase::Healthy;
            before = after;
        }
    })
    .detach();
    events
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notifications_only_follow_new_attention_facts_and_opt_in() {
        let mut w = Workspace::default();
        let s = AgentSession {
            id: "s".into(),
            runtime: Agent::Codex,
            cli_session_id: "cli".into(),
            terminal: Some("pty".into()),
            current: false,
            task: None,
            outcome: Outcome::Open,
            updated: now(),
            cwd: "/project".into(),
            transcript: "log".into(),
            binding: None,
            name: String::new(),
        };
        w.agent_sessions.push(s);
        let before = w.clone();
        w.content.insert(
            "s".into(),
            keepline_core::content::Content {
                interactive: true,
                turn: keepline_core::content::Turn::Waiting,
                waiting: "选哪一个？ A / B".into(),
                updated: now(),
                turn_started: now(),
                ..Default::default()
            },
        );
        assert!(entered(&before, &w, None).is_empty());
        // The sound alone is reason enough to report it.
        w.settings.sound = true;
        assert_eq!(entered(&before, &w, None).len(), 1);
        w.settings.sound = false;
        w.settings.notifications = true;
        assert_eq!(entered(&before, &w, None).len(), 1);
        assert!(entered(&w, &w, None).is_empty());
        let previous = w.clone();
        w.content.get_mut("s").unwrap().waiting = "选哪一个？（Hook 补充的说明）".into();
        assert!(
            entered(&previous, &w, None).is_empty(),
            "a second signal for the same dialog is not news"
        );
        let c = w.content.get_mut("s").unwrap();
        c.waiting = "是否执行测试？".into();
        c.waiting_episode += 1;
        assert_eq!(entered(&previous, &w, None).len(), 1);
        assert!(
            entered(&previous, &w, Some("pty")).is_empty(),
            "no banner for the pane the user is looking at"
        );
        assert_eq!(entered(&previous, &w, Some("other")).len(), 1);
        w.agent_sessions[0].outcome = Outcome::Completed;
        assert!(entered(&previous, &w, None).is_empty());
        let value: serde_json::Value =
            serde_json::from_str(&panel(&previous, &Link::fresh(RecognitionPhase::Healthy)))
                .unwrap();
        assert_eq!(value["rows"].as_array().unwrap().len(), 1);
        // Banners and the menu panel use the name the sidebar shows, not the raw first ask.
        let mut renamed = previous.clone();
        renamed.agent_sessions[0].name = "发布前检查".into();
        let value: serde_json::Value =
            serde_json::from_str(&panel(&renamed, &Link::fresh(RecognitionPhase::Healthy)))
                .unwrap();
        assert_eq!(value["rows"][0]["title"], "发布前检查");
        let mut asked = renamed.clone();
        let c = asked.content.get_mut("s").unwrap();
        c.waiting = "要继续吗？".into();
        c.waiting_episode += 1;
        assert_eq!(entered(&renamed, &asked, None)[0].2, "发布前检查");
        let mut scanned = previous.clone();
        scanned.agent_sessions[0].terminal = None;
        assert!(
            entered(&before, &scanned, None).is_empty(),
            "a session Keepline did not start never asks for attention"
        );
        let value: serde_json::Value =
            serde_json::from_str(&panel(&scanned, &Link::fresh(RecognitionPhase::Healthy)))
                .unwrap();
        assert!(value["rows"].as_array().unwrap().is_empty());
        assert_eq!(value["badge"], "K");
    }
    /// A managed session in space "工作空间" whose content is `turn`, updated `ago` seconds ago.
    fn session(w: &mut Workspace, id: &str, turn: keepline_core::content::Turn, ago: u64) {
        use keepline_core::content::{Content, Turn};
        if w.spaces.is_empty() {
            w.spaces.push(Space {
                id: "space".into(),
                name: "工作空间".into(),
                cwd: "/project".into(),
            });
        }
        w.terminals.push(Terminal {
            id: format!("pty-{id}"),
            space: "space".into(),
            argv: vec![],
            cwd: "/project".into(),
            pid: None,
            state: ProcessState::Running,
            exit_code: None,
            error: None,
        });
        w.agent_sessions.push(AgentSession {
            id: id.into(),
            runtime: Agent::Claude,
            cli_session_id: format!("cli-{id}"),
            terminal: Some(format!("pty-{id}")),
            current: true,
            task: None,
            outcome: Outcome::Open,
            updated: now() - ago,
            cwd: "/project".into(),
            transcript: "log".into(),
            binding: None,
            name: format!("会话 {id}"),
        });
        w.content.insert(
            id.into(),
            Content {
                interactive: true,
                turn,
                waiting: if turn == Turn::Waiting {
                    format!("{id} 问你：继续吗？")
                } else {
                    String::new()
                },
                error: if turn == Turn::Error {
                    "exit 255".into()
                } else {
                    String::new()
                },
                updated: now() - ago,
                turn_started: now() - ago,
                ..Default::default()
            },
        );
    }
    fn value(w: &Workspace, link: &Link) -> serde_json::Value {
        serde_json::from_str(&panel(w, link)).unwrap()
    }
    /// The three states of menubar-panel.html: some need you, none do, the host is gone.
    fn samples() -> Vec<(&'static str, Workspace, Link)> {
        use keepline_core::content::Turn;
        let mut needs = Workspace::default();
        needs.settings.notifications = true;
        session(&mut needs, "a", Turn::Waiting, 120);
        session(&mut needs, "b", Turn::Waiting, 48 * 60);
        session(&mut needs, "c", Turn::Error, 2 * 3600);
        session(&mut needs, "d", Turn::Running, 30);
        let mut none = Workspace::default();
        none.settings.notifications = true;
        session(&mut none, "d", Turn::Running, 30);
        session(&mut none, "e", Turn::Running, 60);
        let offline = Link {
            phase: RecognitionPhase::Interrupted,
            error: Some("Connection refused (os error 61)".into()),
            since: now() - 1800,
        };
        let mut many = needs.clone();
        for i in 0..4 {
            session(&mut many, &format!("m{i}"), Turn::Waiting, 300 * (i + 1));
        }
        vec![
            (
                "needs",
                needs.clone(),
                Link::fresh(RecognitionPhase::Healthy),
            ),
            ("empty", none, Link::fresh(RecognitionPhase::Healthy)),
            ("offline", needs, offline),
            ("many", many, Link::fresh(RecognitionPhase::Healthy)),
        ]
    }
    #[test]
    fn panel_counts_rows_and_says_where_each_one_runs() {
        let [(_, needs, link), ..] = &samples()[..] else {
            unreachable!()
        };
        let v = value(needs, link);
        assert_eq!(v["heading"], "3 项需要你");
        assert_eq!(v["when"], "刚刚更新");
        assert_eq!(v["badge"], "K 3");
        assert_eq!(v["needs"], 3);
        let counts = &v["counts"];
        assert_eq!(
            (
                &counts["waiting"],
                &counts["errors"],
                &counts["running"],
                &counts["review"]
            ),
            (&2.into(), &1.into(), &1.into(), &0.into()),
            "numbers, not phrases the panel has to parse"
        );
        let rows = v["rows"].as_array().unwrap();
        let order: Vec<_> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(order, ["a", "b", "c"], "waiting first, newest first");
        assert_eq!(rows[0]["kind"], "wait");
        assert_eq!(rows[2]["kind"], "error");
        assert_eq!(rows[0]["meta"], "工作空间 · Claude Code");
        assert_eq!(rows[1]["age"], "48 分钟");
        assert_eq!(v["more"], 0);
        assert!(v["empty"].is_null());
        assert_eq!(v["connection"]["phase"], "healthy");
        assert!(v["connection"]["error"].is_null());
        assert_eq!(v["foot_count"], 0);
        assert_eq!(v["tokens"]["dark"]["accent"], T::MENU_DARK.accent);
    }
    #[test]
    fn panel_lists_five_and_counts_the_rest() {
        use keepline_core::content::Turn;
        let mut w = Workspace::default();
        for i in 0..7 {
            session(&mut w, &format!("s{i}"), Turn::Waiting, 60 * (i + 1));
        }
        let v = value(&w, &Link::fresh(RecognitionPhase::Healthy));
        assert_eq!(v["needs"], 7);
        assert_eq!(v["rows"].as_array().unwrap().len(), 5);
        assert_eq!(v["more"], 2);
    }
    #[test]
    fn empty_panel_says_whether_notifications_will_tell_you() {
        let samples = samples();
        let (_, none, link) = &samples[1];
        let v = value(none, link);
        assert_eq!(v["heading"], "没有需要你的");
        assert_eq!(v["badge"], "K");
        assert!(v["rows"].as_array().unwrap().is_empty());
        assert_eq!(v["empty"]["title"], "在跑的 2 个停下来要你时，会发系统通知");
        assert_eq!(v["empty"]["note"], "通知已开 · 你正看着的画面不会再弹");
        let mut quiet = none.clone();
        quiet.settings.notifications = false;
        let v = value(&quiet, link);
        assert_eq!(v["empty"]["note"], "通知没开 · 在设置的通知页打开");
        assert!(!v["empty"]["title"].as_str().unwrap().contains("系统通知"));
    }
    #[test]
    fn offline_panel_keeps_the_old_rows_and_dates_them() {
        let samples = samples();
        let (_, needs, link) = &samples[2];
        let v = value(needs, link);
        let stopped = minute(link.since);
        assert_eq!(v["heading"], "连不上后台");
        assert_eq!(v["when"], format!("数据停在 {stopped}"));
        assert_eq!(v["section"], format!("{stopped} 时需要你"));
        assert_eq!(v["section_note"], "可能已经变了");
        assert_eq!(v["badge"], "K", "the offline badge carries no count");
        assert_eq!(v["rows"].as_array().unwrap().len(), 3);
        let c = &v["connection"];
        assert_eq!(c["phase"], "interrupted");
        assert_eq!(c["error"], "Connection refused (os error 61)");
        assert_eq!(c["since"], link.since);
        assert_eq!(v["foot_note"], "通知暂停");
        assert!(v["foot_count"].is_null());
        assert!(
            v["empty"].is_null(),
            "no reassurance while the numbers are old"
        );
    }
    /// Writes each sample's panel JSON for a native snapshot of the AppKit panel:
    /// `KEEPLINE_PANEL_SAMPLES=dir cargo test -p keepline-app panel_samples -- --ignored`.
    #[test]
    #[ignore]
    fn panel_samples() {
        let dir = std::path::PathBuf::from(std::env::var_os("KEEPLINE_PANEL_SAMPLES").unwrap());
        std::fs::create_dir_all(&dir).unwrap();
        for (name, w, link) in samples() {
            std::fs::write(dir.join(format!("{name}.json")), panel(&w, &link)).unwrap();
        }
    }
}

#[cfg(feature = "visual-test")]
pub fn snapshot(path: &Path) {
    unsafe extern "C" {
        fn kl_desktop_snapshot(path: *const c_char) -> bool;
    }
    assert!(
        unsafe { kl_desktop_snapshot(cstring(&path.to_string_lossy()).as_ptr()) },
        "native menu panel export"
    );
}
