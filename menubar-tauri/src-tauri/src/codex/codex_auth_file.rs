//! Exclusive lock and same-directory replace for Codex `auth.json`.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

pub(super) struct ExclusiveFileLock {
    file: fs::File,
}

impl ExclusiveFileLock {
    pub(super) fn acquire(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let file = open_lock_file(path)?;
        lock_file_exclusive(&file)?;
        Ok(Self { file })
    }
}

impl Drop for ExclusiveFileLock {
    fn drop(&mut self) {
        // Closing the handle is the backstop: Unix flock and Windows
        // LockFileEx both drop when this descriptor is closed.
        unlock_file_exclusive(&self.file);
    }
}

pub(super) fn refresh_lock_path(auth_path: &Path) -> PathBuf {
    let mut lock_path = auth_path.as_os_str().to_os_string();
    lock_path.push(".refresh.lock");
    PathBuf::from(lock_path)
}

pub(super) fn atomic_replace_private_file(destination: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("auth.json");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let tmp_path = parent.join(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));

    let write_result = (|| -> io::Result<()> {
        let mut file = open_private_temp(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp_path, destination)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_result
}

#[cfg(unix)]
fn open_lock_file(path: &Path) -> io::Result<fs::File> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(path)
}

#[cfg(unix)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    // SAFETY: `file` is the lock file opened above. LOCK_EX only updates that
    // descriptor's advisory lock and blocks until it is held.
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn unlock_file_exclusive(file: &fs::File) {
    // SAFETY: same descriptor acquired above. The handle closes immediately
    // after this, which also releases the lock.
    unsafe {
        libc::flock(file.as_raw_fd(), libc::LOCK_UN);
    }
}

#[cfg(unix)]
fn open_private_temp(path: &Path) -> io::Result<fs::File> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    // SAFETY: `file` is the temp auth file just opened. fchmod runs before
    // secret bytes are written so umask cannot leave it world-readable.
    let rc = unsafe { libc::fchmod(file.as_raw_fd(), 0o600) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(file)
}

#[cfg(windows)]
fn open_lock_file(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    // Share read and write so another process can open the same file and wait
    // on LockFileEx. The exclusive byte-range lock is the mutual exclusion.
    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)
}

#[cfg(windows)]
fn lock_file_exclusive(file: &fs::File) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    let mut overlapped = empty_overlapped();
    // SAFETY: `file` is an open handle. LockFileEx takes an exclusive lock on
    // the whole file and blocks until that lock is held. `overlapped` lives
    // for the duration of this synchronous call.
    let rc = unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            LOCK_RANGE_LOW,
            LOCK_RANGE_HIGH,
            &mut overlapped,
        )
    };
    if rc == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(windows)]
fn unlock_file_exclusive(file: &fs::File) {
    use std::os::windows::io::AsRawHandle;
    let mut overlapped = empty_overlapped();
    // SAFETY: same handle and byte range locked above. Failure is ignored
    // because dropping the handle releases the lock.
    unsafe {
        UnlockFileEx(
            file.as_raw_handle(),
            0,
            LOCK_RANGE_LOW,
            LOCK_RANGE_HIGH,
            &mut overlapped,
        );
    }
}

#[cfg(windows)]
fn open_private_temp(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    // Unix mode 0600 has no equivalent here. Exclusive sharing keeps the temp
    // file private until this handle closes, before the same-directory rename.
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(path)
}

#[cfg(windows)]
const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x0000_0002;
#[cfg(windows)]
const LOCK_RANGE_LOW: u32 = u32::MAX;
#[cfg(windows)]
const LOCK_RANGE_HIGH: u32 = u32::MAX;

#[cfg(windows)]
#[repr(C)]
struct WindowsOverlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    h_event: *mut core::ffi::c_void,
}

#[cfg(windows)]
fn empty_overlapped() -> WindowsOverlapped {
    WindowsOverlapped {
        internal: 0,
        internal_high: 0,
        offset: 0,
        offset_high: 0,
        h_event: std::ptr::null_mut(),
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LockFileEx(
        file: *mut core::ffi::c_void,
        flags: u32,
        reserved: u32,
        bytes_low: u32,
        bytes_high: u32,
        overlapped: *mut WindowsOverlapped,
    ) -> i32;
    fn UnlockFileEx(
        file: *mut core::ffi::c_void,
        reserved: u32,
        bytes_low: u32,
        bytes_high: u32,
        overlapped: *mut WindowsOverlapped,
    ) -> i32;
}

#[cfg(not(any(unix, windows)))]
compile_error!("Codex auth locking supports Unix and Windows only");
