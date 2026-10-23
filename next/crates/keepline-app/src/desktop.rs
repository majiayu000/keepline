//! Event-driven macOS status item and notification delivery; no timers or process probes.
use crate::{connection::RecognitionConnection, theme::Theme as T};
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
    fn kl_desktop_notify(session: *const c_char, title: *const c_char, body: *const c_char);
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
    Some((
        format!("{:?}:{}:{}:{}", group, c.turn_key, c.turn_started, body),
        format!("{} · {}", group.label(), short(&body, 180)),
    ))
}
fn entered(
    before: &Workspace,
    after: &Workspace,
    watching: Option<&str>,
) -> Vec<(String, String, String, String)> {
    if !after.settings.notifications {
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
            let title = after
                .content
                .get(&s.id)
                .map_or_else(|| s.runtime.executable().into(), |c| short(&c.title, 72));
            // Name the project first: several panes often run the same kind of task.
            let space = after
                .terminals
                .iter()
                .find(|t| Some(&t.id) == s.terminal.as_ref())
                .and_then(|t| after.spaces.iter().find(|p| p.id == t.space))
                .map(|p| p.name.clone());
            let title = match space {
                Some(space) => format!("{space} · {title}"),
                None => title,
            };
            Some((s.id.clone(), key, title, body))
        })
        .collect()
}
fn panel(w: &Workspace) -> String {
    let mut rows = Vec::new();
    let mut review = 0;
    let mut running = 0;
    let mut stale = 0;
    let mut waiting = 0;
    let mut errors = 0;
    for s in &w.agent_sessions {
        let Some(c) = w.content.get(&s.id) else {
            continue;
        };
        if c.hidden_for(s).is_some() {
            continue;
        }
        let group = c.group(s, w, now());
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
            let minutes = now().saturating_sub(c.updated) / 60;
            rows.push((group == Inbox::Error, json!({
                "id":s.id,"title":short(&c.title,24),
                "summary":short(if group==Inbox::Waiting {&c.waiting}else{&c.error},40),
                "age":if minutes<60 {format!("{minutes} 分钟")}else{format!("{} 小时",minutes/60)},
                "dot":if group==Inbox::Waiting {T::MENU_WAIT_DOT}else{T::MENU_ERROR_DOT},
                "ink":if group==Inbox::Waiting {T::MENU_WAIT_TEXT}else{T::MENU_ERROR_TEXT}
            })));
        }
    }
    rows.sort_by_key(|r| r.0);
    let total = rows.len();
    json!({"badge":if total==0 {"K".into()}else{format!("K {total}")},"heading":format!("{waiting} 个在等你 · {errors} 个出错"),"summary":format!("另有 {review} 个待核对 · {running} 个运行中"),"footer":format!("很久没动 · {stale} 个{}\n等你、出错只计 Keepline 里的画面；外部会话在鸟瞰里看",if total>5 {format!("\n还有 {} 项需要你，请打开工作台",total-5)}else{String::new()}),"rows":rows.into_iter().take(5).map(|r|r.1).collect::<Vec<_>>(),"tokens":{"width":T::MENUBAR_WIDTH,"height":T::MENUBAR_HEIGHT,"padding":T::S16,"gap":T::S12,"row":T::MENUBAR_ROW,"font":T::BODY_SIZE,"title":T::MENU_TITLE,"small":T::SMALL,"meta":T::META,"inset":T::S12,"button":T::MENU_BUTTON,"radius":T::CARD_RADIUS,"selected":T::MENU_WAIT_BG,"accent":T::ACCENT,"white":T::WHITE,"background":T::WHITE,"text":T::MENU_TEXT,"secondary":T::MENU_SECONDARY}}).to_string()
}
pub fn show() {
    unsafe {
        kl_desktop_show();
    }
}
pub fn start(socket: &Path, initial: Workspace, cx: &mut App) -> UnboundedReceiver<String> {
    let (tx, events) = unbounded();
    let _ = EVENTS.set(tx);
    cx.set_global(Status::default());
    // All AppKit entry points run on the GPUI/main thread.
    unsafe {
        kl_desktop_install(event);
        kl_desktop_update(cstring(&panel(&initial)).as_ptr());
    }
    let (status_tx, mut status_rx) = unbounded();
    let connection = RecognitionConnection::new(socket, status_tx);
    let socket = socket.to_owned();
    cx.spawn(async move |cx| {
        let _connection = connection;
        let mut before = initial;
        let mut ready = false;
        let mut previous_panel = panel(&before);
        while let Some(status) = status_rx.next().await {
            let socket_read = socket.clone();
            let result = cx
                .background_spawn(async move { rpc(&socket_read, &Request::Workspace) })
                .await;
            let after = match result {
                Ok(Reply::Workspace(w)) => w,
                Err(e) => {
                    cx.update(|cx| cx.set_global(Status(format!("菜单栏连接失败：{e}"))));
                    continue;
                }
                _ => continue,
            };
            let value = panel(&after);
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
                            cx.update(|_| unsafe {
                                kl_desktop_notify(
                                    cstring(&session).as_ptr(),
                                    cstring(&title).as_ptr(),
                                    cstring(&body).as_ptr(),
                                );
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
        w.settings.notifications = true;
        assert_eq!(entered(&before, &w, None).len(), 1);
        assert!(entered(&w, &w, None).is_empty());
        let previous = w.clone();
        w.content.get_mut("s").unwrap().waiting = "是否执行测试？".into();
        assert_eq!(entered(&previous, &w, None).len(), 1);
        assert!(
            entered(&previous, &w, Some("pty")).is_empty(),
            "no banner for the pane the user is looking at"
        );
        assert_eq!(entered(&previous, &w, Some("other")).len(), 1);
        w.agent_sessions[0].outcome = Outcome::Completed;
        assert!(entered(&previous, &w, None).is_empty());
        let value: serde_json::Value = serde_json::from_str(&panel(&previous)).unwrap();
        assert_eq!(value["rows"].as_array().unwrap().len(), 1);
        let mut scanned = previous.clone();
        scanned.agent_sessions[0].terminal = None;
        assert!(
            entered(&before, &scanned, None).is_empty(),
            "a session Keepline did not start never asks for attention"
        );
        let value: serde_json::Value = serde_json::from_str(&panel(&scanned)).unwrap();
        assert!(value["rows"].as_array().unwrap().is_empty());
        assert_eq!(value["badge"], "K");
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
