use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use alacritty_terminal::event::VoidListener;
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

pub(crate) struct Screen {
    term: Term<VoidListener>,
    processor: Processor,
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
        Self {
            term: Term::new(config, &size, VoidListener),
            processor: Processor::new(),
        }
    }

    pub(crate) fn advance(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
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

fn push_line(text: &mut String, line: &str) {
    text.push_str(line.trim_end());
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
        let child = self
            .child
            .take()
            .ok_or_else(|| io::Error::other("pty child is already taken"))?;
        let master = self
            .master
            .take()
            .ok_or_else(|| io::Error::other("pty master is already taken"))?;
        Ok((child, master, self.cols, self.rows))
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
    let child = command
        .spawn()
        .map_err(|err| io::Error::new(err.kind(), format!("failed to spawn {}: {err}", program)))?;
    let master = File::from(master_fd);
    let (cols, rows) = read_winsize(&master)?;
    Ok(PtyProcess {
        child: Some(child),
        master: Some(master),
        cols,
        rows,
    })
}

pub(crate) fn write_pty(master: &mut File, data: &[u8]) -> io::Result<()> {
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
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "pty backpressure",
                ));
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
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

pub(crate) fn wait_readable(fd: i32, timeout_ms: i32) -> io::Result<bool> {
    let mut fds = [libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    }];
    // SAFETY: fds contains one initialized pollfd for a live descriptor.
    let rc = unsafe { libc::poll(fds.as_mut_ptr(), 1 as libc::nfds_t, timeout_ms) };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.kind() == io::ErrorKind::Interrupted {
            return Ok(false);
        }
        return Err(err);
    }
    Ok(rc > 0)
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
        screen.advance(b"plain\x1b[31mred\x1b[0m\n");
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
}
