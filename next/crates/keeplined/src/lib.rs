//! Experimental managed PTY daemon.
//!
//! Protocol major 0 is a local JSON framing experiment. The `keepline` CLI does
//! not start this process, and this crate does not write the Bun session database.

mod grid;
mod intent;
mod protocol;
mod server;
mod session;

pub use protocol::{
    DELTA_LIMIT, INTENT_DIR_NAME, MAX_FRAME_BYTES, PROTOCOL_MAJOR, PROTOCOL_MINOR, SOCKET_FILE_NAME,
};
pub use server::serve;
