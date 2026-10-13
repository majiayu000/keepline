use std::{collections::HashSet, sync::Mutex, time::Duration};
use serde_json::{Value};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_shell::{ShellExt, process::{CommandChild, CommandEvent}};

#[derive(Default)]
pub struct ServiceState {
    pub child: Mutex<Option<CommandChild>>,
    pub connection: Mutex<Option<(String,String)>>,
    pending_open: Mutex<Option<LedgerTarget>>,
}
#[derive(Clone)]
struct LedgerTarget {
    session: Option<String>,
    anchor: Option<String>,
}
fn data_home() -> std::path::PathBuf {
    std::env::var_os("KEEPLINE_HOME").map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".keepline"))
}
fn service_port() -> u16 {
    std::fs::read(data_home().join("config.json")).ok().and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|v| v["webPort"].as_u64()).and_then(|p| u16::try_from(p).ok()).filter(|p| *p > 0).unwrap_or(3377)
}
fn client() -> Result<reqwest::Client,String> {
    reqwest::Client::builder().timeout(Duration::from_secs(3)).build().map_err(|e| e.to_string())
}
async fn compatible(http: &reqwest::Client, base: &str) -> Result<bool,String> {
    let health = match http.get(format!("{base}/api/v1/health")).send().await {
        Ok(response) => response,
        Err(error) if error.is_connect() => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    if !health.status().is_success() { return Err("Another service occupies the configured address".into()); }
    let meta: Value = http.get(format!("{base}/api/v1/meta")).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if meta["data"]["capabilities"].as_array().is_some_and(|caps| caps.iter().any(|v| v == "ledger")) { Ok(true) }
    else { Err("The running Keepline service does not support Progress Ledger. Update it before attaching.".into()) }
}
async fn connect(app: &AppHandle) -> Result<(String,String),String> {
    let http = client()?; let base = format!("http://127.0.0.1:{}",service_port());
    if !compatible(&http,&base).await? {
        let state = app.state::<ServiceState>();
        if let Some(old) = state.child.lock().map_err(|e| e.to_string())?.take() { let _ = old.kill(); }
        let resource = app.path().resource_dir().map_err(|e| e.to_string())?.join("web");
        let command = app.shell().sidecar("keepline-service").map_err(|e| e.to_string())?
            .args(["service","--port",&service_port().to_string(),"--scan-interval","5"])
            .env("KEEPLINE_WEB_DIST",resource).env("KEEPLINE_HOME",data_home());
        let (mut receiver, child) = command.spawn().map_err(|e| e.to_string())?;
        *state.child.lock().map_err(|e| e.to_string())? = Some(child);
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(event) = receiver.recv().await {
                if let CommandEvent::Stderr(bytes) = &event {
                    eprintln!("Keepline service: {}",String::from_utf8_lossy(bytes));
                }
                if matches!(event, CommandEvent::Terminated(_)) {
                    *handle.state::<ServiceState>().connection.lock().unwrap() = None;
                    break;
                }
            }
        });
        let mut ready = false;
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if compatible(&http,&base).await.unwrap_or(false) { ready = true; break; }
        }
        if !ready { return Err("Embedded service did not become healthy".into()); }
    }
    let response: Value = http.post(format!("{base}/api/v1/auth/local")).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let token = response["data"]["token"].as_str().ok_or("Local authentication failed")?.to_string();
    *app.state::<ServiceState>().connection.lock().map_err(|e| e.to_string())? = Some((base.clone(),token.clone()));
    Ok((base,token))
}
fn build_window<R: tauri::Runtime>(app: &AppHandle<R>,label: &str,path: &str,popover: bool) -> Result<(),String> {
    let (base,_) = app.state::<ServiceState>().connection.lock().map_err(|e| e.to_string())?.clone().ok_or("Service not ready")?;
    let url: tauri::Url = format!("{base}{path}").parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let origin = url.origin();
    let window = WebviewWindowBuilder::new(app,label,WebviewUrl::External(url))
        .title("Keepline").inner_size(if popover { 390.0 } else { 1180.0 },if popover { 550.0 } else { 800.0 })
        .resizable(!popover).decorations(!popover).always_on_top(popover).skip_taskbar(popover).visible(!popover)
        .on_navigation(move |url| url.origin() == origin)
        .on_page_load(|window, payload| {
            if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) { return; }
            let connection = window.state::<ServiceState>().connection.lock().ok().and_then(|value| value.clone());
            if let Some((base, token)) = connection {
                if tauri::Url::parse(&base).is_ok_and(|base| base.origin() == payload.url().origin()) {
                    // Read the current token on every load. A static initialization
                    // script would restore the expired token after service restart.
                    let script = format!("if(localStorage.getItem('terminal_token')!=={0}){{localStorage.setItem('terminal_token',{0});location.reload();}}", serde_json::json!(token));
                    if let Err(error) = window.eval(&script) { eprintln!("Could not refresh window authentication: {error}"); }
                }
            }
        })
        .build().map_err(|e| e.to_string())?;
    let cloned = window.clone(); let handle = app.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Focused(false) if popover => { let _ = cloned.hide(); },
        tauri::WindowEvent::CloseRequested { api, .. } if !popover => {
            api.prevent_close(); let _ = cloned.hide(); let _ = handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        },
        _ => {},
    });
    Ok(())
}
pub fn reopen_ledger_window<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(),String> {
    if app.state::<ServiceState>().pending_open.lock().map_err(|e| e.to_string())?.is_some() {
        return open_pending_ledger_window(app);
    }
    let Some(window) = app.get_webview_window("dashboard") else {
        return open_ledger_window(app,None,None);
    };
    // macOS reopens the app after dispatching a notification click. Keep the
    // session/anchor selected by that callback instead of navigating to overview.
    app.set_activation_policy(tauri::ActivationPolicy::Regular).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}
