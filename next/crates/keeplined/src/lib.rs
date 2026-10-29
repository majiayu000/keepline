mod agents;
mod codex_status;
mod file_events;
mod handoff;
mod host;
pub mod log_index;
mod official;
mod process_exit;
mod pty;
pub mod recognition;
mod store;
pub use agents::hooks;
pub use host::{serve, serve_taking_over, serve_with_options, serve_with_roots};
pub const PROTOCOL_MAJOR: u32 = keepline_core::protocol::MAJOR;
pub const PROTOCOL_MINOR: u32 = 0;

pub mod content;
