//! macOS file events, including replay from a durable checkpoint. No polling backend.
use std::{
    ffi::{c_char, c_void, CStr, CString},
    io,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
    sync::{
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
};
type Ref = *const c_void;
#[repr(C)]
struct Context {
    version: isize,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(Ref) -> Ref>,
    release: Option<unsafe extern "C" fn(Ref)>,
    description: Option<unsafe extern "C" fn(Ref) -> Ref>,
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(allocator: Ref, text: *const c_char, encoding: u32) -> Ref;
    fn CFArrayCreate(allocator: Ref, values: *const Ref, count: isize, callbacks: Ref) -> Ref;
    fn CFRelease(value: Ref);
}
#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSEventsGetCurrentEventId() -> u64;
    fn FSEventStreamCreate(
        allocator: Ref,
        callback: unsafe extern "C" fn(
            Ref,
            *mut c_void,
            usize,
            *mut c_void,
            *const u32,
            *const u64,
        ),
        context: *mut Context,
        paths: Ref,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> Ref;
    fn FSEventStreamSetDispatchQueue(stream: Ref, queue: Ref);
    fn FSEventStreamStart(stream: Ref) -> u8;
    fn FSEventStreamStop(stream: Ref);
    fn FSEventStreamInvalidate(stream: Ref);
    fn FSEventStreamRelease(stream: Ref);
}
unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attr: Ref) -> Ref;
    fn dispatch_sync_f(
        queue: Ref,
        context: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void),
    );
    fn dispatch_release(queue: Ref);
}
#[derive(Debug)]
pub struct Event {
    pub path: PathBuf,
    pub flags: u32,
    pub id: u64,
}
pub struct Batch {
    pub events: Vec<Event>,
}
struct Callback {
    tx: SyncSender<Batch>,
    overflow: Arc<Mutex<Vec<PathBuf>>>,
    roots: Vec<PathBuf>,
}
#[derive(Clone)]
pub struct Notifier(Arc<Callback>);
impl Notifier {
    pub fn file(&self, path: PathBuf) {
        deliver(
            &self.0,
            Batch {
                events: vec![Event {
                    path,
                    flags: 0,
                    id: 0,
                }],
            },
        );
    }
}
pub struct Watch {
    stream: Ref,
    queue: Ref,
    callback: Arc<Callback>,
    vnodes: Vnodes,
    pub rx: Receiver<Batch>,
}
unsafe extern "C" fn callback(
    _: Ref,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    ids: *const u64,
) {
    // FSEvents owns these arrays until this callback returns. Do not retain pointers.
    let context = unsafe { &*(info as *const Callback) };
    if count > 16_384 {
        let mut scopes = context.overflow.lock().unwrap_or_else(|p| p.into_inner());
        for i in 16_384..count {
            let path = unsafe { CStr::from_ptr(*(paths as *const *const c_char).add(i)) };
            let path = PathBuf::from(std::ffi::OsString::from_vec(path.to_bytes().to_vec()));
            add_scope(&mut scopes, path.parent().unwrap_or(&path), &context.roots);
        }
    }
    let events: Vec<_> = (0..count.min(16_384))
        .map(|i| unsafe {
            let path = CStr::from_ptr(*(paths as *const *const c_char).add(i));
            Event {
                path: PathBuf::from(std::ffi::OsString::from_vec(path.to_bytes().to_vec())),
                flags: *flags.add(i),
                id: *ids.add(i),
            }
        })
        .filter(|e| {
            e.flags & 0x2f != 0 || context.roots.iter().any(|root| e.path.starts_with(root))
        })
        .collect();
    deliver(context, Batch { events });
}
fn deliver(context: &Callback, batch: Batch) {
    if batch.events.is_empty()
        && context
            .overflow
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
    {
        return;
    }
    if let Err(error) = context.tx.try_send(batch) {
        let mut scopes = context.overflow.lock().unwrap_or_else(|p| p.into_inner());
        let batch = match error {
            std::sync::mpsc::TrySendError::Full(batch)
            | std::sync::mpsc::TrySendError::Disconnected(batch) => batch,
        };
        for event in batch.events {
            let path = if event.flags & 0x20000 != 0 {
                event.path.as_path()
            } else {
                event.path.parent().unwrap_or(&event.path)
            };
            add_scope(&mut scopes, path, &context.roots);
        }
        // A full queue already contains a wakeup. The worker consumes these scopes atomically.
    }
}
/// Keep disjoint scopes inside configured roots. An event for an ancestor is clipped to its CLI root.
/// Queue memory is bounded; only overflow of scope storage widens recovery to the affected CLI root.
pub fn add_scope(scopes: &mut Vec<PathBuf>, path: &Path, roots: &[PathBuf]) {
    for root in roots {
        let scope = if path.starts_with(root) {
            path.to_owned()
        } else if root.starts_with(path) {
            root.clone()
        } else {
            continue;
        };
        if scopes.iter().any(|old| scope.starts_with(old)) {
            continue;
        }
        scopes.retain(|old| !old.starts_with(&scope));
        scopes.push(scope);
        if scopes.len() > 64 {
            scopes.retain(|old| !old.starts_with(root));
            scopes.push(root.clone());
        }
    }
}

