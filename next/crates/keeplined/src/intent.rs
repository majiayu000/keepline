use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::protocol::{INTENT_DIR_NAME, MAX_COLUMNS, MAX_ROWS, MIN_DIMENSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntentState {
    Intent,
    Running,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntentRecord {
    pub operation_id: String,
    pub terminal_id: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub payload: String,
    pub state: IntentState,
    pub pid: Option<u32>,
    pub instance_generation: u64,
    pub error: Option<String>,
}

pub(crate) fn load_intents(runtime_dir: &Path) -> io::Result<Vec<IntentRecord>> {
    let intent_dir = runtime_dir.join(INTENT_DIR_NAME);
    if !intent_dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = fs::read_dir(&intent_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut intents = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let intent = read_intent(&path)?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem != intent_file_stem(&intent.operation_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("intent filename does not match {}", intent.operation_id),
            ));
        }
        intents.push(intent);
    }
    Ok(intents)
}

pub(crate) fn write_intent(runtime_dir: &Path, intent: &IntentRecord) -> io::Result<()> {
    let dir = runtime_dir.join(INTENT_DIR_NAME);
    fs::create_dir_all(&dir)?;
    let dir = dir.canonicalize()?;
    chmod_owned_directory(&dir, "intents directory")?;
    // Lowercase hex keeps `build` and `BUILD` as two files on a case-insensitive volume.
    let path = dir.join(format!("{}.json", intent_file_stem(&intent.operation_id)));
    let bytes = serde_json::to_vec_pretty(intent).map_err(|err| {
        io::Error::new(io::ErrorKind::InvalidData, format!("encode intent: {err}"))
    })?;
    write_atomic(&path, &bytes)
}

fn read_intent(path: &Path) -> io::Result<IntentRecord> {
    let mut file = File::open(path)?;
    let mut body = String::new();
    file.read_to_string(&mut body)?;
    let intent: IntentRecord = serde_json::from_str(&body).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("intent {} is corrupt: {err}", path.display()),
        )
    })?;
    let expected = canonical_payload(&intent.argv, &intent.cwd, intent.cols, intent.rows)?;
    if expected != intent.payload {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "intent {} payload does not match its argv",
                intent.operation_id
            ),
        ));
    }
    Ok(intent)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    set_mode(path, 0o600)?;
    if let Some(parent) = path.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

pub(crate) fn sync_dir(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

pub(crate) fn canonical_payload(
    argv: &[String],
    cwd: &str,
    cols: u16,
    rows: u16,
) -> io::Result<String> {
    serde_json::to_string(&json!({
        "argv": argv,
        "cwd": cwd,
        "cols": cols,
        "rows": rows,
    }))
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, format!("encode payload: {err}")))
}

pub(crate) fn validate_argv(argv: &[String]) -> Result<(), String> {
    if argv.is_empty() || argv.len() > 32 {
        return Err("argv must contain 1 to 32 arguments".to_owned());
    }
    let program = &argv[0];
    if !program.starts_with('/') || program.contains('\0') || program.len() > 4096 {
        return Err("argv[0] must be an absolute path".to_owned());
    }
    if argv
        .iter()
        .skip(1)
        .any(|arg| arg.contains('\0') || arg.len() > 4096)
    {
        return Err("argv contains an invalid argument".to_owned());
    }
    Ok(())
}

pub(crate) fn validate_cwd(cwd: &Path) -> Result<(), String> {
    if !cwd.is_absolute() {
        return Err("cwd must be absolute".to_owned());
    }
    let metadata =
        fs::metadata(cwd).map_err(|_| format!("cwd {} does not exist", cwd.display()))?;
    if !metadata.is_dir() {
        return Err(format!("cwd {} is not a directory", cwd.display()));
    }
    Ok(())
}

pub(crate) fn validate_geometry(cols: u16, rows: u16) -> Result<(), String> {
    if !(MIN_DIMENSION..=MAX_COLUMNS).contains(&cols) || !(MIN_DIMENSION..=MAX_ROWS).contains(&rows)
    {
        return Err(format!(
            "geometry must be {MIN_DIMENSION}..={MAX_COLUMNS} columns by {MIN_DIMENSION}..={MAX_ROWS} rows"
        ));
    }
    Ok(())
}

pub(crate) fn intent_file_stem(operation_id: &str) -> String {
    hex_encode(operation_id.as_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn valid_operation_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (1..=80).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

pub(crate) fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
}

pub(crate) fn require_owned_directory(path: &Path, label: &str) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} must be a directory owned by the daemon"),
        ));
    }
    let uid = metadata.uid();
    let euid = daemon_euid();
    if uid != euid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} is owned by uid {uid}, not the daemon euid {euid}"),
        ));
    }
    Ok(())
}

