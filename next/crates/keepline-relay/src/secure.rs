//! What a phone and a machine run inside the tunnel, so the relay sees only ciphertext.
//!
//! Pairing hands the phone a 256-bit code (in the QR). From it both sides derive:
//! - the routing token in the phone's URL; the relay only ever holds its hash;
//! - the pre-shared key of a `Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s` handshake, which
//!   the relay cannot derive from the token. Fresh ephemeral keys on every connection
//!   give each one its own keys (forward secrecy).
//!
//! The phone starts: its first WebSocket message is the handshake's first message, the
//! machine answers with the second, and every later message is one [`Channel::seal`]ed
//! plaintext: a kind byte, then its payload.
//! - [`DATA`]: the next bytes of the remote protocol's JSON lines, cut anywhere.
//! - [`ACK`]: a big-endian u32, how many DATA payload bytes the sender has taken in.
//!   A machine keeps at most [`WINDOW`] unacknowledged bytes in flight to one phone, so a
//!   phone that stops reading holds up only itself, whichever relay is between.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::io;

pub const PATTERN: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";
const PROLOGUE: &[u8] = b"keepline remote 1";
pub const DATA: u8 = 0;
pub const ACK: u8 = 1;
/// Largest DATA payload in one message; a Noise message is at most 65535 bytes.
pub const CHUNK: usize = 32 * 1024;
/// Unacknowledged DATA bytes a machine sends one phone before waiting.
pub const WINDOW: u64 = 1 << 20;
const MAX_NOISE: usize = 65535;

#[derive(Clone, PartialEq, Eq)]
pub struct Code([u8; 32]);

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(..)")
    }
}

impl Code {
    pub fn generate() -> io::Result<Self> {
        let mut code = [0; 32];
        getrandom::fill(&mut code).map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Self(code))
    }
    pub fn parse(text: &str) -> Option<Self> {
        URL_SAFE_NO_PAD
            .decode(text.trim())
            .ok()?
            .try_into()
            .ok()
            .map(Self)
    }
    /// The code as the pairing link carries it.
    pub fn text(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }
    /// What the phone puts in `/c/<token>/…`.
    pub fn token(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.derive(b"keepline route 1"))
    }
    fn psk(&self) -> [u8; 32] {
        self.derive(b"keepline psk 1")
    }
    fn derive(&self, label: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(label);
        h.update([0]);
        h.update(self.0);
        h.finalize().into()
    }
    fn handshake(&self, initiator: bool) -> io::Result<snow::HandshakeState> {
        let psk = self.psk();
        let b = snow::Builder::new(PATTERN.parse().map_err(noise)?)
            .prologue(PROLOGUE)
            .map_err(noise)?
            .psk(0, &psk)
            .map_err(noise)?;
        if initiator {
            b.build_initiator()
        } else {
            b.build_responder()
        }
        .map_err(noise)
    }
}

/// The phone's side, after sending the first message.
pub struct Handshake(snow::HandshakeState);

/// Starts a connection: send the returned bytes as the first WebSocket message.
pub fn initiate(code: &Code) -> io::Result<(Handshake, Vec<u8>)> {
    let mut hs = code.handshake(true)?;
    let mut first = vec![0; MAX_NOISE];
    let n = hs.write_message(&[], &mut first).map_err(noise)?;
    first.truncate(n);
    Ok((Handshake(hs), first))
}

impl Handshake {
    /// Takes the machine's answer; fails if it did not know the code.
    pub fn finish(mut self, answer: &[u8]) -> io::Result<Channel> {
        self.0
            .read_message(answer, &mut [0; MAX_NOISE])
            .map_err(noise)?;
        Ok(Channel(self.0.into_transport_mode().map_err(noise)?))
    }
}

/// The machine's side: answers a phone's first message, failing unless it knew the code.
pub fn respond(code: &Code, first: &[u8]) -> io::Result<(Channel, Vec<u8>)> {
    let mut hs = code.handshake(false)?;
    hs.read_message(first, &mut [0; MAX_NOISE]).map_err(noise)?;
    let mut answer = vec![0; MAX_NOISE];
    let n = hs.write_message(&[], &mut answer).map_err(noise)?;
    answer.truncate(n);
    Ok((Channel(hs.into_transport_mode().map_err(noise)?), answer))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    Data(Vec<u8>),
    Ack(u32),
}

