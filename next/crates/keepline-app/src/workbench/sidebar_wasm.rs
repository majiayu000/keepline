//! Left-column presets arranged by a plugin's WebAssembly module (`"group": "wasm"`).
//!
//! The module runs in `keepline_plugin_host`'s sandbox: no files, network or processes,
//! each call time- and memory-limited. It is handed the sessions the column would list
//! (title, state, times, where each stopped; never a transcript), plus the latest closed
//! ones, and answers with sections of session, folder and note rows. Keepline draws the answer in its own style; a session the
//! module names that the column does not list is ignored, and any failure falls back to
//! the classic list with the reason shown.
use super::sidebar_list::{End, Listed, bucket};
use super::*;
use keepline_core::sidebar::{Bucket, Preset};
use keepline_plugin_host as host;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::OnceLock;

/// Adds one session row to the column, within the preset's row limit.
pub(super) type AddRow<'a> =
    dyn FnMut(&mut Vec<gpui::AnyElement>, Listed, Option<String>, End) + 'a;
/// How many closed sessions a module is handed, newest first.
pub(super) const CLOSED_SHOWN: usize = 50;
/// The longest trailing text a module may put at a row's end.
const TRAILING_CHARS: usize = 12;

#[derive(Serialize)]
struct State<'a> {
    sessions: Vec<StateSession<'a>>,
    /// Every closed session in the space; `sessions` carries only the newest of them.
    closed_total: usize,
    now: u64,
}
#[derive(Serialize)]
struct StateSession<'a> {
    id: &'a str,
    title: String,
    agent: &'a str,
    group: &'static str,
    cwd: &'a str,
    updated: u64,
    running_since: Option<u64>,
    stopped: Option<String>,
    done: Option<String>,
    needs_you: bool,
    closed: bool,
    selected: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Reply {
    sections: Vec<Section>,
}
#[derive(Clone, Debug, Deserialize)]
struct Section {
    id: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    count: Option<usize>,
    #[serde(default)]
    folded_default: bool,
    #[serde(default)]
    rows: Vec<Row>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
enum Row {
    Session {
        id: String,
        #[serde(default)]
        detail: Option<String>,
        #[serde(default)]
        detail_mode: Option<String>,
        #[serde(default)]
        trailing: Trailing,
    },
    /// A folder header: it folds the session rows after it, up to the next folder or note.
    Folder {
        cwd: String,
        #[serde(default)]
        folded_default: bool,
    },
    Note {
        text: String,
        #[serde(default)]
        action: Option<String>,
    },
}

/// The closed sessions a module is handed: the newest few, and how many there are.
#[derive(Default)]
pub(super) struct Closed<'a> {
    pub rows: Vec<Listed<'a>>,
    pub total: usize,
}

/// What a session row ends with. Keepline draws it, so a running clock ticks without
/// asking the module again: `"state"` (the default), `"age"`, `"elapsed"`, `"none"` or
/// `{"text": "…"}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Trailing {
    #[default]
    State,
    Age,
    /// How long the turn has run; a session not running shows its age.
    Elapsed,
    None,
    #[serde(untagged)]
    Text {
        text: String,
    },
}

/// One module, loaded on first use, and its last answer: an unchanged state is not
/// asked again, so redraws cost a byte comparison.
pub(super) struct Loaded {
    plugin: Result<host::Plugin, String>,
    last: Option<(Vec<u8>, Result<Reply, String>)>,
}
#[derive(Default)]
pub(super) struct Views(RefCell<HashMap<String, Loaded>>);

fn engine() -> Result<&'static host::Engine, String> {
    static ENGINE: OnceLock<Result<host::Engine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            let engine = host::engine().map_err(|e| e.to_string())?;
            host::start_ticker(&engine);
            Ok(engine)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn load(preset: &Preset) -> Result<host::Plugin, String> {
    let path = preset.wasm_path().ok_or("没有 wasm 文件")?;
    let bytes = std::fs::read(&path).map_err(|e| format!("读不了 {}：{e}", path.display()))?;
    let engine = engine()?;
    let module = host::module(engine, &bytes).map_err(|e| format!("不是可用的 wasm：{e}"))?;
    host::Plugin::new(engine, &module).map_err(|e| format!("加载失败：{e}"))
}

/// The state string a session's group goes by, the same coarse words as the spec.
fn group_name(g: Group) -> &'static str {
    match g {
        Group::Waiting => "waiting",
        Group::Error => "error",
        Group::Working | Group::Running => "running",
        Group::Done | Group::Review | Group::Ready => "review",
        Group::Paused => "paused",
        Group::Stale => "stale",
    }
}