pub(crate) fn chmod_owned_directory(path: &Path, label: &str) -> io::Result<()> {
    require_owned_directory(path, label)?;
    set_mode(path, 0o700)?;
    require_mode(path, 0o700, label)
}

fn daemon_euid() -> u32 {
    // SAFETY: geteuid has no inputs and cannot fail.
    unsafe { libc::geteuid() }
}

pub(crate) fn require_mode(path: &Path, expected: u32, label: &str) -> io::Result<()> {
    let mode = fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} mode is {mode:04o}, expected {expected:04o}"),
        ));
    }
    Ok(())
}

pub(crate) fn prepare_runtime(runtime_dir: &Path) -> io::Result<PathBuf> {
    let created = nonexistent_chain(runtime_dir)?;
    fs::create_dir_all(runtime_dir)?;
    let runtime_dir = runtime_dir.canonicalize()?;
    chmod_owned_directory(&runtime_dir, "runtime directory")?;
    let intent_dir = runtime_dir.join(INTENT_DIR_NAME);
    fs::create_dir_all(&intent_dir)?;
    let intent_dir = intent_dir.canonicalize()?;
    chmod_owned_directory(&intent_dir, "intents directory")?;
    sync_created_parents(&created)?;
    // A file fsync does not persist the new directory entry in its parent.
    sync_dir(&runtime_dir)?;
    Ok(runtime_dir)
}

fn nonexistent_chain(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut cursor = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut missing = Vec::new();
    while !cursor.exists() {
        missing.push(cursor.clone());
        if !cursor.pop() {
            break;
        }
    }
    missing.reverse();
    Ok(missing)
}

fn sync_created_parents(created: &[PathBuf]) -> io::Result<()> {
    let mut synced = Vec::new();
    for dir in created {
        let Some(parent) = dir.parent().filter(|parent| !parent.as_os_str().is_empty()) else {
            continue;
        };
        let parent = parent.canonicalize()?;
        if synced.iter().any(|done| done == &parent) {
            continue;
        }
        sync_dir(&parent)?;
        synced.push(parent);
    }
    Ok(())
}

pub(crate) fn lock_runtime(runtime_dir: &Path) -> io::Result<File> {
    let path = runtime_dir.join("keeplined.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|err| {
            if err.raw_os_error() == Some(libc::ELOOP) {
                io::Error::new(
                    ErrorKind::PermissionDenied,
                    "runtime lock must not be a symlink",
                )
            } else {
                err
            }
        })?;
    set_lock_mode(&file)?;
    // SAFETY: file is an open descriptor and LOCK_EX|LOCK_NB does not touch other memory.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        return Ok(file);
    }
    let err = io::Error::last_os_error();
    if err.kind() == ErrorKind::WouldBlock || err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Err(io::Error::new(ErrorKind::AlreadyExists, "already_running"))
    } else {
        Err(err)
    }
}

