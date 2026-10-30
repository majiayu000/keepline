//! Shortcuts: which ⌘ chord runs which workbench or terminal action. The ones changed
//! from the defaults live in `~/.config/keepline/keys.json`; a chord without ⌘ always
//! goes to the program in the terminal (specs/terminal-workbench/keyboard-shortcuts.md).
use gpui::{App, Global, Keystroke};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    NewTab,
    Close,
    SplitRight,
    SplitDown,
    PrevTab,
    NextTab,
    PrevPane,
    NextPane,
    /// Input to the pane on that side, by where panes sit (Ghostty's goto_split).
    PaneLeft,
    PaneRight,
    PaneUp,
    PaneDown,
    /// The nth tab of the space, 1 to 8: fixed on ⌘1–⌘8, not rebindable.
    Tab(u8),
    /// Fixed on ⌘9, like the browsers and terminal apps.
    LastTab,
    Search,
    Settings,
    /// The next session that waits on you, as the top bar's chip.
    NextWaiting,
    Find,
    Clear,
    SelectAll,
    ZoomIn,
    ZoomOut,
    ZoomReset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Tabs,
    Terminal,
    Workbench,
}
impl Section {
    pub const ALL: [Self; 3] = [Self::Tabs, Self::Terminal, Self::Workbench];
    pub fn label(self) -> &'static str {
        match self {
            Self::Tabs => "标签与分屏",
            Self::Terminal => "终端",
            Self::Workbench => "工作台",
        }
    }
}
impl Action {
    pub const ALL: [Self; 30] = [
        Self::NewTab,
        Self::Close,
        Self::SplitRight,
        Self::SplitDown,
        Self::PrevTab,
        Self::NextTab,
        Self::PrevPane,
        Self::NextPane,
        Self::PaneLeft,
        Self::PaneRight,
        Self::PaneUp,
        Self::PaneDown,
        Self::Tab(1),
        Self::Tab(2),
        Self::Tab(3),
        Self::Tab(4),
        Self::Tab(5),
        Self::Tab(6),
        Self::Tab(7),
        Self::Tab(8),
        Self::LastTab,
        Self::Find,
        Self::Clear,
        Self::SelectAll,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ZoomReset,
        Self::Search,
        Self::Settings,
        Self::NextWaiting,
    ];
    pub fn name(self) -> String {
        match self {
            Self::NewTab => "new_tab".into(),
            Self::Close => "close".into(),
            Self::SplitRight => "split_right".into(),
            Self::SplitDown => "split_down".into(),
            Self::PrevTab => "prev_tab".into(),
            Self::NextTab => "next_tab".into(),
            Self::PrevPane => "prev_pane".into(),
            Self::NextPane => "next_pane".into(),
            Self::PaneLeft => "pane_left".into(),
            Self::PaneRight => "pane_right".into(),
            Self::PaneUp => "pane_up".into(),
            Self::PaneDown => "pane_down".into(),
            Self::Tab(n) => format!("tab_{n}"),
            Self::LastTab => "last_tab".into(),
            Self::Search => "search".into(),
            Self::Settings => "settings".into(),
            Self::NextWaiting => "next_waiting".into(),
            Self::Find => "find".into(),
            Self::Clear => "clear".into(),
            Self::SelectAll => "select_all".into(),
            Self::ZoomIn => "zoom_in".into(),
            Self::ZoomOut => "zoom_out".into(),
            Self::ZoomReset => "zoom_reset".into(),
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::NewTab => "新建标签".into(),
            Self::Close => "关闭画面".into(),
            Self::SplitRight => "向右分屏".into(),
            Self::SplitDown => "向下分屏".into(),
            Self::PrevTab => "上一个标签".into(),
            Self::NextTab => "下一个标签".into(),
            Self::PrevPane => "上一个画面".into(),
            Self::NextPane => "下一个画面".into(),
            Self::PaneLeft => "左边的画面".into(),
            Self::PaneRight => "右边的画面".into(),
            Self::PaneUp => "上面的画面".into(),
            Self::PaneDown => "下面的画面".into(),
            Self::Tab(n) => format!("第 {n} 个标签"),
            Self::LastTab => "最后一个标签".into(),
            Self::Search => "搜索".into(),
            Self::Settings => "设置".into(),
            Self::NextWaiting => "下一个等你的".into(),
            Self::Find => "在终端里查找".into(),
            Self::Clear => "清屏".into(),
            Self::SelectAll => "全选".into(),
            Self::ZoomIn => "放大字号".into(),
            Self::ZoomOut => "缩小字号".into(),
            Self::ZoomReset => "还原字号".into(),
        }
    }
    pub fn section(self) -> Section {
        match self {
            Self::Find | Self::Clear | Self::SelectAll => Section::Terminal,
            Self::ZoomIn | Self::ZoomOut | Self::ZoomReset => Section::Terminal,
            Self::Search | Self::Settings | Self::NextWaiting => Section::Workbench,
            _ => Section::Tabs,
        }
    }
    /// Run by the focused terminal itself; the rest are the workbench's.
    pub fn in_terminal(self) -> bool {
        self.section() == Section::Terminal
    }
    /// ⌘1–⌘9 always pick a tab: settings shows them as one read-only row.
    pub fn fixed(self) -> bool {
        matches!(self, Self::Tab(_) | Self::LastTab)
    }
    fn defaults(self) -> &'static [&'static str] {
        match self {
            Self::NewTab => &["cmd-t"],
            Self::Close => &["cmd-w"],
            Self::SplitRight => &["cmd-d"],
            Self::SplitDown => &["cmd-shift-d"],
            Self::PrevTab => &["cmd-shift-["],
            Self::NextTab => &["cmd-shift-]"],
            Self::PrevPane => &["cmd-["],
            Self::NextPane => &["cmd-]"],
            Self::PaneLeft => &["cmd-alt-left"],
            Self::PaneRight => &["cmd-alt-right"],
            Self::PaneUp => &["cmd-alt-up"],
            Self::PaneDown => &["cmd-alt-down"],
            Self::Tab(1) => &["cmd-1"],
            Self::Tab(2) => &["cmd-2"],
            Self::Tab(3) => &["cmd-3"],
            Self::Tab(4) => &["cmd-4"],
            Self::Tab(5) => &["cmd-5"],
            Self::Tab(6) => &["cmd-6"],
            Self::Tab(7) => &["cmd-7"],
            Self::Tab(_) => &["cmd-8"],
            Self::LastTab => &["cmd-9"],
            Self::Search => &["cmd-p"],
            Self::Settings => &["cmd-,"],
            Self::NextWaiting => &["cmd-j"],
            Self::Find => &["cmd-f"],
            Self::Clear => &["cmd-k"],
            Self::SelectAll => &["cmd-a"],
            Self::ZoomIn => &["cmd-=", "cmd-shift-="],
            Self::ZoomOut => &["cmd--"],
            Self::ZoomReset => &["cmd-0"],
        }
    }
    fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// One key with its modifiers; shifted symbols are kept as their base key plus ⇧.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub key: String,
}
const SHIFTED: [(&str, &str); 11] = [
    ("{", "["),
    ("}", "]"),
    ("+", "="),
    ("_", "-"),
    ("<", ","),
    (">", "."),
    ("?", "/"),
    (":", ";"),
    ("\"", "'"),
    ("|", "\\"),
    ("~", "`"),
];
const MODIFIER_KEYS: [&str; 7] = [
    "shift", "control", "alt", "platform", "cmd", "function", "fn",
];
impl Chord {
    pub fn from_keystroke(k: &Keystroke) -> Option<Self> {
        if MODIFIER_KEYS.contains(&k.key.as_str()) || k.key.is_empty() {
            return None;
        }
        let m = &k.modifiers;
        let mut chord = Self {
            cmd: m.platform,
            ctrl: m.control,
            alt: m.alt,
            shift: m.shift,
            key: k.key.to_lowercase(),
        };
        if let Some((_, base)) = SHIFTED.iter().find(|(s, _)| *s == chord.key) {
            chord.key = (*base).into();
            chord.shift = true;
        }
        Some(chord)
    }
    /// `cmd-shift-t`, as gpui writes keystrokes; the key itself may be `-`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut chord = Self {
            cmd: false,
            ctrl: false,
            alt: false,
            shift: false,
            key: String::new(),
        };
        let mut rest = text.trim();
        loop {
            let Some((head, tail)) = rest.split_once('-').filter(|(_, t)| !t.is_empty()) else {
                break;
            };
            match head {
                "cmd" => chord.cmd = true,
                "ctrl" => chord.ctrl = true,
                "alt" => chord.alt = true,
                "shift" => chord.shift = true,
                _ => return None,
            }
            rest = tail;
        }
        if rest.is_empty() || (rest.contains('-') && rest != "-") || MODIFIER_KEYS.contains(&rest) {
            return None;
        }
        let mut keystroke = Keystroke::parse("a").ok()?;
        keystroke.key = rest.into();
        let base = Self::from_keystroke(&keystroke)?;
        chord.shift |= base.shift;
        chord.key = base.key;
        Some(chord)
    }
    pub fn text(&self) -> String {
        let mut out = String::new();
        for (on, name) in [
            (self.cmd, "cmd-"),
            (self.ctrl, "ctrl-"),
            (self.alt, "alt-"),
            (self.shift, "shift-"),
        ] {
            if on {
                out.push_str(name);
            }
        }
        out + &self.key
    }
    /// ⌃⌥⇧⌘T, in the order macOS menus use.
    pub fn display(&self) -> String {
        let mut out = String::new();
        for (on, sign) in [
            (self.ctrl, "⌃"),
            (self.alt, "⌥"),
            (self.shift, "⇧"),
            (self.cmd, "⌘"),
        ] {
            if on {
                out.push_str(sign);
            }
        }
        let key = match self.key.as_str() {
            "space" => "空格".into(),
            "enter" => "↩".into(),
            "tab" => "⇥".into(),
            "backspace" => "⌫".into(),
            "escape" => "⎋".into(),
            "left" => "←".into(),
            "right" => "→".into(),
            "up" => "↑".into(),
            "down" => "↓".into(),
            key => key.to_uppercase(),
        };
        out + &key
    }
    /// ⌘1–⌘9 alone, which always pick a tab.
    fn is_tab_number(&self) -> bool {
        self.cmd
            && !self.ctrl
            && !self.alt
            && !self.shift
            && matches!(self.key.as_bytes(), [b'1'..=b'9'])
    }
    /// Why this chord cannot be bound, if it cannot.
    pub fn refusal(&self) -> Option<&'static str> {
        if !self.cmd {
            return Some("快捷键需要包含 ⌘；其他组合键留给终端里的程序");
        }
        if !self.ctrl && !self.alt && !self.shift && matches!(self.key.as_str(), "c" | "v" | "q") {
            return Some("⌘C、⌘V、⌘Q 保留给复制、粘贴和退出");
        }
        if self.key == "escape" {
            return Some("Esc 用来取消录制");
        }
        if self.is_tab_number() {
            return Some("⌘1–⌘9 固定用于切换标签");
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keymap {
    bindings: Vec<(Action, Vec<Chord>)>,
}
impl Global for Keymap {}
impl Default for Keymap {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .into_iter()
                .map(|a| {
                    (
                        a,
                        a.defaults()
                            .iter()
                            .filter_map(|t| Chord::parse(t))
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}
impl Keymap {
    /// Every rebindable action, in the order the settings page lists them.
    pub fn actions() -> impl Iterator<Item = Action> {
        Action::ALL.into_iter().filter(|a| !a.fixed())
    }
    pub fn action_for(&self, k: &Keystroke) -> Option<Action> {
        let chord = Chord::from_keystroke(k)?;
        self.bindings
            .iter()
            .find(|(_, chords)| chords.contains(&chord))
            .map(|(a, _)| *a)
    }
    pub fn chords(&self, action: Action) -> &[Chord] {
        self.bindings
            .iter()
            .find(|(a, _)| *a == action)
            .map_or(&[], |(_, c)| c.as_slice())
    }
    pub fn is_default(&self, action: Action) -> bool {
        self.chords(action) == Self::default().chords(action)
    }
    /// The other action already on `chord`, if any.
    pub fn holder(&self, action: Action, chord: &Chord) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(a, c)| *a != action && c.contains(chord))
            .map(|(a, _)| *a)
    }
    /// Puts `chord` on `action` alone, taking it from whichever action had it.
    pub fn assign(&mut self, action: Action, chord: Chord) {
        for (a, chords) in &mut self.bindings {
            if *a == action {
                *chords = vec![chord.clone()];
            } else {
                chords.retain(|c| *c != chord);
            }
        }
    }
    /// Back to the default chords; one taken by another action is taken back too.
    pub fn reset(&mut self, action: Action) {
        let defaults = Self::default().chords(action).to_vec();
        for (a, chords) in &mut self.bindings {
            if *a == action {
                *chords = defaults.clone();
            } else {
                chords.retain(|c| !defaults.contains(c));
            }
        }
    }
    /// The changed actions only, `{}` when nothing differs.
    pub fn to_json(&self) -> String {
        let changed: serde_json::Map<_, _> = self
            .bindings
            .iter()
            .filter(|(a, _)| !a.fixed() && !self.is_default(*a))
            .map(|(a, chords)| {
                let list = chords.iter().map(|c| c.text().into()).collect();
                (a.name(), serde_json::Value::Array(list))
            })
            .collect();
        serde_json::to_string_pretty(&changed).unwrap_or_else(|_| "{}".into())
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let object = value
            .as_object()
            .ok_or("expected an object of action: [chords]")?;
        let mut map = Self::default();
        // Entries an older Keepline wrote for the tab numbers, or that move ⌘1–⌘9, are
        // skipped: those chords are fixed now, and the rest of the file still applies.
        let mut named = vec![];
        'entries: for (name, list) in object {
            let action = Action::parse(name).ok_or_else(|| format!("unknown action \"{name}\""))?;
            let list = list
                .as_array()
                .ok_or_else(|| format!("\"{name}\": expected a list of chords"))?;
            let mut chords = vec![];
            for item in list {
                let chord = item
                    .as_str()
                    .and_then(Chord::parse)
                    .ok_or_else(|| format!("\"{name}\": bad chord {item}"))?;
                if chord.is_tab_number() {
                    continue 'entries;
                }
                if let Some(why) = chord.refusal() {
                    return Err(format!("\"{name}\": {}: {why}", chord.text()));
                }
                chords.push(chord);
            }
            if action.fixed() {
                continue;
            }
            if let Some((_, c)) = map.bindings.iter_mut().find(|(a, _)| *a == action) {
                *c = chords;
            }
            named.push(action);
        }
        // A file only names what changed; a default it now uses elsewhere moves with it.
        let claimed: Vec<Chord> = map
            .bindings
            .iter()
            .filter(|(a, _)| named.contains(a))
            .flat_map(|(_, c)| c.clone())
            .collect();
        for (a, chords) in &mut map.bindings {
            if !named.contains(a) {
                chords.retain(|c| !claimed.contains(c));
            }
        }
        let mut seen: Vec<&Chord> = vec![];
        for (a, chords) in &map.bindings {
            for c in chords {
                if seen.contains(&c) {
                    return Err(format!(
                        "{} is bound twice (again on \"{}\")",
                        c.text(),
                        a.name()
                    ));
                }
                seen.push(c);
            }
        }
        Ok(map)
    }
    pub fn load() -> Self {
        let Some(path) = path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                eprintln!(
                    "keepline: {}: {e}; using the default shortcuts",
                    path.display()
                );
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
    fn save(&self) {
        let Some(path) = path() else {
            return;
        };
        let json = self.to_json();
        let written = if json == "{}" {
            std::fs::remove_file(&path).or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
        } else {
            path.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, json))
        };
        if let Err(e) = written {
            eprintln!("keepline: saving {}: {e}", path.display());
        }
    }
}
/// The action bound to this keystroke in the current keymap.
pub fn action(k: &Keystroke, cx: &App) -> Option<Action> {
    match cx.try_global::<Keymap>() {
        Some(map) => map.action_for(k),
        None => Keymap::default().action_for(k),
    }
}
/// The current keymap, or the defaults before one is loaded.
pub fn current(cx: &App) -> Keymap {
    cx.try_global::<Keymap>().cloned().unwrap_or_default()
}
/// Saves the changed keymap and makes it the one every pane reads.
pub fn update(map: Keymap, cx: &mut App) {
    map.save();
    cx.set_global(map);
    cx.refresh_windows();
}
pub fn path() -> Option<std::path::PathBuf> {
    crate::ui::status_colors::config_dir().map(|dir| dir.join("keys.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &str) -> Keystroke {
        Keystroke::parse(text).unwrap()
    }

    #[test]
    fn chords_round_trip_and_shifted_symbols_fold() {
        for text in [
            "cmd-t",
            "cmd-shift-[",
            "cmd--",
            "cmd-,",
            "cmd-ctrl-alt-shift-k",
        ] {
            assert_eq!(Chord::parse(text).unwrap().text(), text);
        }
        assert_eq!(
            Chord::parse("cmd-}").unwrap(),
            Chord::parse("cmd-shift-]").unwrap()
        );
        assert_eq!(
            Chord::from_keystroke(&press("cmd-shift-}")),
            Chord::parse("cmd-shift-]")
        );
        assert_eq!(Chord::parse("cmd-shift-d").unwrap().display(), "⇧⌘D");
        assert_eq!(Chord::parse("cmd--").unwrap().display(), "⌘-");
        assert_eq!(Chord::parse("hyper-t"), None);
        assert_eq!(Chord::parse("cmd-"), None);
        assert_eq!(Chord::parse("cmd-shift"), None);
    }

    #[test]
    fn defaults_follow_the_terminal_apps() {
        let map = Keymap::default();
        for (keys, action) in [
            ("cmd-t", Action::NewTab),
            ("cmd-w", Action::Close),
            ("cmd-d", Action::SplitRight),
            ("cmd-shift-d", Action::SplitDown),
            ("cmd-shift-[", Action::PrevTab),
            ("cmd-shift-}", Action::NextTab),
            ("cmd-[", Action::PrevPane),
            ("cmd-]", Action::NextPane),
            ("cmd-1", Action::Tab(1)),
            ("cmd-8", Action::Tab(8)),
            ("cmd-9", Action::LastTab),
            ("cmd-p", Action::Search),
            ("cmd-,", Action::Settings),
            ("cmd-j", Action::NextWaiting),
            ("cmd-f", Action::Find),
            ("cmd-k", Action::Clear),
            ("cmd-=", Action::ZoomIn),
            ("cmd-shift-=", Action::ZoomIn),
            ("cmd--", Action::ZoomOut),
            ("cmd-0", Action::ZoomReset),
        ] {
            assert_eq!(map.action_for(&press(keys)), Some(action), "{keys}");
        }
        // Copy, paste and everything without ⌘ stay out of the keymap.
        for keys in [
            "cmd-c",
            "cmd-v",
            "ctrl-t",
            "ctrl-d",
            "alt-f",
            "t",
            "cmd-shift-t",
        ] {
            assert_eq!(map.action_for(&press(keys)), None, "{keys}");
        }
        let mut seen = vec![];
        for (a, chords) in &map.bindings {
            for c in chords {
                assert!(a.fixed() || c.refusal().is_none(), "{}", c.text());
                assert!(!seen.contains(c), "{}", c.text());
                seen.push(c.clone());
            }
        }
    }

    #[test]
    fn tab_numbers_are_fixed_and_old_files_still_load() {
        assert!(Keymap::actions().all(|a| !a.fixed()));
        assert!(Keymap::actions().any(|a| a == Action::NextPane));
        let refused = Chord::parse("cmd-3").unwrap().refusal();
        assert_eq!(refused, Some("⌘1–⌘9 固定用于切换标签"));
        assert!(Chord::parse("cmd-shift-3").unwrap().refusal().is_none());
        // An older file: its tab-number entries and a move onto ⌘1 are skipped, the
        // rest still applies.
        let old = r#"{"tab_1":["cmd-shift-1"],"last_tab":["cmd-0"],"close":["cmd-1"],"new_tab":["cmd-shift-t"]}"#;
        let map = Keymap::parse(old).unwrap();
        assert_eq!(map.action_for(&press("cmd-1")), Some(Action::Tab(1)));
        assert_eq!(map.action_for(&press("cmd-9")), Some(Action::LastTab));
        assert_eq!(map.action_for(&press("cmd-w")), Some(Action::Close));
        assert_eq!(map.action_for(&press("cmd-0")), Some(Action::ZoomReset));
        assert_eq!(map.action_for(&press("cmd-shift-t")), Some(Action::NewTab));
        assert_eq!(
            map.to_json(),
            r#"{
  "new_tab": [
    "cmd-shift-t"
  ]
}"#
        );
        let mut map = Keymap::default();
        map.assign(Action::NextPane, Chord::parse("cmd-shift-p").unwrap());
        assert_eq!(Keymap::parse(&map.to_json()).unwrap(), map);
    }

    #[test]
    fn assigning_a_taken_chord_moves_it_and_reset_takes_it_back() {
        let mut map = Keymap::default();
        let chord = Chord::parse("cmd-d").unwrap();
        assert_eq!(map.holder(Action::NewTab, &chord), Some(Action::SplitRight));
        map.assign(Action::NewTab, chord.clone());
        assert_eq!(map.action_for(&press("cmd-d")), Some(Action::NewTab));
        assert_eq!(map.action_for(&press("cmd-t")), None);
        assert!(map.chords(Action::SplitRight).is_empty());
        map.reset(Action::SplitRight);
        assert_eq!(map.action_for(&press("cmd-d")), Some(Action::SplitRight));
        assert!(map.chords(Action::NewTab).is_empty());
        map.reset(Action::NewTab);
        assert_eq!(map, Keymap::default());
    }

    #[test]
    fn only_changes_are_written_and_read_back() {
        assert_eq!(Keymap::default().to_json(), "{}");
        let mut map = Keymap::default();
        map.assign(Action::NewTab, Chord::parse("cmd-shift-t").unwrap());
        map.assign(Action::SplitDown, Chord::parse("cmd-d").unwrap());
        let json = map.to_json();
        let read = Keymap::parse(&json).unwrap();
        assert_eq!(read, map);
        assert!(json.contains("new_tab") && json.contains("split_right"));
        assert!(!json.contains("tab_1"));
        // A file that names one action alone gets that chord moved off its default owner.
        let read = Keymap::parse(r#"{"new_tab":["cmd-d"]}"#).unwrap();
        assert_eq!(read.action_for(&press("cmd-d")), Some(Action::NewTab));
        assert!(read.chords(Action::SplitRight).is_empty());
    }

    #[test]
    fn bad_files_are_refused_whole() {
        for text in [
            r#"{"new_tabb":["cmd-t"]}"#,
            r#"{"new_tab":"cmd-t"}"#,
            r#"{"new_tab":["ctrl-t"]}"#,
            r#"{"new_tab":["cmd-c"]}"#,
            r#"{"new_tab":["cmd-x"],"close":["cmd-x"]}"#,
            "[]",
        ] {
            assert!(Keymap::parse(text).is_err(), "{text}");
        }
        assert_eq!(Keymap::parse("{}").unwrap(), Keymap::default());
    }
}
