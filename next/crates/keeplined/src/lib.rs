mod agents;
mod codex_status;
mod command_marks;
mod diagnostics;
mod file_events;
mod handoff;
mod host;
pub mod log_index;
mod official;
mod plugin_hooks;
mod process_exit;
mod pty;
pub mod recognition;
pub mod screen_rules;
mod store;
pub use agents::hooks;
pub use host::{serve, serve_taking_over, serve_with_options, serve_with_roots};
pub const PROTOCOL_MAJOR: u32 = keepline_core::protocol::MAJOR;
pub const PROTOCOL_MINOR: u32 = 1;

pub mod content;

/// Starts writing out the conversations declared CLIs keep in SQLite, so the host reads
/// them like any transcript. Only the real daemon calls it; fixtures use their own roots.
pub fn start_mirrors() {
    if let Some(home) = std::env::var_os("HOME") {
        agents::mirror::start(std::path::Path::new(&home));
    }
}
