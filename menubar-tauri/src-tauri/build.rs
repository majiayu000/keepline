fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new().file("src/notifications.m").flag("-fobjc-arc").flag("-fblocks").compile("keepline_notifications");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=UserNotifications");
        println!("cargo:rerun-if-changed=src/notifications.m");
    }
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "open_ledger", "set_ledger_counts", "get_app_autostart", "set_app_autostart", "quit_app",
            "get_quota", "update_tray_icon", "resize_window", "set_dock_visibility", "get_codex_info",
            "get_codex_stats", "open_chatgpt_quota", "get_codex_rate_limits", "get_cost_overview",
        ])
    )).expect("failed to build app permissions")
}