pub fn open_ledger_window<R: tauri::Runtime>(app: &AppHandle<R>,session: Option<&str>,anchor: Option<&str>) -> Result<(),String> {
    // An explicit user request, including the menu's overview action, replaces
    // the previous target. Startup/Reopen only replay it, on the main thread.
    *app.state::<ServiceState>().pending_open.lock().map_err(|e| e.to_string())? =
        Some(LedgerTarget { session: session.map(str::to_owned),anchor: anchor.map(str::to_owned) });
    open_pending_ledger_window(app)
}
fn open_pending_ledger_window<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<(),String> {
    let state = app.state::<ServiceState>();
    let Some(target) = state.pending_open.lock().map_err(|e| e.to_string())?.clone() else {
        return Ok(());
    };
    let Some(base) = state.connection.lock().map_err(|e| e.to_string())?.as_ref().map(|v| v.0.clone()) else {
        return Ok(());
    };
    let url = ledger_url(&base,target.session.as_deref(),target.anchor.as_deref())?;
    app.set_activation_policy(tauri::ActivationPolicy::Regular).map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window("dashboard") {
        window.navigate(url).map_err(|e| e.to_string())?;
    } else { build_window(app,"dashboard",url.as_str().strip_prefix(&base).unwrap_or("/"),false)?; }
    if let Some(window) = app.get_webview_window("dashboard") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    // Connection or window errors above leave the request available for retry.
    *state.pending_open.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}
