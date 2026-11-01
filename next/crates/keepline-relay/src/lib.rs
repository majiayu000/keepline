//! The relay: a machine dials out and registers, a phone opens `/c/<token>/…`, and each
//! phone socket is bridged over its machine's socket. It speaks the WebSocket part of the
//! alma-relay protocol ([`keepline_remote::wire`]) and never reads a message's contents:
//! phone and machine encrypt end to end ([`keepline_remote::secure`]).
//! Shapes and limits are in `specs/extensions/relay.md`.
pub mod server;
