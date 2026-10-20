//! One blocking kqueue for proved managed Agent PIDs. No polling or global process scan.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::Arc,
};

pub struct ExitWatch {
    queue: Arc<OwnedFd>,
}
impl ExitWatch {
    pub fn new(on_exit: impl Fn(u32) + Send + 'static) -> io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let queue = Arc::new(unsafe { OwnedFd::from_raw_fd(fd) });
        let thread_queue = queue.clone();
        std::thread::spawn(move || loop {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::kevent(
                    thread_queue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    &mut event,
                    1,
                    std::ptr::null(),
                )
            };
            if result < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                eprintln!("Agent exit subscription failed: {e}");
                return;
            }
            if event.flags & libc::EV_ERROR != 0 {
                let data = event.data;
                eprintln!("Agent exit event error: {data}");
                continue;
            }
            on_exit(event.ident as u32);
        });
        Ok(Self { queue })
    }
    /// False closes the registration/exit race: the process already exited.
    pub fn watch(&self, pid: u32) -> io::Result<bool> {
        let event = libc::kevent {
            ident: pid as _,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        if unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::ESRCH) {
                return Ok(false);
            }
            return Err(e);
        }
        Ok(true)
    }
}
