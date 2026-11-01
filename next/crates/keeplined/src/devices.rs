//! Phones paired with this machine and what each may do, and the log of what they did.
//! Both files sit next to the socket, readable only by this user:
//! - `remote-devices.json`: one entry per phone, its own key, its role.
//! - `remote-log.jsonl`: connections, refusals, and every send, keys and answer.
//! - `remote-code`: the machine's code its phones find it by on a relay.
//! - `machine-id`: this machine's id, the same across restarts.
use keepline_remote::{link::Link, protocol::Machine, secure::Code};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub use keepline_remote::role::{acts, Role};

pub fn parse_role(s: &str) -> io::Result<Role> {
    Role::parse(s).ok_or_else(|| {
        io::Error::other(format!(
            "--role 只能是 view（只看）、answer（能答审批）或 full（完全控制），不是 {s}"
        ))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    /// The key its handshake proves it holds, as the pairing link carried it.
    key: String,
    pub role: Role,
    /// Unix seconds.
    pub paired: u64,
    /// Unix milliseconds of the last "disconnect now": sessions opened before it end.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub kicked: u64,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

impl Device {
    pub fn key(&self) -> Option<Code> {
        Code::parse(&self.key)
    }
}

fn file(socket: &Path) -> PathBuf {
    socket.with_file_name("remote-devices.json")
}
pub fn log_file(socket: &Path) -> PathBuf {
    socket.with_file_name("remote-log.jsonl")
}

pub fn load(socket: &Path) -> Vec<Device> {
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

pub fn add(socket: &Path, name: &str, role: Role) -> io::Result<(Device, Code)> {
    let key = Code::generate()?;
    let device = Device {
        id: uuid::Uuid::new_v4().simple().to_string()[..12].to_owned(),
        name: name.to_owned(),
        key: key.text(),
        role,
        paired: now(),
        kicked: 0,
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
pub fn remove(socket: &Path, needle: &str) -> io::Result<Device> {
    let mut devices = load(socket);
    let gone = devices.remove(find(&devices, needle)?);
    save(socket, &devices)?;
    Ok(gone)
}

/// Gives a device another role; its open sessions end and it reconnects with the new one.
pub fn set_role(socket: &Path, needle: &str, role: Role) -> io::Result<Device> {
    let mut devices = load(socket);
    let i = find(&devices, needle)?;
    devices[i].role = role;
    save(socket, &devices)?;
    Ok(devices[i].clone())
}

/// Ends a device's open sessions now; it stays paired and may connect again.
pub fn kick(socket: &Path, needle: &str) -> io::Result<Device> {
    let mut devices = load(socket);
    let i = find(&devices, needle)?;
    devices[i].kicked = now_ms();
    save(socket, &devices)?;
    Ok(devices[i].clone())
}

pub fn clear(socket: &Path) -> io::Result<()> {
    save(socket, &[])
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn now() -> u64 {
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
pub fn log(path: &Path, mut entry: Value) {
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
pub fn last_seen(socket: &Path) -> std::collections::HashMap<String, u64> {
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

/// This machine's id stays the same across restarts, so a client that knows several
/// machines can tell their terminals apart.
pub fn machine(socket: &Path) -> Machine {
    let file = socket.with_file_name("machine-id");
    let id = std::fs::read_to_string(&file)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let id = uuid::Uuid::new_v4().to_string();
            let _ = std::fs::write(&file, &id);
            id
        });
    let mut name = [0u8; 256];
    // SAFETY: gethostname writes at most `len` bytes into the buffer it is given.
    let ok = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } == 0;
    let name = if ok {
        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        String::from_utf8_lossy(&name[..end]).into_owned()
    } else {
        String::new()
    };
    Machine { id, name }
}

/// The machine's pairing code, made on first use, readable only by this user.
pub fn machine_code(socket: &Path, new: bool) -> io::Result<Code> {
    let file = socket.with_file_name("remote-code");
    if !new {
        if let Some(code) = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| Code::parse(&t))
        {
            return Ok(code);
        }
    }
    let code = Code::generate()?;
    let tmp = file.with_extension("tmp");
    let _ = std::fs::remove_file(&tmp);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?
        .write_all(code.text().as_bytes())?;
    std::fs::rename(&tmp, &file)?;
    Ok(code)
}

/// Pairs one more phone: its device entry, and the link its QR code carries.
pub fn pair_link(
    socket: &Path,
    relay: &str,
    name: &str,
    role: Role,
) -> io::Result<(Device, String)> {
    Link::websocket_base(relay).map_err(io::Error::other)?;
    let code = machine_code(socket, false)?;
    let machine = machine(socket);
    let (device, key) = add(socket, name, role)?;
    let link = Link {
        relay: relay.to_owned(),
        code,
        device: device.id.clone(),
        key,
        machine: machine.id,
        name: machine.name,
    }
    .text();
    Ok((device, link))
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

    #[test]
    fn the_code_is_kept_private_and_only_replaced_on_request() {
        let dir = std::env::temp_dir().join(format!("kl-code-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("workbench.sock");
        let a = machine_code(&socket, false).unwrap();
        assert_eq!(machine_code(&socket, false).unwrap(), a);
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("remote-code"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let b = machine_code(&socket, true).unwrap();
        assert_ne!(a, b);
        assert_eq!(machine_code(&socket, false).unwrap(), b);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
