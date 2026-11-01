//! Frames on the machine's socket to the relay: JSON text frames, and binary frames
//! `[kind][id length][id][payload]` for message bytes (alma-relay protocol 2).
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The newest tunnel protocol spoken here.
pub const PROTOCOL: u32 = 2;
/// Where a machine registers.
pub const AGENT_PATH: &str = "/agent";
/// The path after `/c/<token>` that reaches Keepline's remote protocol.
pub const KEEPLINE_PATH: &str = "/keepline";

/// Binary frame kinds.
pub mod bin {
    /// A binary WebSocket message, either way.
    pub const WBIN: u8 = 3;
    /// A text WebSocket message, either way, UTF-8.
    pub const WTEXT: u8 = 4;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Frame {
    /// Machine → relay, first: route phones that know `sha256_hex(token)` here.
    Register {
        #[serde(rename = "tokenHash")]
        token_hash: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ver: Option<String>,
        /// The relay's registration key, when it asks for one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proto: Option<u32>,
    },
    Registered {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proto: Option<u32>,
    },
    Error {
        message: String,
    },
    /// A phone opened a WebSocket on `path` (the part after `/c/<token>`).
    Wopen {
        id: String,
        path: String,
    },
    Wopened {
        id: String,
    },
    Werr {
        id: String,
        message: String,
    },
    /// A message in JSON form; protocol 2 carries them as binary frames instead.
    Wmsg {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        s: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<String>,
    },
    Wclose {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<u16>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A phone made a plain HTTP request; this relay never sends one, alma-relay may.
    Hreq {
        id: String,
    },
    Hres {
        id: String,
        status: u16,
        headers: serde_json::Map<String, serde_json::Value>,
    },
    Hend {
        id: String,
    },
    /// Anything else the other side speaks; ignored.
    #[serde(other)]
    Unknown,
}

impl Frame {
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).expect("frames serialize")
    }
}

pub fn encode(kind: u8, id: &str, payload: &[u8]) -> Vec<u8> {
    assert!(id.len() <= 255, "frame id too long");
    let mut out = Vec::with_capacity(2 + id.len() + payload.len());
    out.push(kind);
    out.push(id.len() as u8);
    out.extend_from_slice(id.as_bytes());
    out.extend_from_slice(payload);
    out
}

/// `(kind, id, payload)`, or `None` when malformed.
pub fn decode(bytes: &[u8]) -> Option<(u8, &str, &[u8])> {
    let (&kind, rest) = bytes.split_first()?;
    let (&len, rest) = rest.split_first()?;
    let id = std::str::from_utf8(rest.get(..len as usize)?).ok()?;
    Some((kind, id, &rest[len as usize..]))
}

pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn is_token_hash(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `/c/<token>/rest?query` → `(token, "/rest?query")`.
pub fn client_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/c/")?;
    let (token, tail) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let ok = (16..=128).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    ok.then_some((token, if tail.is_empty() { "/" } else { tail }))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_match_the_alma_relay_shapes() {
        let f = Frame::Register {
            token_hash: "ab".into(),
            name: Some("mac".into()),
            ver: None,
            key: None,
            proto: Some(2),
        };
        assert_eq!(
            f.to_text(),
            r#"{"t":"register","tokenHash":"ab","name":"mac","proto":2}"#
        );
        let f: Frame =
            serde_json::from_str(r#"{"t":"wopen","id":"w1","path":"/keepline"}"#).unwrap();
        assert_eq!(
            f,
            Frame::Wopen {
                id: "w1".into(),
                path: "/keepline".into()
            }
        );
        let f: Frame =
            serde_json::from_str(r#"{"t":"pushed","id":"1","unsupported":true}"#).unwrap();
        assert_eq!(f, Frame::Unknown);
        let f: Frame = serde_json::from_str(
            r#"{"t":"hreq","id":"h1","method":"GET","path":"/","headers":{}}"#,
        )
        .unwrap();
        assert_eq!(f, Frame::Hreq { id: "h1".into() });
    }

    #[test]
    fn binary_frames_round_trip_and_reject_short_ones() {
        let b = encode(bin::WBIN, "w12", b"\x00hi");
        assert_eq!(decode(&b), Some((bin::WBIN, "w12", &b"\x00hi"[..])));
        assert_eq!(decode(&[3]), None);
        assert_eq!(decode(&[3, 5, b'a']), None);
        assert_eq!(decode(&[3, 0]), Some((3, "", &b""[..])));
    }

    #[test]
    fn client_paths_carry_a_token_and_the_rest() {
        let t = "abcdefghijklmnop";
        assert_eq!(
            client_path(&format!("/c/{t}/keepline")),
            Some((t, "/keepline"))
        );
        assert_eq!(client_path(&format!("/c/{t}")), Some((t, "/")));
        assert_eq!(client_path(&format!("/c/{t}/k?x=1")), Some((t, "/k?x=1")));
        assert_eq!(client_path(&format!("/c/{t}?x=1")), None);
        assert_eq!(client_path("/c/short/keepline"), None);
        assert_eq!(client_path("/c/abcdefghijklmnop!/x"), None);
        assert_eq!(client_path("/agent"), None);
    }

    #[test]
    fn token_hashes_are_lowercase_sha256() {
        let h = token_hash("abc");
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(is_token_hash(&h));
        assert!(!is_token_hash(&h.to_uppercase()));
    }
}
