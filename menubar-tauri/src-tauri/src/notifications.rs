use std::{ffi::{CStr, CString, c_char}, sync::OnceLock};
use tauri::AppHandle;

static APP: OnceLock<AppHandle> = OnceLock::new();
extern "C" {
    fn keepline_notifications_init(callback: extern "C" fn(*const c_char, *const c_char));
    fn keepline_notify(id: *const c_char, body: *const c_char, session: *const c_char, anchor: *const c_char, sound: bool) -> bool;
    fn keepline_notification_clear(id: *const c_char);
}
extern "C" fn clicked(session: *const c_char, anchor: *const c_char) {
    if session.is_null() || anchor.is_null() { return; }
    let session = unsafe { CStr::from_ptr(session) }.to_string_lossy().into_owned();
    let anchor = unsafe { CStr::from_ptr(anchor) }.to_string_lossy().into_owned();
    if let Some(app) = APP.get() {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Err(error) = crate::ledger_app::open_ledger_window(&handle, Some(&session), Some(&anchor)) {
                eprintln!("Could not open notification target: {error}");
            }
        });
    }
}
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    unsafe { keepline_notifications_init(clicked); }
}
pub fn show(id: &str, body: &str, session: &str, anchor: &str, sound: bool) -> Result<(), String> {
    let values: Vec<CString> = [id, body, session, anchor].iter().map(|s| CString::new(*s).map_err(|_| "Notification contains a null byte".to_string())).collect::<Result<_,_>>()?;
    if unsafe { keepline_notify(values[0].as_ptr(),values[1].as_ptr(),values[2].as_ptr(),values[3].as_ptr(),sound) } { Ok(()) } else { Err("Native notification delivery failed".into()) }
}
pub fn clear(id: &str) { if let Ok(value) = CString::new(id) { unsafe { keepline_notification_clear(value.as_ptr()); } } }
