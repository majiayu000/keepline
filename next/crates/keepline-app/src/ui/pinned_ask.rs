//! How a pane shows the turn's ask once its answer runs past the top of the terminal.
//! Three ways, each on or off: a bar along the terminal's top (on until chosen), a chip
//! that floats up while reading history, and a 「↑ 你问」 button in the pane's header.
//! Kept in `~/.config/keepline/pinned-ask` as the names that are on, e.g. `bar,float`,
//! or `off`.
use gpui::{App, Global};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinnedAsk {
    pub bar: bool,
    pub float: bool,
    pub button: bool,
}
impl Default for PinnedAsk {
    fn default() -> Self {
        Self {
            bar: true,
            float: false,
            button: false,
        }
    }
}
impl Global for PinnedAsk {}
impl PinnedAsk {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or_else(Self::default, |text| Self::parse(&text))
    }
    fn parse(text: &str) -> Self {
        let on: Vec<&str> = text.trim().split(',').map(str::trim).collect();
        Self {
            bar: on.contains(&"bar"),
            float: on.contains(&"float"),
            button: on.contains(&"button"),
        }
    }
    fn text(self) -> String {
        let on: Vec<&str> = [
            (self.bar, "bar"),
            (self.float, "float"),
            (self.button, "button"),
        ]
        .into_iter()
        .filter_map(|(on, name)| on.then_some(name))
        .collect();
        if on.is_empty() {
            "off".into()
        } else {
            on.join(",")
        }
    }
    pub fn any(self) -> bool {
        self.bar || self.float || self.button
    }
    /// The header button only when no bar already says the same.
    pub fn header_button(self) -> bool {
        self.button && !self.bar
    }
}
pub fn current(cx: &App) -> PinnedAsk {
    cx.try_global::<PinnedAsk>().copied().unwrap_or_default()
}
pub fn choose(choice: PinnedAsk, cx: &mut App) -> Result<(), String> {
    let path = path().ok_or("找不到配置目录")?;
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, choice.text()))
        .map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
    cx.set_global(choice);
    Ok(())
}
fn path() -> Option<std::path::PathBuf> {
    super::status_colors::config_dir().map(|dir| dir.join("pinned-ask"))
}

#[cfg(test)]
mod tests {
    use super::PinnedAsk;

    #[test]
    fn the_saved_text_reads_back_and_off_is_all_off() {
        for choice in [
            PinnedAsk::default(),
            PinnedAsk {
                bar: true,
                float: true,
                button: false,
            },
            PinnedAsk {
                bar: false,
                float: false,
                button: true,
            },
            PinnedAsk {
                bar: false,
                float: false,
                button: false,
            },
        ] {
            assert_eq!(PinnedAsk::parse(&choice.text()), choice);
        }
        assert!(!PinnedAsk::parse("off\n").any());
    }
}
