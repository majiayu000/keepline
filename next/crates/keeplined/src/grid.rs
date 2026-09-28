use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::Term;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor};

struct GridSize {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

pub(crate) struct GridView {
    pub text: String,
    pub checksum: u64,
    pub text_checksum: u64,
    pub attributed_cells: u64,
}

struct ReplyBus {
    replies: Arc<Mutex<Vec<u8>>>,
}

impl EventListener for ReplyBus {
    fn send_event(&self, event: Event) {
        // Clipboard, color, and text-area requests stay inside the daemon.
        // Clients do not answer them, and only PtyWrite bytes go back to the master.
        let Event::PtyWrite(text) = event else {
            return;
        };
        let mut replies = match self.replies.lock() {
            Ok(replies) => replies,
            Err(poisoned) => poisoned.into_inner(),
        };
        replies.extend(text.into_bytes());
    }
}

pub(crate) struct Screen {
    term: Term<ReplyBus>,
    processor: Processor,
    replies: Arc<Mutex<Vec<u8>>>,
}

impl Screen {
    pub(crate) fn new(cols: u16, rows: u16) -> Self {
        let size = GridSize {
            columns: usize::from(cols),
            screen_lines: usize::from(rows),
        };
        let config = alacritty_terminal::term::Config {
            scrolling_history: 200,
            ..alacritty_terminal::term::Config::default()
        };
        let replies = Arc::new(Mutex::new(Vec::new()));
        let listener = ReplyBus {
            replies: Arc::clone(&replies),
        };
        Self {
            term: Term::new(config, &size, listener),
            processor: Processor::new(),
            replies,
        }
    }

    pub(crate) fn advance(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.processor.advance(&mut self.term, bytes);
        let mut replies = match self.replies.lock() {
            Ok(replies) => replies,
            Err(poisoned) => poisoned.into_inner(),
        };
        std::mem::take(&mut *replies)
    }

    pub(crate) fn resize(&mut self, cols: u16, rows: u16) {
        self.term.resize(GridSize {
            columns: usize::from(cols),
            screen_lines: usize::from(rows),
        });
    }

