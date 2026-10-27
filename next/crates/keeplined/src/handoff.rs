//! Passing a running terminal's PTY to a newer host: the descriptors travel over the
//! control socket as SCM_RIGHTS, so the shells and Agents behind them never notice.
use std::{
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::net::UnixStream,
    },
};

/// Most descriptors one message carries; a host never runs more terminals than this.
pub const MAX_FDS: usize = 200;

/// Sends `fds` in one message with a one-byte payload.
pub fn send_fds(stream: &UnixStream, fds: &[RawFd]) -> io::Result<()> {
    if fds.len() > MAX_FDS {
        return Err(io::Error::other("too many terminals to hand over"));
    }
    let byte = [fds.len() as u8];
    let mut iov = libc::iovec {
        iov_base: byte.as_ptr() as *mut _,
        iov_len: 1,
    };
    let space = unsafe { libc::CMSG_SPACE(std::mem::size_of_val(fds) as u32) } as usize;
    let mut control = vec![0u8; space.max(1)];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    if !fds.is_empty() {
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = space as _;
        // SAFETY: the control buffer is CMSG_SPACE bytes for exactly these descriptors.
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&msg);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of_val(fds) as u32) as _;
            std::ptr::copy_nonoverlapping(
                fds.as_ptr(),
                libc::CMSG_DATA(header).cast::<RawFd>(),
                fds.len(),
            );
        }
    }
    // SAFETY: msg points at live buffers for the duration of the call.
    if unsafe { libc::sendmsg(stream.as_raw_fd(), &msg, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Receives the message `send_fds` sent; exactly `count` descriptors or an error.
pub fn recv_fds(stream: &UnixStream, count: usize) -> io::Result<Vec<OwnedFd>> {
    if count > MAX_FDS {
        return Err(io::Error::other("too many terminals offered"));
    }
    let mut byte = [0u8; 1];
    let mut iov = libc::iovec {
        iov_base: byte.as_mut_ptr().cast(),
        iov_len: 1,
    };
    let space = unsafe { libc::CMSG_SPACE((count * std::mem::size_of::<RawFd>()) as u32) } as usize;
    let mut control = vec![0u8; space.max(1)];
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = space as _;
    // SAFETY: msg points at live buffers for the duration of the call.
    let n = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut msg, 0) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    if n == 0 {
        return Err(io::Error::other("the old host closed before handing over"));
    }
    let mut fds = Vec::new();
    // SAFETY: walking the control messages the kernel just filled in.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&msg);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                let len = (*header).cmsg_len as usize - (data as usize - header as usize);
                for i in 0..len / std::mem::size_of::<RawFd>() {
                    fds.push(OwnedFd::from_raw_fd(*data.add(i)));
                }
            }
            header = libc::CMSG_NXTHDR(&msg, header);
        }
    }
    if msg.msg_flags & libc::MSG_CTRUNC != 0 || fds.len() != count || usize::from(byte[0]) != count
    {
        return Err(io::Error::other("handed-over terminals arrived incomplete"));
    }
    Ok(fds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    #[test]
    fn descriptors_survive_the_trip() {
        let (a, b) = UnixStream::pair().unwrap();
        let (mut left, right) = UnixStream::pair().unwrap();
        send_fds(&a, &[right.as_raw_fd()]).unwrap();
        drop(right);
        let fds = recv_fds(&b, 1).unwrap();
        let mut received = UnixStream::from(fds.into_iter().next().unwrap());
        left.write_all(b"ok").unwrap();
        let mut buf = [0; 2];
        received.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ok");
    }
}
