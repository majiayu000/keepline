//! Left-column presets arranged by a plugin's WebAssembly module (`"group": "wasm"`).
//!
//! The module runs in `keepline_plugin_host`'s sandbox: no files, network or processes,
//! each call time- and memory-limited. It is handed the sessions the column would list
//! (title, state, times, where each stopped; never a transcript) and answers with sections
//! of those sessions and notes. Keepline draws the answer in its own style; a session the
//! module names that the column does not list is ignored, and any failure falls back to
//! the classic list with the reason shown.
use super::sidebar_list::{Listed, bucket};
use super::*;
use keepline_core::sidebar::{Bucket, Preset};
use keepline_plugin_host as host;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::OnceLock;

/// Adds one session row to the column, within the preset's row limit.
pub(super) type AddRow<'a> = dyn FnMut(&mut Vec<gpui::AnyElement>, Listed, Option<String>) + 'a;

#[derive(Serialize)]
struct State<'a> {
    sessions: Vec<StateSession<'a>>,
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
    },
    Note {
        text: String,
        #[serde(default)]
        action: Option<String>,
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
    fn wasm_state(&self, sessions: &[Listed], current: Option<&str>) -> Vec<u8> {
        let first = |t: &str| {
            t.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(|l| l.chars().take(T::TAB_TIP_REASON_CHARS).collect::<String>())
        };
        let state = State {
            sessions: sessions
                .iter()
                .map(|(s, g, _)| {
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
                        closed: false,
                        selected: current == Some(s.id.as_str()),
                    }
                })
                .collect(),
            // Whole minutes, so a clock tick alone never asks the module again.
            now: now() / 60 * 60,
        };
        serde_json::to_vec(&state).unwrap_or_default()
    }
    /// The module's sections for this state, from its last answer when nothing changed.
    fn wasm_reply(
        &self,
        preset: &Preset,
        sessions: &[Listed],
        current: Option<&str>,
    ) -> Result<Reply, String> {
        let state = self.wasm_state(sessions, current);
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
    pub(super) fn wasm_rows(
        &self,
        preset: &Preset,
        sessions: Vec<Listed>,
        current: Option<&str>,
        push: &mut AddRow,
        out: &mut Vec<gpui::AnyElement>,
        cx: &Context<Self>,
    ) {
        let reply = match self.wasm_reply(preset, &sessions, current) {
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
                    push(out, l, None);
                }
                return;
            }
        };
        for section in reply.sections {
            let key = format!("wasm:{}:{}", preset.id, section.id);
            let open = self.sidebar.open(&key, !section.folded_default);
            let named: Vec<&Listed> = section
                .rows
                .iter()
                .filter_map(|r| match r {
                    Row::Session { id, .. } => sessions.iter().find(|l| &l.0.id == id),
                    Row::Note { .. } => None,
                })
                .collect();
            let notes = section.rows.iter().any(|r| matches!(r, Row::Note { .. }));
            if named.is_empty() && !notes {
                continue;
            }
            if let Some(title) = section.title.clone() {
                out.push(
                    Self::section_title(title, section.count.unwrap_or(named.len()))
                        .into_any_element(),
                );
            }
            for (i, row) in section.rows.iter().enumerate() {
                match row {
                    Row::Note { text, action } => out.push(self.wasm_note(
                        format!("{key}:{i}"),
                        &key,
                        text.clone(),
                        action.as_deref(),
                        open,
                        cx,
                    )),
                    Row::Session {
                        id,
                        detail,
                        detail_mode,
                    } => {
                        let Some(l) = sessions.iter().find(|l| &l.0.id == id) else {
                            continue;
                        };
                        // The session you are in stays in sight while its section is folded.
                        let selected = current == Some(id.as_str());
                        if !open && !selected {
                            continue;
                        }
                        let shown = match detail_mode.as_deref() {
                            Some("always") => detail.clone(),
                            Some("selected") if selected => detail.clone(),
                            _ => None,
                        };
                        push(out, *l, shown);
                    }
                }
            }
        }
    }
    /// A note row: plain text, or a way to fold its section, open ⌘P or 鸟瞰.
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
        let state = State {
            sessions: vec![
                session("a", "waiting"),
                session("b", "running"),
                session("c", "review"),
            ],
            now: 100,
        };
        let bytes = plugin.render(&serde_json::to_vec(&state).unwrap()).unwrap();
        let reply: Reply = serde_json::from_slice(&bytes).unwrap();
        let ids = |section: &str| -> Vec<String> {
            reply
                .sections
                .iter()
                .find(|s| s.id == section)
                .map(|s| {
                    s.rows
                        .iter()
                        .filter_map(|r| match r {
                            Row::Session { id, .. } => Some(id.clone()),
                            Row::Note { .. } => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        assert_eq!(ids("need"), ["a"]);
        assert_eq!(ids("running"), ["b"]);
        assert_eq!(ids("rest"), ["c"]);
        let rest = reply.sections.iter().find(|s| s.id == "rest").unwrap();
        assert!(rest.folded_default);
        assert!(matches!(&rest.rows[0], Row::Note { action: Some(a), .. } if a == "toggle"));
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