fn ledger_url(base: &str,session: Option<&str>,anchor: Option<&str>) -> Result<tauri::Url,String> {
    let mut url = base.parse::<tauri::Url>().map_err(|e| e.to_string())?;
    if let Some(session) = session { url.query_pairs_mut().append_pair("sessionId",session).append_pair("anchor",anchor.unwrap_or("current-step")); }
    Ok(url)
}
#[tauri::command]
pub fn open_ledger(app: AppHandle,session_id: Option<String>,anchor: Option<String>) -> Result<(),String> { open_ledger_window(&app,session_id.as_deref(),anchor.as_deref()) }
#[tauri::command]
pub fn get_app_autostart(app: AppHandle) -> Result<bool,String> { app.autolaunch().is_enabled().map_err(|e| e.to_string()) }
#[tauri::command]
pub fn set_app_autostart(app: AppHandle,enabled: bool) -> Result<(),String> {
    (if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() }).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(data_home()).map_err(|e| e.to_string())?;
    std::fs::write(data_home().join("app-autostart.json"),if enabled { "true" } else { "false" }).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn quit_app(app: AppHandle,stop_monitoring: bool) -> Result<(),String> {
    stop_owned_service(&app.state::<ServiceState>().child,stop_monitoring)?;
    app.exit(0); Ok(())
}
fn stop_owned_service(child: &Mutex<Option<CommandChild>>,stop_monitoring: bool) -> Result<(),String> {
    if stop_monitoring {
        if let Some(child) = child.lock().map_err(|e| e.to_string())?.take() { child.kill().map_err(|e| e.to_string())?; }
    }
    Ok(())
}
pub fn request_quit(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.eval("window.dispatchEvent(new Event('keepline:quit'))");
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // A failed connection can leave no webview to present the choice in.
        app.exit(0);
    }
}
fn service_capability(port: u16) -> tauri::ipc::CapabilityBuilder {
    // Remote IPC is scoped to the configured service origin and the five ledger commands.
    // The compiled app manifest denies other custom commands by default.
    tauri::ipc::CapabilityBuilder::new("ledger-service")
        .local(false).windows(["main", "dashboard"])
        .remote(format!("http://127.0.0.1:{port}/*"))
        .permission("core:default").permission("allow-open-ledger")
        .permission("allow-set-ledger-counts").permission("allow-get-app-autostart")
        .permission("allow-set-app-autostart").permission("allow-quit-app")
}
fn connected_windows<R: tauri::Runtime>(app: &AppHandle<R>,background: bool) {
    // Keep existing window identities, visibility and deep links. Destroying
    // and rebuilding the same label races Tauri's asynchronous teardown.
    for label in ["main","dashboard"] {
        if let Some(window) = app.get_webview_window(label) {
            if let Err(error) = window.reload() { eprintln!("Could not reload reconnected window: {error}"); }
        }
    }
    let initial_start = app.get_webview_window("main").is_none();
    if initial_start {
        if let Err(e) = build_window(app,"main","/menubar",true) { eprintln!("Could not open menubar: {e}"); }
    }
    let pending = app.state::<ServiceState>().pending_open.lock().unwrap().is_some();
    if pending || (initial_start && !background && app.get_webview_window("dashboard").is_none()) {
        if let Err(e) = reopen_ledger_window(app) { eprintln!("Could not open Keepline: {e}"); }
    }
}
pub fn start(app: AppHandle) {
    if let Err(error) = app.add_capability(service_capability(service_port())) { eprintln!("Could not grant service IPC: {error}"); return; }
    tauri::async_runtime::spawn(async move {
        let http = match client() { Ok(http) => http,Err(error) => { eprintln!("{error}"); return; } };
        let mut notifications = HashSet::new();
        loop {
            let connection = app.state::<ServiceState>().connection.lock().unwrap().clone();
            let (base,token) = match connection {
                Some(connection) => connection,
                None => match connect(&app).await {
                    Ok(connection) => {
                        let handle = app.clone();
                        if let Err(error) = app.run_on_main_thread(move || {
                            connected_windows(&handle,std::env::args().any(|arg| arg == "--background"));
                        }) { eprintln!("Could not schedule connected windows: {error}"); }
                        connection
                    },
                    Err(error) => { eprintln!("Keepline service unavailable: {error}"); tokio::time::sleep(Duration::from_secs(5)).await; continue; },
                },
            };
            if http.get(format!("{base}/api/v1/health")).send().await.is_err() { *app.state::<ServiceState>().connection.lock().unwrap() = None; continue; }
            if let Ok(response) = http.post(format!("{base}/api/ledger/native-channel")).bearer_auth(&token).send().await {
                if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                    *app.state::<ServiceState>().connection.lock().unwrap() = None;
                    continue;
                }
            }
            if let Ok(response) = http.get(format!("{base}/api/ledger/notifications")).bearer_auth(&token).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    if let Some(alerts) = data["data"].as_array() {
                        for alert in alerts {
                            let Some(id) = alert["id"].as_str() else { continue; };
                            let session = alert["sessionId"].as_str().unwrap_or("");
                            let kind = alert["kind"].as_str().unwrap_or("");
                            let sound = kind == "needs_input";
                            let anchor = if kind == "off_plan" { "off-plan" } else if kind == "claimed_unverified" { "requirements" } else { "current-step" };
                            let delivered = if notifications.contains(id) { true } else {
                                let (id,body,session,anchor) = (id.to_string(),alert["detail"].as_str().unwrap_or("").to_string(),session.to_string(),anchor.to_string());
                                matches!(tauri::async_runtime::spawn_blocking(move || crate::notifications::show(&id,&body,&session,&anchor,sound)).await,Ok(Ok(())))
                            };
                            if delivered {
                                let ids: Vec<&str> = alert["bundledIds"].as_array().map(|ids| ids.iter().filter_map(Value::as_str).collect()).unwrap_or_else(|| vec![id]);
                                for delivered_id in ids {
                                    notifications.insert(delivered_id.to_string());
                                    let _ = http.post(format!("{base}/api/ledger/notifications/{delivered_id}/delivered")).bearer_auth(&token).send().await;
                                }
                            }
                        }
                    }
                }
            }
            // Withdraw alerts that no longer exist in the service's active list.
            if let Ok(response) = http.get(format!("{base}/api/ledger/active-alerts")).bearer_auth(&token).send().await {
                if let Ok(data) = response.json::<Value>().await {
                    let active: HashSet<String> = data["data"].as_array().map(|a| a.iter().filter_map(|v| v["id"].as_str().map(str::to_string)).collect()).unwrap_or_default();
                    notifications.retain(|id| { if active.contains(id) { true } else { crate::notifications::clear(id); false } });
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
}

#[tauri::command]
pub fn set_ledger_counts(app: AppHandle,needs_you: usize,running: usize) -> Result<(),String> {
    if let Some(tray) = app.tray_by_id("quota-tray") {
        tray.set_title(Some(format!("{needs_you} ! · {running} ▶"))).map_err(|e| e.to_string())?;
        tray.set_tooltip(Some(format!("Keepline：{needs_you} 项需要你，{running} 项正在执行"))).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ledger_url,service_capability,stop_owned_service,ServiceState,open_ledger_window,reopen_ledger_window,connected_windows};
    use std::{sync::Mutex,time::Duration};
    use tauri::{Manager,WebviewUrl};
    use tauri_plugin_shell::{ShellExt,process::CommandEvent};
    #[tauri::command]
    fn get_app_autostart() -> bool { true }
    #[tauri::command]
    fn get_codex_info() -> bool { true }
    #[test]
    fn ipc_denies_other_loopback_ports_and_ungranted_commands() {
        let app = tauri::test::mock_builder().invoke_handler(tauri::generate_handler![self::get_app_autostart,self::get_codex_info])
            .build(crate::app_context()).unwrap();
        app.add_capability(service_capability(3377)).unwrap();
        let webview = tauri::WebviewWindowBuilder::new(&app,"dashboard",WebviewUrl::External("http://127.0.0.1:3377/".parse().unwrap())).build().unwrap();
        let invoke = |origin: &str,cmd: &str| tauri::test::get_ipc_response(&webview,tauri::webview::InvokeRequest {
            cmd: cmd.into(),callback: tauri::ipc::CallbackFn(0),error: tauri::ipc::CallbackFn(1),
            url: origin.parse().unwrap(),body: tauri::ipc::InvokeBody::default(),headers: Default::default(),invoke_key: tauri::test::INVOKE_KEY.into(),
        });
        assert!(invoke("http://127.0.0.1:3377/","get_app_autostart").is_ok());
        assert!(invoke("http://127.0.0.1:5572/","get_app_autostart").is_err());
        assert!(invoke("http://evil.test/","get_app_autostart").is_err());
        assert!(invoke("http://127.0.0.1:3377/","get_codex_info").is_err());
    }
    #[test]
    fn notification_target_preserves_origin_and_encodes_session() {
        let url = ledger_url("http://127.0.0.1:3377",Some("session/?#&"),Some("off-plan")).unwrap();
        assert_eq!(url.origin().ascii_serialization(),"http://127.0.0.1:3377");
        let params: std::collections::HashMap<_,_> = url.query_pairs().into_owned().collect();
        assert_eq!(params["sessionId"],"session/?#&"); assert_eq!(params["anchor"],"off-plan");
        assert!(ledger_url("http://127.0.0.1:3377",Some("s"),None).unwrap().as_str().contains("anchor=current-step"));
    }
    fn window_app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder().manage(ServiceState::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets())).unwrap()
    }
    fn set_connected(app: &tauri::App<tauri::test::MockRuntime>) {
        *app.state::<ServiceState>().connection.lock().unwrap() = Some(("http://127.0.0.1:3377".into(),"test-token".into()));
    }
    fn assert_target(app: &tauri::App<tauri::test::MockRuntime>,session: Option<&str>,anchor: Option<&str>) {
        assert_eq!(app.get_webview_window("dashboard").unwrap().url().unwrap(),ledger_url("http://127.0.0.1:3377",session,anchor).unwrap());
    }
    #[test]
    fn notification_before_connection_survives_reopen_and_startup() {
        for background in [false,true] {
            let app = window_app();
            open_ledger_window(app.handle(),Some("cold-session"),Some("requirements")).unwrap();
            reopen_ledger_window(app.handle()).unwrap();
            assert!(app.get_webview_window("dashboard").is_none());
            set_connected(&app);
            connected_windows(app.handle(),background);
            reopen_ledger_window(app.handle()).unwrap();
            assert_target(&app,Some("cold-session"),Some("requirements"));
        }
    }
    #[test]
    fn connected_notification_before_startup_is_not_overwritten() {
        let app = window_app();
        set_connected(&app);
        open_ledger_window(app.handle(),Some("early-session"),Some("off-plan")).unwrap();
        connected_windows(app.handle(),false);
        reopen_ledger_window(app.handle()).unwrap();
        assert_target(&app,Some("early-session"),Some("off-plan"));
    }
    #[test]
    fn notification_survives_connection_retries_and_latest_click_wins() {
        let app = window_app();
        set_connected(&app);
        connected_windows(app.handle(),false);
        open_ledger_window(app.handle(),Some("old-session"),None).unwrap();
        *app.state::<ServiceState>().connection.lock().unwrap() = None;
        open_ledger_window(app.handle(),Some("first-click"),Some("off-plan")).unwrap();
        // Failed attempts leave the service disconnected; Reopen must not consume
        // or replace the pending target even with an existing dashboard.
        for _ in 0..3 { reopen_ledger_window(app.handle()).unwrap(); }
        open_ledger_window(app.handle(),Some("latest-click"),Some("requirements")).unwrap();
        reopen_ledger_window(app.handle()).unwrap();
        set_connected(&app);
        connected_windows(app.handle(),false);
        assert_target(&app,Some("latest-click"),Some("requirements"));
        // Once consumed, a target must not hijack later explicit overview opens.
        open_ledger_window(app.handle(),None,None).unwrap();
        assert_target(&app,None,None);
        connected_windows(app.handle(),false);
        assert_target(&app,None,None);
    }
    #[test]
    fn reopen_before_notification_and_before_connection_opens_requested_target() {
        let app = window_app();
        reopen_ledger_window(app.handle()).unwrap();
        open_ledger_window(app.handle(),Some("clicked-session"),None).unwrap();
        set_connected(&app);
        // Connection can be published before the startup main-thread callback.
        reopen_ledger_window(app.handle()).unwrap();
        connected_windows(app.handle(),false);
        assert_target(&app,Some("clicked-session"),None);
    }
    #[test]
    fn ordinary_reopen_before_connection_overrides_background_startup() {
        let app = window_app();
        reopen_ledger_window(app.handle()).unwrap();
        set_connected(&app);
        connected_windows(app.handle(),true);
        assert_target(&app,None,None);
    }
    #[test]
    fn explicit_overview_replaces_pending_notification() {
        let app = window_app();
        set_connected(&app);
        connected_windows(app.handle(),false);
        *app.state::<ServiceState>().connection.lock().unwrap() = None;
        open_ledger_window(app.handle(),Some("superseded-click"),Some("off-plan")).unwrap();
        // The menu's "Open dashboard" is an explicit user choice, unlike Reopen.
        open_ledger_window(app.handle(),None,None).unwrap();
        set_connected(&app);
        connected_windows(app.handle(),false);
        assert_target(&app,None,None);
    }
    #[test]
    fn ordinary_startup_and_background_reopen() {
        for background in [false,true] {
            let app = window_app();
            set_connected(&app);
            connected_windows(app.handle(),background);
            assert!(app.get_webview_window("main").is_some());
            assert_eq!(app.get_webview_window("dashboard").is_none(),background);
            reopen_ledger_window(app.handle()).unwrap();
            assert_target(&app,None,None);
        }
    }
    #[tokio::test]
    async fn quitting_stops_only_owned_child_and_keep_choice_leaves_it_running() {
        let app = tauri::test::mock_builder().plugin(tauri_plugin_shell::init())
            .build(tauri::test::mock_context(tauri::test::noop_assets())).unwrap();
        let (mut events,owned) = app.shell().command("/bin/sleep").arg("30").spawn().unwrap();
        let child = Mutex::new(Some(owned));
        let mut attached = std::process::Command::new("/bin/sleep").arg("30").spawn().unwrap();
        stop_owned_service(&child,false).unwrap();
        assert!(child.lock().unwrap().is_some()); assert!(attached.try_wait().unwrap().is_none());
        stop_owned_service(&child,true).unwrap();
        assert!(child.lock().unwrap().is_none());
        tokio::time::timeout(Duration::from_secs(2),async {
            while let Some(event) = events.recv().await { if matches!(event,CommandEvent::Terminated(_)) { return; } }
            panic!("owned child did not terminate");
        }).await.unwrap();
        stop_owned_service(&Mutex::new(None),true).unwrap();
        assert!(attached.try_wait().unwrap().is_none());
        attached.kill().unwrap(); attached.wait().unwrap();
    }
}
