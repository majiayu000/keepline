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
    set_mode(&dir, 0o700)?;
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
    Ok(hex_encode(&bytes))
}

pub(crate) fn intent_cwd(cwd: &str) -> PathBuf {
    PathBuf::from(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

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
}
