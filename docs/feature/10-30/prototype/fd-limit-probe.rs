extern crate libc;
fn raise_open_file_limit() {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit/setrlimit only read and write the struct passed by pointer.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            eprintln!("keeplined: getrlimit: {}", std::io::Error::last_os_error());
            return;
        }
        // macOS rejects RLIM_INFINITY for the soft limit; use a finite cap below kern.maxfilesperproc.
        let wanted = limit.rlim_max.min(65_536);
        if wanted <= limit.rlim_cur {
            return;
        }
        limit.rlim_cur = wanted;
        if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
            eprintln!("keeplined: setrlimit: {}", std::io::Error::last_os_error());
        } else {
            eprintln!("keeplined: open file limit raised to {wanted}");
        }
    }
}

fn limit() -> libc::rlimit {
    let mut l = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut l) }, 0);
    l
}
fn main() {
    let mut l = limit();
    l.rlim_cur = 256;
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &l) }, 0);
    let before = limit().rlim_cur;
    raise_open_file_limit();
    let after = limit();
    let expected = after.rlim_max.min(65_536);
    assert_eq!(after.rlim_cur, expected);
    println!("before_soft={before} after_soft={} expected={expected} hard={}", after.rlim_cur, after.rlim_max);
    let again_before = after.rlim_cur;
    raise_open_file_limit();
    assert_eq!(limit().rlim_cur, again_before);
    println!("second_call_soft={} (unchanged)", limit().rlim_cur);
}
