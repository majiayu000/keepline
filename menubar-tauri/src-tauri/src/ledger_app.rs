use std::{collections::HashSet, sync::Mutex, time::Duration};
use serde_json::{Value};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_shell::{ShellExt, process::{CommandChild, CommandEvent}};

pub struct ServiceState {
    pub child: Mutex<Option<CommandChild>>,
    pub connection: Mutex<Option<(String,String)>>,
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
fn build_window(app: &AppHandle,label: &str,path: &str,popover: bool) -> Result<(),String> {
    let (base,token) = app.state::<ServiceState>().connection.lock().map_err(|e| e.to_string())?.clone().ok_or("Service not ready")?;
    let url: tauri::Url = format!("{base}{path}").parse::<tauri::Url>().map_err(|e| e.to_string())?;
    let script = format!("localStorage.setItem('terminal_token',{});",serde_json::to_string(&token).map_err(|e| e.to_string())?);
    let origin = url.origin();
    let window = WebviewWindowBuilder::new(app,label,WebviewUrl::External(url))
        .title("Keepline").inner_size(if popover { 390.0 } else { 1180.0 },if popover { 550.0 } else { 800.0 })
        .resizable(!popover).decorations(!popover).always_on_top(popover).skip_taskbar(popover).visible(!popover)
        .initialization_script(&script).on_navigation(move |url| url.origin() == origin)
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
pub fn open_ledger_window(app: &AppHandle,session: Option<&str>,anchor: Option<&str>) -> Result<(),String> {
    let base = app.state::<ServiceState>().connection.lock().map_err(|e| e.to_string())?.as_ref().map(|v| v.0.clone()).ok_or("Service not ready")?;
    let url = ledger_url(&base,session,anchor)?;
    app.set_activation_policy(tauri::ActivationPolicy::Regular).map_err(|e| e.to_string())?;
    if let Some(window) = app.get_webview_window("dashboard") {
        window.navigate(url).map_err(|e| e.to_string())?;
    } else { build_window(app,"dashboard",url.as_str().strip_prefix(&base).unwrap_or("/"),false)?; }
    if let Some(window) = app.get_webview_window("dashboard") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
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
                        let _ = app.run_on_main_thread(move || {
                            // Initialization scripts retain their original token. Recreate webviews
                            // after a service restart so navigation cannot restore an expired token.
                            for label in ["main","dashboard"] {
                                if let Some(window) = handle.get_webview_window(label) {
                                    let visible = window.is_visible().unwrap_or(false);
                                    let path = window.url().map(|url| format!("{}{}",url.path(),url.query().map(|q| format!("?{q}")).unwrap_or_default())).unwrap_or_else(|_| "/".into());
                                    let _ = window.destroy();
                                    if let Err(error) = build_window(&handle,label,&path,label == "main") { eprintln!("Could not reconnect window: {error}"); }
                                    if let Some(window) = handle.get_webview_window(label) {
                                        let _ = if visible { window.show() } else { window.hide() };
                                    }
                                }
                            }
                            if handle.get_webview_window("main").is_none() {
                                if let Err(e) = build_window(&handle,"main","/menubar",true) { eprintln!("Could not open menubar: {e}"); }
                                if !std::env::args().any(|arg| arg == "--background") {
                                    if let Err(e) = open_ledger_window(&handle,None,None) { eprintln!("Could not open Keepline: {e}"); }
                                }
                            }
                        });
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
    use super::{ledger_url,service_capability,stop_owned_service};
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
