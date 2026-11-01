//! Phones paired with this machine and what each may do, and the log of what they did.
//! Both files sit next to the socket, readable only by this user:
//! - `remote-devices.json`: one entry per phone, its own key, its role.
//! - `remote-log.jsonl`: connections, refusals, and every send, keys and answer.
use keepline_relay::secure::Code;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    /// Sees terminals, screens and events.
    View,
    /// Also answers approvals.
    Answer,
    /// Also types and presses keys.
    Full,
}

impl Role {
    pub(crate) fn parse(s: &str) -> io::Result<Self> {
        match s {
            "view" => Ok(Self::View),
            "answer" => Ok(Self::Answer),
            "full" => Ok(Self::Full),
            _ => Err(io::Error::other(format!(
                "--role 只能是 view（只看）、answer（能答审批）或 full（完全控制），不是 {s}"
            ))),
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::View => "只看",
            Self::Answer => "能答审批",
            Self::Full => "完全控制",
        }
    }
    pub(crate) fn allows(self, op: &str) -> bool {
        match op {
            "send" | "keys" => self == Self::Full,
            "answer" => self != Self::View,
            _ => true,
        }
    }
}

/// Ops that change something; each one is logged.
pub(crate) fn acts(op: &str) -> bool {
    matches!(op, "send" | "keys" | "answer")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Device {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The key its handshake proves it holds, as the pairing link carried it.
    key: String,
    pub(crate) role: Role,
    /// Unix seconds.
    pub(crate) paired: u64,
}

impl Device {
    pub(crate) fn key(&self) -> Option<Code> {
        Code::parse(&self.key)
    }
}

fn file(socket: &Path) -> PathBuf {
    socket.with_file_name("remote-devices.json")
}
pub(crate) fn log_file(socket: &Path) -> PathBuf {
    socket.with_file_name("remote-log.jsonl")
}

pub(crate) fn load(socket: &Path) -> Vec<Device> {
    std::fs::read(file(socket))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(socket: &Path, devices: &[Device]) -> io::Result<()> {
    let path = file(socket);
    let tmp = path.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(devices)?)?;
    f.sync_all()?;
    std::fs::rename(&tmp, &path)
}

pub(crate) fn add(socket: &Path, name: &str, role: Role) -> io::Result<(Device, Code)> {
    let key = Code::generate()?;
    let device = Device {
        id: uuid::Uuid::new_v4().simple().to_string()[..12].to_owned(),
        name: name.to_owned(),
        key: key.text(),
        role,
        paired: now(),
    };
    let mut devices = load(socket);
    devices.push(device.clone());
    save(socket, &devices)?;
    Ok((device, key))
}

/// Unpairs the phone whose id starts with `needle`, or whose name is `needle`.
pub(crate) fn remove(socket: &Path, needle: &str) -> io::Result<Device> {
    let mut devices = load(socket);
    let hits: Vec<usize> = (0..devices.len())
        .filter(|&i| devices[i].name == needle || devices[i].id.starts_with(needle))
        .collect();
    match hits[..] {
        [i] => {
            let gone = devices.remove(i);
            save(socket, &devices)?;
            Ok(gone)
        }
        [] => Err(io::Error::other(format!("没有配对过的设备叫 {needle}"))),
        _ => Err(io::Error::other(format!(
            "{needle} 匹配到 {} 台设备，请写编号",
            hits.len()
        ))),
    }
}

pub(crate) fn clear(socket: &Path) -> io::Result<()> {
    save(socket, &[])
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Appends one entry, stamped with the time; a log that cannot be written is not fatal.
pub(crate) fn log(path: &Path, mut entry: Value) {
    entry["at"] = json!(now());
    let line = format!("{entry}\n");
    let _ = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

/// When each device last connected, from the log.
pub(crate) fn last_seen(socket: &Path) -> std::collections::HashMap<String, u64> {
    let mut seen = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(log_file(socket)) else {
        return seen;
    };
    for v in text
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        if let (Some("connected"), Some(d), Some(at)) =
            (v["event"].as_str(), v["device"].as_str(), v["at"].as_u64())
        {
            seen.insert(d.to_owned(), at);
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_allow_only_what_they_name() {
        for op in ["hello", "list", "read", "watch", "events", "close"] {
            assert!(Role::View.allows(op));
        }
        assert!(!Role::View.allows("answer") && !Role::View.allows("send"));
        assert!(Role::Answer.allows("answer") && !Role::Answer.allows("keys"));
        assert!(Role::Full.allows("send") && Role::Full.allows("keys"));
        assert!(Role::parse("admin").is_err());
    }

    #[test]
    fn devices_are_added_found_removed_and_kept_private() {
        let dir = std::env::temp_dir().join(format!("kl-dev-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("workbench.sock");
        let (a, key) = add(&socket, "iPhone", Role::Full).unwrap();
        let (b, _) = add(&socket, "iPad", Role::View).unwrap();
        assert_eq!(load(&socket)[0].key(), Some(key));
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(file(&socket))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(remove(&socket, "nope").is_err());
        assert_eq!(remove(&socket, &b.id[..6]).unwrap().name, "iPad");
        assert_eq!(remove(&socket, "iPhone").unwrap().id, a.id);
        assert!(load(&socket).is_empty());

        log(
            &log_file(&socket),
            json!({ "event": "connected", "device": a.id }),
        );
        assert!(last_seen(&socket).contains_key(&a.id));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
