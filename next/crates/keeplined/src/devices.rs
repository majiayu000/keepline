//! Phones paired with this machine and what each may do, and the log of what they did.
//! Both files sit next to the socket, readable only by this user:
//! - `remote-devices.json`: one entry per phone, its own key, its role.
//! - `remote-log.jsonl`: connections, refusals, and every send, keys and answer.
use keepline_remote::secure::Code;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use keepline_remote::role::{acts, Role};

pub(crate) fn parse_role(s: &str) -> io::Result<Role> {
    Role::parse(s).ok_or_else(|| {
        io::Error::other(format!(
            "--role 只能是 view（只看）、answer（能答审批）或 full（完全控制），不是 {s}"
        ))
    })
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

/// The one device whose id starts with `needle`, or whose name is `needle`.
fn find(devices: &[Device], needle: &str) -> io::Result<usize> {
    let hits: Vec<usize> = (0..devices.len())
        .filter(|&i| devices[i].name == needle || devices[i].id.starts_with(needle))
        .collect();
    match hits[..] {
        [i] => Ok(i),
        [] => Err(io::Error::other(format!("没有配对过的设备叫 {needle}"))),
        _ => Err(io::Error::other(format!(
            "{needle} 匹配到 {} 台设备，请写编号",
            hits.len()
        ))),
    }
}

/// Unpairs a device.
pub(crate) fn remove(socket: &Path, needle: &str) -> io::Result<Device> {
    let mut devices = load(socket);
    let gone = devices.remove(find(&devices, needle)?);
    save(socket, &devices)?;
    Ok(gone)
}

/// Gives a device another role; its open sessions end and it reconnects with the new one.
pub(crate) fn set_role(socket: &Path, needle: &str, role: Role) -> io::Result<Device> {
    let mut devices = load(socket);
    let i = find(&devices, needle)?;
    devices[i].role = role;
    save(socket, &devices)?;
    Ok(devices[i].clone())
}

pub(crate) fn clear(socket: &Path) -> io::Result<()> {
    save(socket, &[])
}

pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The log past which it is moved to `remote-log.1.jsonl`, replacing the older one.
const LOG_LIMIT: u64 = 4 << 20;

fn previous(path: &Path) -> PathBuf {
    path.with_extension("1.jsonl")
}

/// Appends one entry, stamped with the time; a log that cannot be written is not fatal.
pub(crate) fn log(path: &Path, mut entry: Value) {
    entry["at"] = json!(now());
    let line = format!("{entry}\n");
    if std::fs::metadata(path).is_ok_and(|m| m.len() + line.len() as u64 > LOG_LIMIT) {
        let _ = std::fs::rename(path, previous(path));
    }
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
    let path = log_file(socket);
    let text: String = [previous(&path), path]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect();
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
        assert_eq!(
            set_role(&socket, "iPad", Role::Answer).unwrap().role,
            Role::Answer
        );
        assert_eq!(load(&socket)[1].role, Role::Answer);
        assert_eq!(remove(&socket, &b.id[..6]).unwrap().name, "iPad");
        assert_eq!(remove(&socket, "iPhone").unwrap().id, a.id);
        assert!(load(&socket).is_empty());

        let path = log_file(&socket);
        log(&path, json!({ "event": "connected", "device": a.id }));
        assert!(last_seen(&socket).contains_key(&a.id));
        // Past the limit the log moves aside once; what it said is still read.
        std::fs::write(&path, vec![b'\n'; LOG_LIMIT as usize]).unwrap();
        log(&path, json!({ "event": "connected", "device": b.id }));
        assert!(std::fs::metadata(&path).unwrap().len() < 200);
        assert!(previous(&path).exists());
        assert!(last_seen(&socket).contains_key(&b.id));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
