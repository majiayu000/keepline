//! Keepline's remote protocol, shared by everything that speaks it: the machine
//! (`keepline remote`), the relay (`keepline-relay`), the desktop workbench, and as a
//! reference for phone apps in other languages.
//!
//! - [`protocol`]: the JSON lines a client and a machine exchange.
//! - [`role`]: what a paired device may do.
//! - [`link`]: the pairing link a QR code carries.
//! - [`secure`]: the Noise channel a phone and a machine run, so a relay sees ciphertext.
//! - [`wire`]: the frames between a machine and a relay (the alma-relay protocol).
//!
//! `tests/vectors.rs` checks `vectors/v1.json`: fixed inputs and the exact bytes they
//! must produce, for an implementation in another language to check itself against.
//! Shapes and rules are in `specs/extensions/remote-protocol.md` and `relay.md`.
pub mod link;
pub mod protocol;
pub mod role;
pub mod secure;
pub mod wire;
