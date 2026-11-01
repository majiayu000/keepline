//! The pairing link a QR code carries:
//! `keepline://pair?v=2&relay=…&code=…&device=…&key=…&machine=…&name=…`.
//! `code` is the machine's, shared by its phones; `device` and `key` are this phone's own.
use crate::secure::Code;

pub const VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The relay, as given: `https://relay.example.com`.
    pub relay: String,
    pub code: Code,
    pub device: String,
    pub key: Code,
    /// The machine's id and name, for the phone to show before it connects.
    pub machine: String,
    pub name: String,
}

impl Link {
    pub fn parse(text: &str) -> Result<Self, String> {
        let query = text
            .trim()
            .strip_prefix("keepline://pair?")
            .ok_or("not a keepline://pair link")?;
        let field = |name: &str| {
            query
                .split('&')
                .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
                .map(unescape)
        };
        let version = field("v").and_then(|v| v.parse::<u32>().ok());
        if version != Some(VERSION) {
            return Err(format!("unsupported link version {version:?}"));
        }
        let code = |name: &str| {
            field(name)
                .and_then(|c| Code::parse(&c))
                .ok_or(format!("bad {name}"))
        };
        Ok(Self {
            relay: field("relay").ok_or("no relay")?,
            code: code("code")?,
            device: field("device")
                .filter(|d| !d.is_empty())
                .ok_or("no device")?,
            key: code("key")?,
            machine: field("machine").unwrap_or_default(),
            name: field("name").unwrap_or_default(),
        })
    }

    pub fn text(&self) -> String {
        format!(
            "keepline://pair?v={VERSION}&relay={}&code={}&device={}&key={}&machine={}&name={}",
            escape(&self.relay),
            self.code.text(),
            escape(&self.device),
            self.key.text(),
            escape(&self.machine),
            escape(&self.name),
        )
    }

    /// `https://host` → `wss://host`; a bare host is taken as https.
    pub fn websocket_base(relay: &str) -> Result<String, String> {
        let relay = relay.trim_end_matches('/');
        if let Some(rest) = relay.strip_prefix("https://") {
            Ok(format!("wss://{rest}"))
        } else if let Some(rest) = relay.strip_prefix("http://") {
            Ok(format!("ws://{rest}"))
        } else if relay.starts_with("wss://") || relay.starts_with("ws://") {
            Ok(relay.to_owned())
        } else if !relay.contains("://") && !relay.is_empty() {
            Ok(format!("wss://{relay}"))
        } else {
            Err(format!("看不懂的中继地址 {relay}"))
        }
    }

    /// Where this phone opens its WebSocket.
    pub fn url(&self) -> Result<String, String> {
        Ok(format!(
            "{}/c/{}{}?d={}",
            Self::websocket_base(&self.relay)?,
            self.code.token(),
            crate::wire::KEEPLINE_PATH,
            escape(&self.device)
        ))
    }
}

pub fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = b
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_round_trip_and_name_their_url() {
        let link = Link {
            relay: "https://relay.example.com".into(),
            code: Code::generate().unwrap(),
            device: "a1b2c3d4e5f6".into(),
            key: Code::generate().unwrap(),
            machine: "m-1".into(),
            name: "Lin 的 Mac".into(),
        };
        let text = link.text();
        assert!(text.contains("relay=https%3A%2F%2Frelay.example.com"));
        assert_eq!(Link::parse(&text).unwrap(), link);
        let url = link.url().unwrap();
        assert!(url.starts_with("wss://relay.example.com/c/"));
        assert!(url.ends_with("/keepline?d=a1b2c3d4e5f6"));
        assert!(Link::parse(&text.replace("v=2", "v=1")).is_err());
        assert!(Link::parse("https://x").is_err());
    }

    #[test]
    fn relay_addresses_become_websocket_bases() {
        let base = Link::websocket_base;
        assert_eq!(
            base("https://r.example.com/").unwrap(),
            "wss://r.example.com"
        );
        assert_eq!(
            base("http://127.0.0.1:8787").unwrap(),
            "ws://127.0.0.1:8787"
        );
        assert_eq!(base("ws://h:1").unwrap(), "ws://h:1");
        assert_eq!(base("r.example.com").unwrap(), "wss://r.example.com");
        assert!(base("ftp://x").is_err());
        assert_eq!(unescape(&escape("a b/é%")), "a b/é%");
        assert_eq!(unescape("%zz%4"), "%zz%4");
    }
}