    pub(crate) fn view(&self) -> GridView {
        let mut text = String::new();
        let mut line = String::new();
        let mut current_line = None;
        let mut full = Fnv::new();
        let mut text_only = Fnv::new();
        let mut attributed_cells = 0u64;

        for indexed in self.term.grid().display_iter() {
            if Some(indexed.point.line) != current_line {
                if current_line.is_some() {
                    push_line(&mut text, &line);
                    line.clear();
                }
                current_line = Some(indexed.point.line);
            }
            let cell = indexed.cell;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            line.push(cell.c);
            hash_char(&mut text_only, cell.c);
            hash_char(&mut full, cell.c);
            // The parser stores an unbounded zerowidth list. One scalar keeps
            // text and checksum aligned and a 400x200 snapshot inside 1 MiB.
            if let Some(ch) = cell.zerowidth().and_then(|chars| chars.first().copied()) {
                line.push(ch);
                hash_char(&mut text_only, ch);
                hash_char(&mut full, ch);
            }
            full.write_u64(color_code(cell.fg));
            full.write_u64(color_code(cell.bg));
            full.write_u64(u64::from(cell.flags.bits()));
            if cell.c != ' ' && !is_default_fg(cell.fg) {
                attributed_cells += 1;
            }
        }
        if current_line.is_some() {
            push_line(&mut text, &line);
        }

        GridView {
            text,
            checksum: full.finish(),
            text_checksum: text_only.finish(),
            attributed_cells,
        }
    }
}

// Empty padding cells are U+0020. Other trailing spaces are cell content.
fn push_line(text: &mut String, line: &str) {
    text.push_str(line.trim_end_matches(' '));
    text.push('\n');
}

fn hash_char(hasher: &mut Fnv, ch: char) {
    let mut buf = [0u8; 4];
    hasher.write(ch.encode_utf8(&mut buf).as_bytes());
}

fn is_default_fg(color: Color) -> bool {
    matches!(color, Color::Named(NamedColor::Foreground))
}

fn color_code(color: Color) -> u64 {
    match color {
        Color::Named(named) => 0x1_0000 | u64::from(named as u16),
        Color::Indexed(index) => 0x2_0000 | u64::from(index),
        Color::Spec(rgb) => {
            0x3_0000 | (u64::from(rgb.r) << 16) | (u64::from(rgb.g) << 8) | u64::from(rgb.b)
        }
    }
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

pub(crate) struct PtyProcess {
    child: Option<Child>,
    master: Option<File>,
    pub cols: u16,
    pub rows: u16,
}

impl PtyProcess {
    pub(crate) fn into_parts(mut self) -> io::Result<(Child, File, u16, u16)> {
        let child = self.child.take();
        let master = self.master.take();
        match (child, master) {
            (Some(child), Some(master)) => Ok((child, master, self.cols, self.rows)),
            (child, master) => {
                self.child = child;
                self.master = master;
                Err(io::Error::other("pty child is already taken"))
            }
        }
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(err) = child.kill() {
                eprintln!("keeplined: failed to stop an unowned pty child: {err}");
            }
            if let Err(err) = child.wait() {
                eprintln!("keeplined: failed to reap an unowned pty child: {err}");
            }
        }
    }
}

/// `SIG_IGN` survives `exec`. A launcher that ignores terminal signals would
/// otherwise drop the interrupt and `SIGWINCH` this session promises to deliver.
fn reset_terminal_signals() -> io::Result<()> {
    for signal in [
        libc::SIGINT,
        libc::SIGQUIT,
        libc::SIGTSTP,
        libc::SIGTTIN,
        libc::SIGTTOU,
        libc::SIGWINCH,
    ] {
        let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
        action.sa_sigaction = libc::SIG_DFL;
        // SAFETY: called in the forked child. Both calls are async-signal-safe.
        let rc = unsafe {
            if libc::sigemptyset(&mut action.sa_mask) < 0 {
                return Err(io::Error::last_os_error());
            }
            libc::sigaction(signal, &action, std::ptr::null_mut())
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(crate) fn spawn_pty(
    argv: &[String],
    cwd: &Path,
    cols: u16,
    rows: u16,
) -> io::Result<PtyProcess> {
    let program = argv
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "argv must not be empty"))?;
    let mut master = -1;
    let mut slave = -1;
    let mut winsize = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: master and slave are out-params, and winsize is a valid stack value.
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut winsize,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: openpty succeeded, so both descriptors are open and owned here.
    let master_fd = unsafe { OwnedFd::from_raw_fd(master) };
    let slave_fd = unsafe { OwnedFd::from_raw_fd(slave) };
    set_cloexec(master_fd.as_raw_fd())?;
    set_cloexec(slave_fd.as_raw_fd())?;
    set_nonblocking(master_fd.as_raw_fd())?;

    let mut command = Command::new(program);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .env("TERM", "xterm-256color")
        .stdin(Stdio::from(slave_fd.try_clone()?))
        .stdout(Stdio::from(slave_fd.try_clone()?))
        .stderr(Stdio::from(slave_fd));
    // SAFETY: this runs in the forked child after stdin/stdout/stderr are the slave.
    // setsid, ioctl, getpid, tcsetpgrp, sigemptyset, and sigaction are async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            let tty = libc::STDIN_FILENO;
            if libc::ioctl(tty, libc::c_ulong::from(libc::TIOCSCTTY), 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            let pid = libc::getpid();
            if libc::tcsetpgrp(tty, pid) < 0 {
                return Err(io::Error::last_os_error());
            }
            reset_terminal_signals()
        });
    }
    let child = command
        .spawn()
        .map_err(|err| io::Error::new(err.kind(), format!("failed to spawn {}: {err}", program)))?;
    let mut pty = PtyProcess {
        child: Some(child),
        master: Some(File::from(master_fd)),
        cols,
        rows,
    };
    // The child is inside PtyProcess, so a later setup error kills and reaps it.
    if let Some(err) = injected_winsize_failure() {
        return Err(err);
    }
    let (actual_cols, actual_rows) = {
        let Some(master) = pty.master.as_ref() else {
            return Err(io::Error::other("pty master missing after spawn"));
        };
        read_post_spawn_winsize(master)?
    };
    // Screen::new allocates the grid from this size. Returning here drops the
    // child through PtyProcess before that allocation.
    let (actual_cols, actual_rows) = supported_winsize(actual_cols, actual_rows)?;
    pty.cols = actual_cols;
    pty.rows = actual_rows;
    Ok(pty)
}

