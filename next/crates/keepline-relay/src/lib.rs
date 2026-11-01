//! A phone reaching a Keepline machine from anywhere, through a relay that sees only
//! ciphertext. The machine dials out to the relay and keeps that socket open; a phone
//! opens a WebSocket to `/c/<token>/keepline` and the relay bridges it over the machine's
//! socket. The tunnel speaks the WebSocket part of the alma-relay protocol, so a machine
//! works with this relay or with a deployed alma-relay alike.
//!
//! - [`wire`]: the frames between a machine and the relay.
//! - [`secure`]: the pairing code, and the Noise channel a phone and a machine run
//!   inside the tunnel.
//! - `server`: the relay.
//!
//! Shapes and limits are in `specs/extensions/relay.md`.
pub mod secure;
#[cfg(feature = "server")]
pub mod server;
pub mod wire;
