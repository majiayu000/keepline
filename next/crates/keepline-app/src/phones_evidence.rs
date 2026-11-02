//! KEEPLINE_PHONES_EVIDENCE: the title bar's phone button and its panel with a real relay
//! and a real phone client on it. KEEPLINE_RELAY_BIN and KEEPLINE_PHONE_BIN name the
//! `keepline-relay` binary and its `phone` example.
use super::*;
use gpui::WindowHandle;
use std::io::{BufRead, Write};
use std::time::Duration;

pub(super) fn start(
    handle: Option<WindowHandle<workbench::Workbench>>,
    evidence: Option<PathBuf>,
    cx: &mut App,
) -> bool {
    let (Some(handle), Some(directory)) = (handle, evidence) else {
        return false;
    };
    if std::env::var_os("KEEPLINE_PHONES_EVIDENCE").is_none() {
        return false;
    }
    let relay_bin = std::env::var("KEEPLINE_RELAY_BIN").expect("KEEPLINE_RELAY_BIN");
    let phone_bin = std::env::var("KEEPLINE_PHONE_BIN").expect("KEEPLINE_PHONE_BIN");
    cx.spawn(async move |cx| {
        let executor = cx.background_executor().clone();
        let wait = move |ms| executor.timer(Duration::from_millis(ms));
        wait(1500).await;
        std::fs::create_dir_all(&directory).unwrap();
        let socket = handle.update(cx, |v, _, _| v.visual_socket()).unwrap();
        let call = |r: Request| rpc(&socket, &r).unwrap();
        let workspace = || match call(Request::Workspace) {
            Reply::Workspace(w) => w,
            x => panic!("{x:?}"),
        };
        let w = workspace();
        let space = match w.spaces.first() {
            Some(s) => s.id.clone(),
            None => {
                call(Request::Edit {
                    revision: w.revision,
                    edit: Edit::AddSpace(Space { id: "s".into(), name: "keepline".into(), cwd: "/tmp".into() }),
                });
                "s".into()
            }
        };
        let launch = |tid: &str, script: &str| {
            call(Request::Launch {
                id: tid.into(),
                space: space.clone(),
                argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
                cwd: "/tmp".into(),
                cols: 100,
                rows: 30,
                scheme: Default::default(),
            });
            let w = workspace();
            call(Request::Edit {
                revision: w.revision,
                edit: Edit::PutTab {
                    tab: Tab { id: format!("tab-{tid}"), space: space.clone(), default_task: None, title: tid.into(), layout: Some(Layout::Leaf(tid.into())) },
                },
            });
        };
        launch("keepline-relay", "printf '$ cargo test -p keepline-relay\\nrunning 4 tests\\ntest a_phone_reaches_its_machine ... ok\\n'; stty raw -echo; exec cat");

        // A real relay on a free local port.
        let mut relay = std::process::Command::new(&relay_bin)
            .args(["--listen", "127.0.0.1:0"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        std::io::BufReader::new(relay.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let addr = line.split_whitespace().nth(3).unwrap().to_owned();
        let base = format!("http://{addr}");
        call(Request::RemoteConfigure { enabled: true, relay: base.clone(), key: None });
        let pair = |name: &str, role: &str| match call(Request::RemotePair { name: name.into(), role: role.into() }) {
            Reply::Text(link) => link,
            x => panic!("{x:?}"),
        };
        let iphone = pair("iPhone", "full");
        let _ipad = pair("iPad", "view");
        let remote = |cx: &mut gpui::AsyncApp| handle.update(cx, |v, _, _| v.visual_remote()).unwrap();
        for _ in 0..100 {
            wait(100).await;
            if remote(cx).live.state == keepline_core::protocol::RemoteState::Connected { break; }
        }
        // The phone client watches the terminal and presses a key, as a phone would.
        let mut phone = std::process::Command::new(&phone_bin)
            .arg(&iphone)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut say = phone.stdin.take().unwrap();
        for line in [
            r#"{"id":1,"op":"hello"}"#,
            r#"{"id":2,"op":"watch","terminal":"keepline-relay"}"#,
            r#"{"id":3,"op":"keys","terminal":"keepline-relay","keys":["up","enter"]}"#,
        ] {
            writeln!(say, "{line}").unwrap();
        }
        let mut seen = false;
        for _ in 0..100 {
            wait(100).await;
            let r = remote(cx);
            if r.live.online.first().is_some_and(|p| !p.watching.is_empty() && p.last.is_some()) {
                seen = true;
                break;
            }
        }
        assert!(seen, "the phone never showed: {:?}", remote(cx));
        let shot = |name: &str, cx: &mut gpui::AsyncApp| {
            cx.update_window(handle.into(), |_, w, a| {
                w.refresh();
                w.draw(a).clear(a);
                w.render_to_image().unwrap().save(directory.join(name)).unwrap();
            })
            .unwrap();
        };
        let both = async |name: &str, cx: &mut gpui::AsyncApp| {
            wait(300).await;
            shot(&format!("{name}-light.png"), cx);
            cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Dark, a).unwrap());
            wait(300).await;
            shot(&format!("{name}-dark.png"), cx);
            cx.update(|a| crate::ui::appearance::choose(crate::ui::appearance::Appearance::Light, a).unwrap());
        };
        shot("0-button.png", cx);
        handle.update(cx, |v, _, a| v.visual_phones(true, None, a)).unwrap();
        both("1-panel", cx).await;
        let ipad = remote(cx).devices.iter().find(|d| d.name == "iPad").unwrap().id.clone();
        handle.update(cx, |v, _, a| v.visual_phones(true, Some(ipad.clone()), a)).unwrap();
        both("2-more", cx).await;
        handle.update(cx, |v, w, a| v.visual_phones_pair("Pixel", "answer", w, a)).unwrap();
        for _ in 0..50 {
            wait(100).await;
            if handle.update(cx, |v, _, _| v.visual_phones_paired()).unwrap() { break; }
        }
        both("3-qr", cx).await;
        handle.update(cx, |v, w, a| v.visual_phones_settings(w, a)).unwrap();
        both("4-settings", cx).await;
        // 断开 from the panel ends the phone's session; it stays paired.
        let id = remote(cx).live.online[0].device.clone();
        handle.update(cx, |v, _, a| v.visual_phones_kick(&id, a)).unwrap();
        let mut gone = false;
        for _ in 0..50 {
            wait(100).await;
            if remote(cx).live.online.is_empty() { gone = true; break; }
        }
        assert!(gone, "断开 did not end the session");
        assert_eq!(remote(cx).devices.len(), 3);
        handle.update(cx, |v, _, a| v.visual_phones(true, None, a)).unwrap();
        both("5-after-kick", cx).await;
        let _ = phone.kill();
        call(Request::RemoteConfigure { enabled: false, relay: base, key: None });
        let _ = relay.kill();
        eprintln!("phones evidence: ok");
        cx.update(|cx| cx.quit());
    })
    .detach();
    true
}