fn injected_winsize_failure() -> Option<io::Error> {
    match std::env::var("KEEPLINED_TEST_FAIL_POST_SPAWN") {
        Ok(stage) if stage == "winsize" => {
            Some(io::Error::other("injected post-spawn winsize failure"))
        }
        _ => None,
    }
}

fn read_post_spawn_winsize(master: &File) -> io::Result<(u16, u16)> {
    if let Some(size) = configured_test_winsize("KEEPLINED_TEST_POST_SPAWN_WINSIZE") {
        return size;
    }
    read_winsize(master)
}

pub(crate) fn read_resize_winsize(master: &File) -> io::Result<(u16, u16)> {
    if let Some(size) = configured_test_winsize("KEEPLINED_TEST_RESIZE_WINSIZE") {
        return size;
    }
    read_winsize(master)
}

/// Accepts a winsize only inside the same bounds as a launch request.
pub(crate) fn supported_winsize(cols: u16, rows: u16) -> io::Result<(u16, u16)> {
    match crate::intent::validate_geometry(cols, rows) {
        Ok(()) => Ok((cols, rows)),
        Err(message) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("pty winsize {cols}x{rows} rejected: {message}"),
        )),
    }
}

fn configured_test_winsize(name: &str) -> Option<io::Result<(u16, u16)>> {
    let Ok(raw) = std::env::var(name) else {
        return None;
    };
    let Some((cols, rows)) = raw.split_once('x') else {
        return Some(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be COLSxROWS"),
        )));
    };
    let Ok(cols) = cols.parse::<u16>() else {
        return Some(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad test winsize columns",
        )));
    };
    let Ok(rows) = rows.parse::<u16>() else {
        return Some(Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bad test winsize rows",
        )));
    };
    Some(Ok((cols, rows)))
}

/// Writes as many bytes as the nonblocking master accepts.
///
/// `Ok(0)` means the kernel accepted nothing (`WouldBlock` before the first byte).
/// A short `Ok(n)` means the prefix was written and the caller still owns `data[n..]`.
pub(crate) fn write_available(master: &mut File, data: &[u8]) -> io::Result<usize> {
    let mut offset = 0;
    while offset < data.len() {
        match master.write(&data[offset..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "pty write returned no bytes",
                ));
            }
            Ok(written) => offset += written,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(offset),
            Err(err) => return Err(err),
        }
    }
    Ok(offset)
}

pub(crate) fn set_winsize(master: &File, cols: u16, rows: u16) -> io::Result<()> {
    let winsize = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: master is a live PTY fd and winsize is a valid stack value.
    let rc = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &winsize) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn read_winsize(master: &File) -> io::Result<(u16, u16)> {
    let mut winsize = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: master is a live PTY fd and winsize is a writable stack value.
    let rc = unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCGWINSZ, &mut winsize) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok((winsize.ws_col, winsize.ws_row))
    }
}

pub(crate) struct PtyReady {
    pub readable: bool,
    pub writable: bool,
    pub hangup: bool,
}