impl Workbench {
    fn wasm_state(&self, open: &[Listed], closed: &Closed, current: Option<&str>) -> Vec<u8> {
        let first = |t: &str| {
            t.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(|l| l.chars().take(T::TAB_TIP_REASON_CHARS).collect::<String>())
        };
        let state = State {
            sessions: open
                .iter()
                .map(|l| (l, false))
                .chain(closed.rows.iter().map(|l| (l, true)))
                .map(|((s, g, _), closed)| {
                    let content = self.workspace.content.get(&s.id);
                    let b = bucket(*g);
                    StateSession {
                        id: &s.id,
                        title: self.session_title(s),
                        agent: s.runtime.name(),
                        group: group_name(*g),
                        cwd: &s.cwd,
                        updated: self.session_updated(s),
                        running_since: content
                            .filter(|_| b == Bucket::Running)
                            .map(|c| c.turn_started)
                            .filter(|t| *t > 0),
                        stopped: match b {
                            Bucket::Attention => self.attention_reason(s, *g),
                            Bucket::Stopped => content.and_then(|c| first(&c.last_reply)),
                            _ => None,
                        },
                        done: content
                            .filter(|_| b == Bucket::Done)
                            .and_then(|c| first(&c.last_reply)),
                        needs_you: b == Bucket::Attention,
                        closed,
                        selected: current == Some(s.id.as_str()),
                    }
                })
                .collect(),
            closed_total: closed.total,
            // Whole minutes, so a clock tick alone never asks the module again.
            now: now() / 60 * 60,
        };
        serde_json::to_vec(&state).unwrap_or_default()
    }
    /// The module's sections for this state, from its last answer when nothing changed.
    fn wasm_reply(
        &self,
        preset: &Preset,
        open: &[Listed],
        closed: &Closed,
        current: Option<&str>,
    ) -> Result<Reply, String> {
        let state = self.wasm_state(open, closed, current);
        let mut views = self.sidebar.wasm.0.borrow_mut();
        let view = views.entry(preset.id.clone()).or_insert_with(|| Loaded {
            plugin: load(preset),
            last: None,
        });
        if let Some((input, reply)) = &view.last
            && *input == state
        {
            return reply.clone();
        }
        let plugin = view.plugin.as_mut().map_err(|e| e.clone())?;
        let reply = plugin
            .render(&state)
            .map_err(|e| format!("调用出错：{e}"))
            .and_then(|bytes| {
                serde_json::from_slice::<Reply>(&bytes).map_err(|e| format!("回复格式不对：{e}"))
            });
        view.last = Some((state, reply.clone()));
        reply
    }
    /// The rows for a `wasm` preset, or the classic list under the reason it failed.
    /// Returns whether the module's answer is shown: it then owns the closed sessions too.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wasm_rows(
        &self,
        preset: &Preset,
        sessions: Vec<Listed>,
        closed: &Closed,
        current: Option<&str>,
        push: &mut AddRow,
        out: &mut Vec<gpui::AnyElement>,
        cx: &Context<Self>,
    ) -> bool {
        let reply = match self.wasm_reply(preset, &sessions, closed, current) {
            Ok(r) => r,
            Err(why) => {
                out.push(
                    label(format!(
                        "左栏「{}」没能排出来，先按经典显示：{why}",
                        preset.name
                    ))
                    .px(px(T::S8))
                    .mt(px(T::S12))
                    .text_size(px(T::CHIP_SIZE))
                    .text_color(Group::Error.colors().1)
                    .into_any_element(),
                );
                for l in sessions {
                    push(out, l, None, End::State);
                }
                return false;
            }
        };
        let find = |id: &str| {
            sessions
                .iter()
                .chain(closed.rows.iter())
                .find(|l| l.0.id == id)
                .copied()
        };
        for section in reply.sections {
            let key = format!("wasm:{}:{}", preset.id, section.id);
            let open = self.sidebar.open(&key, !section.folded_default);
            let named = section
                .rows
                .iter()
                .filter(|r| matches!(r, Row::Session { id, .. } if find(id).is_some()))
                .count();
            let notes = section.rows.iter().any(|r| matches!(r, Row::Note { .. }));
            if named == 0 && !notes {
                continue;
            }
            if let Some(title) = section.title.clone() {
                out.push(
                    self.heading(title, section.count.unwrap_or(named), cx)
                        .into_any_element(),
                );
            }
            // Whether the folder the next session rows sit under is open.
            let mut in_open_folder = true;
            for (i, row) in section.rows.iter().enumerate() {
                match row {
                    Row::Note { text, action } => {
                        in_open_folder = true;
                        out.push(self.wasm_note(
                            format!("{key}:{i}"),
                            &key,
                            text.clone(),
                            action.as_deref(),
                            open,
                            cx,
                        ))
                    }
                    Row::Folder {
                        cwd,
                        folded_default,
                    } => {
                        let under: Vec<Listed> = section.rows[i + 1..]
                            .iter()
                            .take_while(|r| matches!(r, Row::Session { .. }))
                            .filter_map(|r| match r {
                                Row::Session { id, .. } => find(id),
                                _ => None,
                            })
                            .collect();
                        let folder = format!("{key}:dir:{cwd}");
                        in_open_folder = self.sidebar.open(&folder, !folded_default);
                        if !open || under.is_empty() {
                            continue;
                        }
                        let needs = under
                            .iter()
                            .filter(|l| bucket(l.1) == Bucket::Attention)
                            .count();
                        let running = under.iter().any(|l| bucket(l.1) == Bucket::Running);
                        out.push(self.folder_row(
                            cwd,
                            &folder,
                            under.len(),
                            needs,
                            in_open_folder,
                            running,
                            cx,
                        ));
                    }
                    Row::Session {
                        id,
                        detail,
                        detail_mode,
                        trailing,
                    } => {
                        let Some(l) = find(id) else {
                            continue;
                        };
                        // The session you are in stays in sight while its section is folded.
                        let selected = current == Some(id.as_str());
                        if !(open && in_open_folder) && !selected {
                            continue;
                        }
                        let shown = match detail_mode.as_deref() {
                            Some("always") => detail.clone(),
                            Some("selected") if selected => detail.clone(),
                            _ => None,
                        };
                        push(out, l, shown, self.wasm_end(l, trailing, cx));
                    }
                }
            }
        }
        true
    }
    /// When the running turn began, for a session whose turn is running.
    pub(super) fn turn_started(&self, s: &AgentSession) -> Option<u64> {
        self.workspace
            .content
            .get(&s.id)
            .filter(|c| c.turn == keepline_core::content::Turn::Running && c.turn_started > 0)
            .map(|c| c.turn_started)
    }
    fn wasm_end(&self, (s, _, _): Listed, trailing: &Trailing, cx: &Context<Self>) -> End {
        let ago = || match age(self.session_updated(s)) {
            a if a == "刚刚" => a,
            a => format!("{a}前"),
        };
        match trailing {
            Trailing::State => End::State,
            Trailing::None => End::Nothing,
            Trailing::Age => End::Text(ago()),
            Trailing::Elapsed => match self.turn_started(s) {
                Some(at) => {
                    self.tick_turn(cx);
                    End::Text(super::layout::turn_clock(now().saturating_sub(at)))
                }
                None => End::Text(ago()),
            },
            Trailing::Text { text } => End::Text(text.chars().take(TRAILING_CHARS).collect()),
        }
    }
    /// A note row: plain text, or a way to fold its section, open ⌘P, 鸟瞰 or the closed
    /// sessions there.
    fn wasm_note(
        &self,
        id: String,
        section: &str,
        text: String,
        action: Option<&str>,
        open: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let row = div()
            .id(gpui::SharedString::from(format!("sidebar-note-{id}")))
            .min_h(px(T::ROW))
            .px(px(T::S8))
            .flex()
            .items_center()
            .gap(px(T::S6))
            .rounded(px(T::CONTROL_RADIUS))
            .text_size(px(T::CHIP_SIZE))
            .text_color(T::color(T::muted()))
            .child(div().flex_1().min_w_0().truncate().child(text));
        let section = section.to_owned();
        match action {
            Some("toggle") => row
                .cursor_pointer()
                .hover(|s| s.bg(T::color(T::raised())))
                .child(if open { "⌄" } else { "›" })
                .on_click(
                    cx.listener(move |this, _, _, cx| this.toggle_sidebar_group(&section, cx)),
                )
                .into_any_element(),
            Some("palette") => row
                .cursor_pointer()
                .hover(|s| s.bg(T::color(T::raised())))
                .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx)))
                .into_any_element(),
            Some("overview") => row
                .cursor_pointer()
                .hover(|s| s.bg(T::color(T::raised())))
                .on_click(cx.listener(|this, _, _, cx| this.show_overview(cx)))
                .into_any_element(),
            Some("closed") => row
                .cursor_pointer()
                .hover(|s| s.bg(T::color(T::raised())))
                .on_click(cx.listener(|this, _, _, cx| this.show_closed(cx)))
                .into_any_element(),
            _ => row.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session<'a>(id: &'a str, group: &'static str) -> StateSession<'a> {
        StateSession {
            id,
            title: format!("会话 {id}"),
            agent: "codex",
            group,
            cwd: "/project",
            updated: 10,
            running_since: (group == "running").then_some(5),
            stopped: (group == "waiting").then(|| "要不要推送".into()),
            done: (group == "review").then(|| "改好了".into()),
            needs_you: group == "waiting",
            closed: false,
            selected: false,
        }
    }

    #[test]
    fn the_shipped_wasm_preset_sorts_sessions_into_sections() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/sidebar-status");
        let (presets, report) = keepline_core::sidebar::load(vec![dir.join("sidebar.json")]);
        assert!(report[0].result.is_ok(), "{report:?}");
        let mut plugin = load(presets.last().unwrap()).unwrap();
        let mut gone = session("z", "paused");
        gone.closed = true;
        let mut elsewhere = session("d", "paused");
        elsewhere.cwd = "/other";
        let state = State {
            sessions: vec![
                session("a", "waiting"),
                session("b", "running"),
                session("c", "review"),
                elsewhere,
                gone,
            ],
            closed_total: 9,
            now: 100,
        };
        let bytes = plugin.render(&serde_json::to_vec(&state).unwrap()).unwrap();
        let reply: Reply = serde_json::from_slice(&bytes).unwrap();
        let section = |id: &str| reply.sections.iter().find(|s| s.id == id).unwrap();
        let ids = |id: &str| -> Vec<String> {
            section(id)
                .rows
                .iter()
                .filter_map(|r| match r {
                    Row::Session { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(ids("need"), ["a"]);
        assert_eq!(ids("running"), ["b"]);
        assert_eq!(ids("rest"), ["c", "d"]);
        assert_eq!(ids("closed"), ["z"]);
        assert!(matches!(
            &section("running").rows[0],
            Row::Session {
                trailing: Trailing::Elapsed,
                ..
            }
        ));
        let rest = section("rest");
        assert!(rest.folded_default);
        assert!(matches!(&rest.rows[0], Row::Note { action: Some(a), .. } if a == "toggle"));
        let folders: Vec<&str> = rest
            .rows
            .iter()
            .filter_map(|r| match r {
                Row::Folder { cwd, .. } => Some(cwd.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(folders, ["/project", "/other"]);
        // More closed than it lists: the last row points at the rest in 鸟瞰.
        assert!(matches!(
            section("closed").rows.last(),
            Some(Row::Note { action: Some(a), text }) if a == "closed" && text.contains('9')
        ));
    }

    #[test]
    fn row_ends_are_a_few_known_words_or_a_text() {
        let row = |end: &str| {
            serde_json::from_str::<Row>(&format!(r#"{{"kind":"Session","id":"a"{end}}}"#)).map(
                |r| match r {
                    Row::Session { trailing, .. } => trailing,
                    _ => unreachable!(),
                },
            )
        };
        assert_eq!(row("").unwrap(), Trailing::State);
        assert_eq!(row(r#","trailing":"elapsed""#).unwrap(), Trailing::Elapsed);
        assert_eq!(row(r#","trailing":"none""#).unwrap(), Trailing::None);
        assert_eq!(
            row(r#","trailing":{"text":"PR #12"}"#).unwrap(),
            Trailing::Text {
                text: "PR #12".into()
            }
        );
        assert!(row(r#","trailing":"clock""#).is_err());
    }

    #[test]
    fn a_module_that_is_not_wasm_is_refused_with_a_reason() {
        let d = std::env::temp_dir().join(format!("kl-wasm-bad-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("bad.wasm"), b"not wasm").unwrap();
        std::fs::write(
            d.join("sidebar.json"),
            r#"{"api":1,"presets":[{"id":"bad","name":"坏","group":"wasm","wasm":"bad.wasm"}]}"#,
        )
        .unwrap();
        let (presets, _) = keepline_core::sidebar::load(vec![d.join("sidebar.json")]);
        let err = load(presets.last().unwrap()).err().unwrap();
        assert!(err.contains("不是可用的 wasm"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