fn set_lock_mode(file: &File) -> io::Result<()> {
    // SAFETY: file is the descriptor opened above, so this does not follow a
    // path that was replaced with a symlink.
    let rc = unsafe { libc::fchmod(file.as_raw_fd(), 0o600) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub(crate) fn random_hex(nbytes: usize) -> io::Result<String> {
    let mut bytes = vec![0u8; nbytes];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex_encode(&bytes))
}

pub(crate) fn intent_cwd(cwd: &str) -> PathBuf {
    PathBuf::from(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

    fn sample(operation_id: &str) -> IntentRecord {
        let argv = vec!["/bin/sh".to_owned()];
        let cwd = "/tmp".to_owned();
        let payload = canonical_payload(&argv, &cwd, 80, 24).expect("payload");
        IntentRecord {
            operation_id: operation_id.to_owned(),
            terminal_id: format!("term-{operation_id}"),
            argv,
            cwd,
            cols: 80,
            rows: 24,
            payload,
            state: IntentState::Intent,
            pid: None,
            instance_generation: 1,
            error: None,
        }
    }

    #[test]
    fn case_variant_operation_ids_keep_distinct_intent_files() {
        let runtime = std::env::temp_dir().join(format!(
            "keeplined-intent-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&runtime);
        write_intent(&runtime, &sample("build")).expect("write build");
        write_intent(&runtime, &sample("BUILD")).expect("write BUILD");

        let dir = runtime.join(INTENT_DIR_NAME);
        let lower = dir.join(format!("{}.json", intent_file_stem("build")));
        let upper = dir.join(format!("{}.json", intent_file_stem("BUILD")));
        assert_ne!(lower, upper);
        let lower_meta = fs::metadata(&lower).expect("build intent");
        let upper_meta = fs::metadata(&upper).expect("BUILD intent");
        assert_ne!(
            (lower_meta.dev(), lower_meta.ino()),
            (upper_meta.dev(), upper_meta.ino())
        );

        let mut loaded = load_intents(&runtime).expect("load");
        loaded.sort_by(|left, right| left.operation_id.cmp(&right.operation_id));
        let ids: Vec<_> = loaded
            .iter()
            .map(|intent| intent.operation_id.as_str())
            .collect();
        assert_eq!(ids, ["BUILD", "build"]);
        assert_eq!(loaded[0].terminal_id, "term-BUILD");
        assert_eq!(loaded[1].terminal_id, "term-build");
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn write_intent_does_not_chmod_an_unowned_intents_directory() {
        let Some(foreign) = unowned_directory() else {
            return;
        };
        let before = fs::metadata(foreign).expect("foreign metadata").mode();
        let runtime = std::env::temp_dir().join(format!(
            "keeplined-intent-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&runtime);
        fs::create_dir_all(&runtime).expect("runtime");
        symlink(foreign, runtime.join(INTENT_DIR_NAME)).expect("symlink");
        let err = write_intent(&runtime, &sample("op")).expect_err("unowned intents");
        assert!(err.to_string().contains("not the daemon euid"), "{err}");
        assert_eq!(
            fs::metadata(foreign).expect("foreign metadata").mode(),
            before
        );
        let _ = fs::remove_dir_all(&runtime);
    }

    #[test]
    fn nested_runtime_directory_is_private() {
        let root = temp_root("nested");
        let canonical = prepare_runtime(&root.join("nested").join("runtime")).expect("prepare");
        assert!(canonical.parent().unwrap().ends_with("nested"));
        assert_eq!(canonical.metadata().expect("meta").uid(), daemon_euid());
        require_mode(&canonical, 0o700, "runtime").expect("runtime mode");
        require_mode(&canonical.join(INTENT_DIR_NAME), 0o700, "intents").expect("intents mode");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn prepare_runtime_refuses_a_directory_it_does_not_own() {
        let Some(foreign) = unowned_directory() else {
            return;
        };
        let before = fs::metadata(foreign).expect("foreign").mode();
        let err = prepare_runtime(foreign).expect_err("foreign runtime");
        assert!(err.to_string().contains("not the daemon euid"), "{err}");
        assert_eq!(fs::metadata(foreign).expect("foreign").mode(), before);

        let root = temp_root("foreign-intents");
        let runtime = root.join("runtime");
        fs::create_dir_all(&runtime).expect("runtime");
        symlink(foreign, runtime.join(INTENT_DIR_NAME)).expect("symlink");
        let err = prepare_runtime(&runtime).expect_err("foreign intents");
        assert!(err.to_string().contains("not the daemon euid"), "{err}");
        assert_eq!(fs::metadata(foreign).expect("foreign").mode(), before);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_runtime_creates_a_private_lock() {
        let root = temp_root("lock");
        let runtime = prepare_runtime(&root.join("runtime")).expect("prepare");
        let file = lock_runtime(&runtime).expect("lock");
        assert_eq!(
            file.metadata().expect("meta").permissions().mode() & 0o777,
            0o600
        );
        let err = lock_runtime(&runtime).expect_err("second lock");
        assert_eq!(err.kind(), ErrorKind::AlreadyExists);
        drop(file);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_runtime_does_not_follow_a_symlink() {
        let root = temp_root("lock-link");
        let runtime = prepare_runtime(&root.join("runtime")).expect("prepare");
        let secret = root.join("secret");
        fs::write(&secret, b"keep").expect("secret");
        let mode = fs::metadata(&secret).expect("meta").mode();
        symlink(&secret, runtime.join("keeplined.lock")).expect("symlink");
        let err = lock_runtime(&runtime).expect_err("symlink lock");
        assert_eq!(err.kind(), ErrorKind::PermissionDenied, "{err}");
        assert!(err.to_string().contains("symlink"), "{err}");
        assert_eq!(fs::read(&secret).expect("secret"), b"keep");
        assert_eq!(fs::metadata(&secret).expect("meta").mode(), mode);
        assert!(fs::symlink_metadata(runtime.join("keeplined.lock"))
            .expect("lock")
            .file_type()
            .is_symlink());
        let _ = fs::remove_dir_all(&root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "keeplined-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn unowned_directory() -> Option<&'static Path> {
        let path = Path::new("/usr");
        let meta = fs::metadata(path).ok()?;
        if meta.uid() == daemon_euid() {
            None
        } else {
            Some(path)
        }
    }
}