pub(crate) fn wait_pty(
    fd: i32,
    want_read: bool,
    want_write: bool,
    timeout_ms: i32,
) -> io::Result<PtyReady> {
    let mut events = 0;
    if want_read {
        events |= libc::POLLIN;
    }
    if want_write {
        events |= libc::POLLOUT;
    }
    if events == 0 {
        events = libc::POLLIN;
    }
    let mut fds = [libc::pollfd {
        fd,
        events,
        revents: 0,
    }];
    // SAFETY: fds contains one initialized pollfd for a live descriptor.
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), 1 as libc::nfds_t, timeout_ms) };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::Interrupted {
            return Ok(PtyReady {
                readable: false,
                writable: false,
                hangup: false,
            });
        }
        return Err(err);
    }
    let revents = fds[0].revents;
    Ok(PtyReady {
        readable: revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0,
        writable: revents & libc::POLLOUT != 0,
        hangup: revents & (libc::POLLHUP | libc::POLLERR) != 0,
    })
}

fn set_cloexec(fd: i32) -> io::Result<()> {
    set_fd_flag(fd, libc::F_GETFD, libc::F_SETFD, libc::FD_CLOEXEC)
}

fn set_nonblocking(fd: i32) -> io::Result<()> {
    set_fd_flag(fd, libc::F_GETFL, libc::F_SETFL, libc::O_NONBLOCK)
}

