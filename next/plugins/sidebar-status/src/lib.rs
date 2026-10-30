//! The 状态 sidebar view as a plugin: pins what needs you, lists what is running, and
//! folds everything finished or paused into one summary row.
//!
//! ABI for the prototype: the host writes UTF-8 JSON into memory from `alloc`, calls
//! `render(ptr, len)`, and reads the reply as a length-prefixed buffer at the returned
//! pointer (4-byte little-endian length, then JSON).
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct State {
    sessions: Vec<Session>,
    #[allow(dead_code)]
    now: u64,
}

#[derive(Deserialize)]
struct Session {
    id: String,
    title: String,
    group: String,
    updated: u64,
    #[serde(default)]
    running_since: Option<u64>,
    #[serde(default)]
    stopped: Option<String>,
    #[serde(default)]
    done: Option<String>,
    needs_you: bool,
    closed: bool,
    selected: bool,
}

#[derive(Serialize)]
struct Sidebar {
    sections: Vec<Section>,
}

#[derive(Serialize)]
struct Section {
    id: &'static str,
    title: Option<&'static str>,
    count: Option<usize>,
    folded_default: bool,
    pinned: bool,
    rows: Vec<Row>,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum Row {
    Session {
        id: String,
        glyph: String,
        title: String,
        trailing: Trailing,
        detail: Option<String>,
        detail_mode: &'static str,
    },
    Note {
        text: String,
        action: Option<&'static str>,
    },
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "value")]
enum Trailing {
    Time(u64),
    Timer(u64),
}

fn row(s: &Session, detail_mode: &'static str) -> Row {
    Row::Session {
        id: s.id.clone(),
        glyph: s.group.clone(),
        title: s.title.clone(),
        trailing: match s.running_since {
            Some(t) => Trailing::Timer(t),
            None => Trailing::Time(s.updated),
        },
        detail: s
            .stopped
            .clone()
            .map(|t| format!("停在 {t}"))
            .or_else(|| s.done.clone().map(|t| format!("做成了 {t}"))),
        detail_mode,
    }
}

fn build(state: &State) -> Sidebar {
    let open: Vec<&Session> = state.sessions.iter().filter(|s| !s.closed).collect();
    let mut need: Vec<&Session> = open.iter().copied().filter(|s| s.needs_you).collect();
    need.sort_by(|a, b| b.updated.cmp(&a.updated));
    let running: Vec<&Session> = open
        .iter()
        .copied()
        .filter(|s| !s.needs_you && s.group == "running")
        .collect();
    let count = |g: &str| open.iter().filter(|s| !s.needs_you && s.group == g).count();
    let (done, paused, stale) = (count("review"), count("paused"), count("stale"));
    let closed = state.sessions.len() - open.len();
    let mut sections = vec![
        Section {
            id: "need",
            title: Some("需要你"),
            count: Some(need.len()),
            folded_default: false,
            pinned: true,
            rows: need.iter().map(|s| row(s, "always")).collect(),
        },
        Section {
            id: "running",
            title: Some("在跑"),
            count: Some(running.len()),
            folded_default: false,
            pinned: false,
            rows: running
                .iter()
                .map(|s| row(s, if s.selected { "always" } else { "hover" }))
                .collect(),
        },
    ];
    let rest: Vec<&Session> = open
        .iter()
        .copied()
        .filter(|s| !s.needs_you && s.group != "running")
        .collect();
    sections.push(Section {
        id: "rest",
        title: None,
        count: None,
        folded_default: true,
        pinned: false,
        rows: std::iter::once(Row::Note {
            text: format!("做完了 {done} · 停下了 {paused} · 很久没动 {stale}"),
            action: Some("toggle"),
        })
        .chain(rest.iter().map(|s| row(s, "hover")))
        .collect(),
    });
    sections.push(Section {
        id: "closed",
        title: None,
        count: None,
        folded_default: false,
        pinned: false,
        rows: vec![Row::Note {
            text: format!("已关闭 {closed} · ⌘P 找回"),
            action: Some("palette"),
        }],
    });
    Sidebar { sections }
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, cap: usize) {
    // SAFETY: the host only frees buffers this module handed out with this capacity.
    unsafe { drop(Vec::from_raw_parts(ptr, 0, cap)) }
}

#[unsafe(no_mangle)]
pub extern "C" fn render(ptr: *const u8, len: usize) -> *const u8 {
    // SAFETY: the host wrote `len` bytes at `ptr`, which it obtained from `alloc`.
    let input = unsafe { std::slice::from_raw_parts(ptr, len) };
    let out = match serde_json::from_slice::<State>(input) {
        Ok(state) => serde_json::to_vec(&build(&state)).unwrap_or_default(),
        Err(e) => format!("{{\"error\":{:?}}}", e.to_string()).into_bytes(),
    };
    let mut framed = Vec::with_capacity(out.len() + 4);
    framed.extend_from_slice(&(out.len() as u32).to_le_bytes());
    framed.extend_from_slice(&out);
    let p = framed.as_ptr();
    std::mem::forget(framed);
    p
}

/// A deliberately broken entry point for the failure-isolation check: never returns.
#[unsafe(no_mangle)]
pub extern "C" fn spin() -> u32 {
    let mut i: u32 = 0;
    loop {
        i = i.wrapping_add(1);
        if i == u32::MAX {
            i = 0;
        }
        std::hint::black_box(i);
    }
}