unsafe extern "C" fn barrier(_: *mut c_void) {}
impl Watch {
    pub fn current_id() -> u64 {
        unsafe { FSEventsGetCurrentEventId() }
    }
    pub fn start(paths: &[PathBuf], since: u64) -> io::Result<Self> {
        let (tx, rx) = sync_channel(16);
        let callback_context = Arc::new(Callback {
            tx,
            overflow: Arc::new(Mutex::new(vec![])),
            roots: paths.to_vec(),
        });
        // Watch an existing ancestor if the CLI has not created its log directory yet.
        // Callback filtering excludes sibling paths (including our own SQLite WAL).
        let watched: std::collections::BTreeSet<_> = paths
            .iter()
            .map(|path| {
                path.ancestors()
                    .find(|p| p.is_dir())
                    .unwrap_or(path)
                    .to_owned()
            })
            .collect();
        let paths: Vec<CString> = watched
            .iter()
            .map(|p| CString::new(p.as_os_str().as_bytes()).map_err(io::Error::other))
            .collect::<io::Result<_>>()?;
        unsafe {
            let strings: Vec<_> = paths
                .iter()
                .map(|p| CFStringCreateWithCString(ptr::null(), p.as_ptr(), 0x08000100))
                .collect();
            if strings.iter().any(|s| s.is_null()) {
                for s in strings.into_iter().filter(|s| !s.is_null()) {
                    CFRelease(s);
                }
                return Err(io::Error::other("invalid FSEvents path"));
            }
            let array = CFArrayCreate(
                ptr::null(),
                strings.as_ptr(),
                strings.len() as isize,
                ptr::null(),
            );
            let mut context = Context {
                version: 0,
                info: Arc::as_ptr(&callback_context).cast_mut().cast(),
                retain: None,
                release: None,
                description: None,
            };
            // FileEvents | WatchRoot. Latency batches actual events; it is not a scan timer.
            let stream = FSEventStreamCreate(
                ptr::null(),
                callback,
                &mut context,
                array,
                since,
                0.2,
                0x10 | 0x4,
            );
            CFRelease(array);
            for s in strings {
                CFRelease(s);
            }
            if stream.is_null() {
                return Err(io::Error::other("FSEventStreamCreate failed"));
            }
            let queue = dispatch_queue_create(c"keepline.logs".as_ptr(), ptr::null());
            if queue.is_null() {
                FSEventStreamRelease(stream);
                return Err(io::Error::other("file-event dispatch queue unavailable"));
            }
            FSEventStreamSetDispatchQueue(stream, queue);
            let vnodes = match Vnodes::new(Arc::clone(&callback_context)) {
                Ok(v) => v,
                Err(error) => {
                    FSEventStreamInvalidate(stream);
                    FSEventStreamRelease(stream);
                    dispatch_release(queue);
                    return Err(error);
                }
            };
            let watch = Self {
                stream,
                queue,
                callback: callback_context,
                vnodes,
                rx,
            };
            if FSEventStreamStart(stream) == 0 {
                return Err(io::Error::other("FSEventStreamStart failed"));
            }
            Ok(watch)
        }
    }
    pub fn notifier(&self) -> Notifier {
        Notifier(self.callback.clone())
    }
    pub fn follow(&self, paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
        self.vnodes.follow(paths)
    }
    pub fn take_overflow(&self) -> Vec<PathBuf> {
        std::mem::take(
            &mut *self
                .callback
                .overflow
                .lock()
                .unwrap_or_else(|p| p.into_inner()),
        )
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            FSEventStreamStop(self.stream);
            FSEventStreamInvalidate(self.stream);
            // Wait for in-flight callbacks before freeing their context. Callback never blocks.
            dispatch_sync_f(self.queue, ptr::null_mut(), barrier);
            FSEventStreamRelease(self.stream);
            dispatch_release(self.queue);
        }
    }
}
// FSEvents may delay append notifications until a writer closes its descriptor.
// Only currently associated transcripts get a vnode watch; historical files do not.
struct Vnodes {
    fd: Arc<std::os::fd::OwnedFd>,
    files: Arc<Mutex<std::collections::HashMap<i32, (std::fs::File, PathBuf)>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
fn vnode_event(id: usize, filter: i16, flags: u16, fflags: u32) -> libc::kevent {
    libc::kevent {
        ident: id,
        filter,
        flags,
        fflags,
        data: 0,
        udata: ptr::null_mut(),
    }
}
impl Vnodes {
    fn new(context: Arc<Callback>) -> io::Result<Self> {
        use std::os::fd::{AsRawFd, FromRawFd};
        let raw = unsafe { libc::kqueue() };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = Arc::new(unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) });
        if unsafe { libc::fcntl(raw, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let wake = vnode_event(0, libc::EVFILT_USER, libc::EV_ADD | libc::EV_CLEAR, 0);
        if unsafe { libc::kevent(raw, &wake, 1, ptr::null_mut(), 0, ptr::null()) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let files = Arc::new(Mutex::new(std::collections::HashMap::<
            i32,
            (std::fs::File, PathBuf),
        >::new()));
        let worker_files = files.clone();
        let worker_fd = fd.clone();
        let thread = std::thread::spawn(move || {
            loop {
                let mut events = [vnode_event(0, 0, 0, 0); 64];
                // Infinite kernel wait; changes or the explicit shutdown event are the only wakeups.
                let n = unsafe {
                    libc::kevent(
                        worker_fd.as_raw_fd(),
                        ptr::null(),
                        0,
                        events.as_mut_ptr(),
                        events.len() as i32,
                        ptr::null(),
                    )
                };
                if n < 0 {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    eprintln!("vnode listener failed: {error}");
                    deliver(
                        &context,
                        Batch {
                            events: context
                                .roots
                                .iter()
                                .map(|p| Event {
                                    path: p.clone(),
                                    flags: 0x20,
                                    id: 0,
                                })
                                .collect(),
                        },
                    );
                    break;
                }
                let mut batch = vec![];
                for event in &events[..n as usize] {
                    if event.filter == libc::EVFILT_USER {
                        return;
                    }
                    let mut files = worker_files.lock().unwrap_or_else(|p| p.into_inner());
                    let key = event.ident as i32;
                    if let Some((_, path)) = files.get(&key) {
                        batch.push(Event {
                            path: path.clone(),
                            flags: if event.fflags & libc::NOTE_REVOKE != 0 {
                                1
                            } else {
                                0
                            },
                            id: 0,
                        });
                    }
                    if event.fflags & (libc::NOTE_DELETE | libc::NOTE_RENAME | libc::NOTE_REVOKE)
                        != 0
                    {
                        files.remove(&key);
                    }
                }
                deliver(&context, Batch { events: batch });
            }
        });
        Ok(Self {
            fd,
            files,
            thread: Some(thread),
        })
    }
    fn follow(&self, paths: &[PathBuf]) -> io::Result<Vec<PathBuf>> {
        let mut added = vec![];
        use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
        let mut files = self
            .files
            .lock()
            .map_err(|_| io::Error::other("vnode watch lock poisoned"))?;
        files.retain(|_, (_, path)| paths.contains(path));
        for path in paths {
            if files.values().any(|(_, p)| p == path) {
                continue;
            }
            let file = match std::fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_EVTONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW)
                .open(path)
            {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            let change = vnode_event(
                file.as_raw_fd() as usize,
                libc::EVFILT_VNODE,
                libc::EV_ADD | libc::EV_CLEAR,
                libc::NOTE_WRITE
                    | libc::NOTE_EXTEND
                    | libc::NOTE_DELETE
                    | libc::NOTE_RENAME
                    | libc::NOTE_REVOKE,
            );
            if unsafe {
                libc::kevent(
                    self.fd.as_raw_fd(),
                    &change,
                    1,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            } < 0
            {
                return Err(io::Error::last_os_error());
            }
            files.insert(file.as_raw_fd(), (file, path.clone()));
            added.push(path.clone());
        }
        Ok(added)
    }
}
impl Drop for Vnodes {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        let wake = vnode_event(0, libc::EVFILT_USER, 0, libc::NOTE_TRIGGER);
        unsafe {
            libc::kevent(
                self.fd.as_raw_fd(),
                &wake,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            );
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Canonicalize existing ancestors too, e.g. /tmp -> /private/tmp, even before roots exist.
pub fn normalized(path: &Path) -> io::Result<PathBuf> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or(e)?;
            Ok(normalized(parent)?.join(
                path.file_name()
                    .ok_or_else(|| io::Error::other("missing path name"))?,
            ))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_keeps_only_affected_subtree_and_does_not_latch() {
        let (tx, rx) = sync_channel(1);
        let context = Callback {
            tx,
            overflow: Arc::new(Mutex::new(vec![])),
            roots: vec![PathBuf::from("/logs")],
        };
        let event = |path: &str| Batch {
            events: vec![Event {
                path: PathBuf::from(path),
                flags: 0,
                id: 1,
            }],
        };
        deliver(&context, event("/logs/a/one.jsonl"));
        deliver(&context, event("/logs/b/two.jsonl"));
        deliver(&context, event("/logs/b/deep/three.jsonl"));
        let scopes = std::mem::take(&mut *context.overflow.lock().unwrap());
        assert_eq!(scopes, vec![PathBuf::from("/logs/b")]);
        rx.recv().unwrap();
        deliver(&context, event("/logs/c/four.jsonl"));
        assert_eq!(
            rx.recv().unwrap().events[0].path,
            PathBuf::from("/logs/c/four.jsonl")
        );
        assert!(context.overflow.lock().unwrap().is_empty());
    }
}