fn set_fd_flag(fd: i32, get: libc::c_int, set: libc::c_int, flag: libc::c_int) -> io::Result<()> {
    // SAFETY: fd is an open descriptor owned by the caller.
    let flags = unsafe { libc::fcntl(fd, get) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: flags came from this fd, and flag is a constant file-status or descriptor bit.
    let rc = unsafe { libc::fcntl(fd, set, flags | flag) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Screen;

    #[test]
    fn parses_plain_text_and_one_sgr_sequence_once() {
        let mut screen = Screen::new(80, 24);
        let replies = screen.advance(b"plain\x1b[31mred\x1b[0m\n");
        assert!(replies.is_empty(), "plain text produced {replies:?}");
        let view = screen.view();
        assert!(
            view.text.contains("plainred"),
            "visible grid was {}",
            view.text
        );
        assert!(
            view.attributed_cells >= 3,
            "attributed {}",
            view.attributed_cells
        );
        assert_ne!(view.checksum, view.text_checksum);

        let mut plain_default = false;
        let mut red_attributed = false;
        for indexed in screen.term.grid().display_iter() {
            if indexed.cell.c == 'p' {
                plain_default = super::is_default_fg(indexed.cell.fg);
            }
            if indexed.cell.c == 'r' {
                red_attributed = !super::is_default_fg(indexed.cell.fg);
            }
        }
        assert!(plain_default);
        assert!(red_attributed);
    }

    #[test]
    fn combining_character_is_part_of_the_cell_text_and_checksums() {
        let mut combined = Screen::new(80, 24);
        combined.advance("e\u{0301}\n".as_bytes());
        let combined_view = combined.view();
        assert!(
            combined_view.text.contains("e\u{0301}"),
            "grid text was {}",
            combined_view.text
        );
        let stored = combined.term.grid().display_iter().any(|indexed| {
            indexed.cell.c == 'e'
                && indexed
                    .cell
                    .zerowidth()
                    .is_some_and(|chars| chars.contains(&'\u{0301}'))
        });
        assert!(stored, "U+0301 was not stored as zerowidth");

        let mut plain = Screen::new(80, 24);
        plain.advance(b"e\n");
        let plain_view = plain.view();
        assert_ne!(combined_view.text, plain_view.text);
        assert_ne!(combined_view.checksum, plain_view.checksum);
        assert_ne!(combined_view.text_checksum, plain_view.text_checksum);
    }

    #[test]
    fn extra_zero_width_scalars_stay_out_of_the_published_cell() {
        let mut many = Screen::new(8, 2);
        many.advance("e\u{0301}\u{0302}\n".as_bytes());
        let stored: Vec<char> = many
            .term
            .grid()
            .display_iter()
            .find_map(|indexed| {
                indexed.cell.zerowidth().and_then(|chars| {
                    if chars.len() >= 2 {
                        Some(chars.to_vec())
                    } else {
                        None
                    }
                })
            })
            .expect("alacritty kept only one zerowidth scalar");
        assert!(stored.contains(&'\u{0301}'));
        assert!(stored.contains(&'\u{0302}'));

        let mut one = Screen::new(8, 2);
        one.advance("e\u{0301}\n".as_bytes());
        let many_view = many.view();
        let one_view = one.view();
        assert_eq!(many_view.text, one_view.text);
        assert!(!many_view.text.contains('\u{0302}'));
        assert_eq!(many_view.checksum, one_view.checksum);
        assert_eq!(many_view.text_checksum, one_view.text_checksum);
    }

    #[test]
    fn one_zero_width_scalar_keeps_a_max_snapshot_in_one_frame() {
        let cols = usize::from(crate::protocol::MAX_COLUMNS);
        let rows = usize::from(crate::protocol::MAX_ROWS);
        let mut screen = Screen::new(crate::protocol::MAX_COLUMNS, crate::protocol::MAX_ROWS);
        let cell = "e\u{0301}\u{0302}";
        let mut bytes = Vec::with_capacity(cols * rows * cell.len());
        for _ in 0..(cols * rows) {
            bytes.extend(cell.as_bytes());
        }
        screen.advance(&bytes);
        let view = screen.view();
        assert!(view.text.contains('\u{0301}'));
        assert!(!view.text.contains('\u{0302}'));
        assert!(view.text.len() <= cols * rows * "e\u{0301}".len() + rows);
        let payload = serde_json::json!({
            "terminal_id": "t",
            "instance_generation": 1,
            "pid": 1,
            "alive": true,
            "exit_code": null,
            "attachable": true,
            "revision": 1,
            "oldest_retained_revision": 1,
            "cols": crate::protocol::MAX_COLUMNS,
            "rows": crate::protocol::MAX_ROWS,
            "checksum": format!("{:016x}", view.checksum),
            "text_checksum": format!("{:016x}", view.text_checksum),
            "attributed_cells": view.attributed_cells,
            "text": view.text,
            "lease_generation": 0,
            "resync_required": true,
        });
        assert!(
            !crate::protocol::response_exceeds_frame("1", &payload).expect("encode"),
            "max snapshot exceeded one frame"
        );
    }

    #[test]
    fn trailing_unicode_spacing_stays_in_grid_text() {
        let mut spaced = Screen::new(8, 2);
        spaced.advance("a\u{00A0}\n".as_bytes());
        let spaced_view = spaced.view();
        assert_eq!(spaced_view.text, "a\u{00A0}\n\n");

        let mut last_cell = Screen::new(4, 2);
        last_cell.advance("abc\u{00A0}".as_bytes());
        assert_eq!(last_cell.view().text, "abc\u{00A0}\n\n");

        let mut em = Screen::new(8, 2);
        em.advance("\u{2003}\n".as_bytes());
        assert_eq!(em.view().text, "\u{2003}\n\n");

        let mut ideographic = Screen::new(8, 2);
        ideographic.advance("\u{3000}\n".as_bytes());
        assert!(
            ideographic.view().text.starts_with('\u{3000}'),
            "grid text was {:?}",
            ideographic.view().text
        );

        let mut plain = Screen::new(8, 2);
        plain.advance(b"a\n");
        let plain_view = plain.view();
        assert_eq!(plain_view.text, "a\n\n");
        assert_ne!(spaced_view.checksum, plain_view.checksum);
        assert_ne!(spaced_view.text_checksum, plain_view.text_checksum);
    }

    #[test]
    fn pty_write_replies_are_returned_once() {
        let mut screen = Screen::new(80, 24);
        assert_eq!(screen.advance(b"\x1b[5n"), b"\x1b[0n");
        assert_eq!(screen.advance(b"\x1b[6n"), b"\x1b[1;1R");
        // OSC 52 store is not turned into a client-specific answer.
        assert!(screen.advance(b"\x1b]52;c;aGVsbG8=\x07").is_empty());
    }
}