pub struct Channel(snow::TransportState);

impl Channel {
    pub fn data(&mut self, bytes: &[u8]) -> io::Result<Vec<u8>> {
        assert!(bytes.len() <= CHUNK, "DATA payload over CHUNK");
        self.seal(DATA, bytes)
    }
    pub fn ack(&mut self, n: u32) -> io::Result<Vec<u8>> {
        self.seal(ACK, &n.to_be_bytes())
    }
    fn seal(&mut self, kind: u8, payload: &[u8]) -> io::Result<Vec<u8>> {
        let mut plain = Vec::with_capacity(1 + payload.len());
        plain.push(kind);
        plain.extend_from_slice(payload);
        let mut out = vec![0; plain.len() + 16];
        let n = self.0.write_message(&plain, &mut out).map_err(noise)?;
        out.truncate(n);
        Ok(out)
    }
    /// Fails on anything tampered with, replayed, reordered or of an unknown kind.
    pub fn open(&mut self, message: &[u8]) -> io::Result<Message> {
        let mut plain = vec![0; message.len()];
        let n = self.0.read_message(message, &mut plain).map_err(noise)?;
        plain.truncate(n);
        match plain.split_first() {
            Some((&DATA, rest)) => Ok(Message::Data(rest.to_vec())),
            Some((&ACK, rest)) => rest
                .try_into()
                .map(|b| Message::Ack(u32::from_be_bytes(b)))
                .map_err(|_| io::Error::other("bad ack")),
            _ => Err(io::Error::other("unknown message kind")),
        }
    }
}

fn noise(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("noise: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(code: &Code) -> (Channel, Channel) {
        let (hs, first) = initiate(code).unwrap();
        let (machine, answer) = respond(code, &first).unwrap();
        (hs.finish(&answer).unwrap(), machine)
    }

    #[test]
    fn both_sides_talk_after_the_handshake() {
        let code = Code::generate().unwrap();
        let (mut phone, mut machine) = pair(&code);
        let m = phone.data(b"{\"id\":1}\n").unwrap();
        assert_eq!(
            machine.open(&m).unwrap(),
            Message::Data(b"{\"id\":1}\n".to_vec())
        );
        let m = machine.data(&[7; CHUNK]).unwrap();
        assert!(m.len() < MAX_NOISE);
        assert_eq!(phone.open(&m).unwrap(), Message::Data(vec![7; CHUNK]));
        let m = phone.ack(CHUNK as u32).unwrap();
        assert_eq!(machine.open(&m).unwrap(), Message::Ack(CHUNK as u32));
    }

    #[test]
    fn a_wrong_code_tampering_or_replay_fails() {
        let code = Code::generate().unwrap();
        let (hs, first) = initiate(&Code::generate().unwrap()).unwrap();
        assert!(respond(&code, &first).is_err());
        drop(hs);

        let (mut phone, mut machine) = pair(&code);
        let mut m = phone.data(b"hi").unwrap();
        let replay = m.clone();
        m[3] ^= 1;
        assert!(machine.open(&m).is_err());

        let (mut phone, mut machine) = pair(&code);
        let m = phone.data(b"hi").unwrap();
        machine.open(&m).unwrap();
        assert!(machine.open(&m).is_err());
        // A message from another connection with the same code does not open either.
        assert!(machine.open(&replay).is_err());
    }

    #[test]
    fn code_text_round_trips_and_derivations_differ() {
        let code = Code::generate().unwrap();
        assert_eq!(Code::parse(&code.text()), Some(code.clone()));
        assert_eq!(Code::parse("short"), None);
        let token = code.token();
        assert_eq!(token.len(), 43);
        assert_ne!(token, code.text());
        assert_ne!(code.psk().to_vec(), URL_SAFE_NO_PAD.decode(&token).unwrap());
        assert!(crate::wire::client_path(&format!("/c/{token}/keepline")).is_some());
        assert_eq!(format!("{code:?}"), "Code(..)");
    }
}
