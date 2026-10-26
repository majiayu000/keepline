//! Input protocols are decided by the live VT, not by UI guesses about the program.
use keepline_core::terminal::{
    Vt,
    libghostty_vt::{self as vt, key::Mods, mouse, terminal::Mode},
};
pub struct MouseEncoder {
    encoder: mouse::Encoder<'static>,
    options: Option<(u16, mouse::EncoderSize)>,
}
impl MouseEncoder {
    pub fn new() -> Result<Self, vt::Error> {
        Ok(Self {
            encoder: mouse::Encoder::new()?,
            options: None,
        })
    }
}
pub fn mouse_bytes(
    state: &mut MouseEncoder,
    terminal: &Vt,
    action: mouse::Action,
    button: Option<mouse::Button>,
    mods: Mods,
    position: mouse::Position,
    size: mouse::EncoderSize,
) -> Result<Vec<u8>, vt::Error> {
    // set_options_from_terminal and set_size reset Ghostty's motion dedup state.
    // Reconfigure only when negotiated modes or measured geometry actually change.
    let mut modes = 0;
    for (i, m) in [
        Mode::X10_MOUSE,
        Mode::NORMAL_MOUSE,
        Mode::BUTTON_MOUSE,
        Mode::ANY_MOUSE,
        Mode::UTF8_MOUSE,
        Mode::SGR_MOUSE,
        Mode::URXVT_MOUSE,
        Mode::SGR_PIXELS_MOUSE,
    ]
    .into_iter()
    .enumerate()
    {
        if terminal.terminal().mode(m)? {
            modes |= 1 << i;
        }
    }
    if state.options != Some((modes, size)) {
        state
            .encoder
            .set_options_from_terminal(terminal.terminal())
            .set_size(size)
            .set_track_last_cell(true);
        state.options = Some((modes, size));
    }
    let encoder = &mut state.encoder;
    encoder.set_any_button_pressed(button.is_some());
    let mut event = mouse::Event::new()?;
    event
        .set_action(action)
        .set_button(button)
        .set_mods(mods)
        .set_position(position);
    let mut bytes = vec![];
    encoder.encode_to_vec(&event, &mut bytes)?;
    Ok(bytes)
}
pub fn focus_bytes(terminal: &Vt, focused: bool) -> Result<Vec<u8>, vt::Error> {
    Ok(if terminal.terminal().mode(Mode::FOCUS_EVENT)? {
        if focused {
            b"\x1b[I".to_vec()
        } else {
            b"\x1b[O".to_vec()
        }
    } else {
        vec![]
    })
}
pub fn alternate_scroll(terminal: &Vt, up: bool) -> Result<Option<Vec<u8>>, String> {
    if terminal
        .terminal()
        .active_screen()
        .map_err(|e| e.to_string())?
        != vt::screen::Screen::Alternate
        || !terminal
            .terminal()
            .mode(Mode::ALT_SCROLL)
            .map_err(|e| e.to_string())?
    {
        return Ok(None);
    }
    let key = if up {
        vt::key::Key::ArrowUp
    } else {
        vt::key::Key::ArrowDown
    };
    crate::terminal::keyboard::encode(terminal, key, if up { "up" } else { "down" }, Mods::empty())
        .map(Some)
}
/// Dropped files as text for the program: each path with shell metacharacters escaped
/// by a backslash (Ghostty's set), joined by spaces, plus a trailing space like Terminal.app.
pub fn dropped_paths(paths: &[std::path::PathBuf]) -> String {
    let mut out = String::new();
    for path in paths {
        for c in path.to_string_lossy().chars() {
            if "\\ ()[]{}<>\"'`!#$&;|*?\t".contains(c) {
                out.push('\\');
            }
            out.push(c);
        }
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn size() -> mouse::EncoderSize {
        mouse::EncoderSize {
            screen_width: 800,
            screen_height: 480,
            cell_width: 10,
            cell_height: 20,
            padding_top: 0,
            padding_bottom: 0,
            padding_left: 0,
            padding_right: 0,
        }
    }
    #[test]
    fn mouse_modes_press_release_motion_wheel_and_dedup() {
        let mut v = Vt::new(80, 24).unwrap();
        let mut e = MouseEncoder::new().unwrap();
        let mut encode = |v: &Vt, a, b| {
            mouse_bytes(
                &mut e,
                v,
                a,
                b,
                Mods::empty(),
                mouse::Position { x: 25., y: 45. },
                size(),
            )
            .unwrap()
        };
        assert!(encode(&v, mouse::Action::Press, Some(mouse::Button::Left)).is_empty());
        v.feed(b"\x1b[?1002h\x1b[?1006h");
        assert_eq!(
            encode(&v, mouse::Action::Press, Some(mouse::Button::Left)),
            b"\x1b[<0;3;3M"
        );
        assert_eq!(
            encode(&v, mouse::Action::Release, Some(mouse::Button::Left)),
            b"\x1b[<0;3;3m"
        );
        assert_eq!(
            encode(&v, mouse::Action::Press, Some(mouse::Button::Four)),
            b"\x1b[<64;3;3M"
        );
        assert!(encode(&v, mouse::Action::Motion, None).is_empty());
        v.feed(b"\x1b[?1003h");
        let first = encode(&v, mouse::Action::Motion, None);
        let second = encode(&v, mouse::Action::Motion, None);
        assert!(!first.is_empty());
        assert!(second.is_empty());
    }
    #[test]
    fn focus_and_alternate_wheel_follow_negotiated_modes() {
        let mut v = Vt::new(80, 24).unwrap();
        assert!(focus_bytes(&v, true).unwrap().is_empty());
        v.feed(b"\x1b[?1004h");
        assert_eq!(focus_bytes(&v, true).unwrap(), b"\x1b[I");
        assert_eq!(focus_bytes(&v, false).unwrap(), b"\x1b[O");
        assert!(alternate_scroll(&v, true).unwrap().is_none());
        v.feed(b"\x1b[?1049h\x1b[?1007h");
        assert_eq!(alternate_scroll(&v, true).unwrap().unwrap(), b"\x1b[A");
        v.feed(b"\x1b[?1h");
        assert_eq!(alternate_scroll(&v, false).unwrap().unwrap(), b"\x1bOB");
        v.feed(b"\x1b[?1007l\x1b[?1004l");
        assert!(alternate_scroll(&v, true).unwrap().is_none());
        assert!(focus_bytes(&v, false).unwrap().is_empty());
    }
    #[test]
    fn dropped_paths_are_shell_escaped() {
        use std::path::PathBuf;
        assert_eq!(
            dropped_paths(&[
                PathBuf::from("/tmp/My File (1).png"),
                PathBuf::from("/tmp/a'b&c$d"),
                PathBuf::from("/tmp/截图.png"),
            ]),
            "/tmp/My\\ File\\ \\(1\\).png /tmp/a\\'b\\&c\\$d /tmp/截图.png "
        );
        assert_eq!(dropped_paths(&[]), "");
    }
}
