//! Physical keys feed Ghostty; committed text remains on the IME path.
use keepline_core::terminal::{
    Vt,
    libghostty_vt::key::{self, Key, Mods},
};
fn unshifted(name: &str) -> &str {
    match name {
        "~" => "`",
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        "_" => "-",
        "+" => "=",
        "{" => "[",
        "}" => "]",
        "|" => "\\",
        ":" => ";",
        "\"" => "'",
        "<" => ",",
        ">" => ".",
        "?" => "/",
        _ => name,
    }
}
pub fn physical_key(name: &str) -> Option<Key> {
    Some(match unshifted(name) {
        "a" => Key::A,
        "b" => Key::B,
        "c" => Key::C,
        "d" => Key::D,
        "e" => Key::E,
        "f" => Key::F,
        "g" => Key::G,
        "h" => Key::H,
        "i" => Key::I,
        "j" => Key::J,
        "k" => Key::K,
        "l" => Key::L,
        "m" => Key::M,
        "n" => Key::N,
        "o" => Key::O,
        "p" => Key::P,
        "q" => Key::Q,
        "r" => Key::R,
        "s" => Key::S,
        "t" => Key::T,
        "u" => Key::U,
        "v" => Key::V,
        "w" => Key::W,
        "x" => Key::X,
        "y" => Key::Y,
        "z" => Key::Z,
        "0" => Key::Digit0,
        "1" => Key::Digit1,
        "2" => Key::Digit2,
        "3" => Key::Digit3,
        "4" => Key::Digit4,
        "5" => Key::Digit5,
        "6" => Key::Digit6,
        "7" => Key::Digit7,
        "8" => Key::Digit8,
        "9" => Key::Digit9,
        "`" => Key::Backquote,
        "\\" => Key::Backslash,
        "[" => Key::BracketLeft,
        "]" => Key::BracketRight,
        "," => Key::Comma,
        "=" => Key::Equal,
        "-" => Key::Minus,
        "." => Key::Period,
        "'" => Key::Quote,
        ";" => Key::Semicolon,
        "/" => Key::Slash,
        "space" => Key::Space,
        " " => Key::Space,
        "enter" => Key::Enter,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" => Key::Escape,
        "tab" => Key::Tab,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        _ => return None,
    })
}
pub fn is_text_key(name: &str) -> bool {
    name.chars().count() == 1 || name == "space"
}
pub fn encode(vt: &Vt, physical: Key, name: &str, mods: Mods) -> Result<Vec<u8>, String> {
    let mut mods = mods;
    if unshifted(name) != name {
        mods |= Mods::SHIFT;
    }
    let mut encoder = key::Encoder::new().map_err(|e| e.to_string())?;
    encoder.set_options_from_terminal(vt.terminal());
    encoder.set_macos_option_as_alt(key::OptionAsAlt::True);
    let mut event = key::Event::new().map_err(|e| e.to_string())?;
    event.set_key(physical).set_mods(mods);
    let text = if name == "space" { " " } else { name };
    if text.chars().count() == 1 {
        event.set_unshifted_codepoint(unshifted(text).chars().next().unwrap());
        // Supply the printable character; Ghostty applies Ctrl/Alt and negotiated modes.
        event.set_utf8(Some(if mods.contains(Mods::SHIFT) {
            text.to_uppercase()
        } else {
            text.to_owned()
        }));
    }
    let mut bytes = Vec::new();
    encoder
        .encode_to_vec(&event, &mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_control_and_alt_keys() {
        let vt = Vt::new(80, 24).unwrap();
        for (name, byte) in [
            ("c", 3),
            ("d", 4),
            ("z", 26),
            ("l", 12),
            ("r", 18),
            ("\\", 28),
            ("6", 30),
            ("^", 30),
            ("_", 31),
            ("?", 127),
            ("space", 0),
        ] {
            assert_eq!(
                encode(&vt, physical_key(name).unwrap(), name, Mods::CTRL).unwrap(),
                [byte],
                "{name}"
            );
        }
        // Ghostty deliberately uses fixterms CSI-u for Ctrl-[ (even without Kitty).
        assert_eq!(
            encode(&vt, Key::BracketLeft, "[", Mods::CTRL).unwrap(),
            b"\x1b[91;5u"
        );
        for name in ["a", "c", "z", "1", "/"] {
            assert_eq!(
                encode(&vt, physical_key(name).unwrap(), name, Mods::ALT).unwrap(),
                format!("\x1b{name}").as_bytes()
            );
        }
    }
    #[test]
    fn kitty_uses_terminal_negotiated_mode() {
        let mut vt = Vt::new(80, 24).unwrap();
        vt.feed(b"\x1b[>1u");
        for (name, code) in [
            ("c", 99),
            ("d", 100),
            ("z", 122),
            ("l", 108),
            ("r", 114),
            ("\\", 92),
        ] {
            assert_eq!(
                encode(&vt, physical_key(name).unwrap(), name, Mods::CTRL).unwrap(),
                format!("\x1b[{code};5u").as_bytes()
            );
        }
        assert_eq!(encode(&vt, Key::A, "a", Mods::ALT).unwrap(), b"\x1b[97;3u");
        vt.feed(b"\x1b[<u");
        assert_eq!(encode(&vt, Key::C, "c", Mods::CTRL).unwrap(), [3]);
    }
}
