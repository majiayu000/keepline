use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
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
        if stem != intent.operation_id {
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
    set_mode(&dir, 0o700)?;
    let path = dir.join(format!("{}.json", intent.operation_id));
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
        File::open(parent)?.sync_all()?;
    }
    Ok(())
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

pub(crate) fn random_hex(nbytes: usize) -> io::Result<String> {
    let mut bytes = vec![0u8; nbytes];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub(crate) fn intent_cwd(cwd: &str) -> PathBuf {
    PathBuf::from(cwd)
}
