//! 会话优先 (specs/sidebar-directions/directions.html, direction A) as a plugin: what needs
//! you on top, then every other open session in one list, newest first. Each row's second
//! line names the session by its short id and says where it stopped or what it got done,
//! so sessions with the same title can be told apart; a row ends with a time only.
//!
//! ABI: the host writes UTF-8 JSON into memory from `alloc`, calls `render(ptr, len)`, and
//! reads the reply as a length-prefixed buffer at the returned pointer (4-byte
//! little-endian length, then JSON).
use serde::{Deserialize, Serialize};

/// How many closed sessions the closed section lists before pointing at 鸟瞰.
const CLOSED_LISTED: usize = 5;

#[derive(Deserialize)]
struct State {
    sessions: Vec<Session>,
    #[serde(default)]
    closed_total: usize,
}

#[derive(Deserialize)]
struct Session {
    id: String,
    group: String,
    updated: u64,
    #[serde(default)]
    short: String,
    #[serde(default)]
    running_since: Option<u64>,
    #[serde(default)]
    line: Option<String>,
    #[serde(default)]
    stopped: Option<String>,
    #[serde(default)]
    done: Option<String>,
    needs_you: bool,
    closed: bool,
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
    rows: Vec<Row>,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum Row {
    Session {
        id: String,
        trailing: Trailing,
        detail: Option<String>,
        detail_mode: &'static str,
    },
    Note {
        text: String,
        action: Option<&'static str>,
    },
}

/// What a row ends with; Keepline draws it, so a running clock ticks on its own.
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Trailing {
    Age,
    /// The turn's clock while it runs, its age otherwise.
    Elapsed,
}

/// The second line: the short id, then what tells this session apart right now.
fn detail(s: &Session) -> Option<String> {
    let what = if s.needs_you {
        s.stopped.clone().or_else(|| s.line.clone())
    } else {
        match s.group.as_str() {
            "review" => s.done.clone().or_else(|| s.line.clone()),
            "paused" => s.stopped.clone().or_else(|| s.line.clone()),
            _ => s.line.clone(),
        }
    };
    match (s.short.is_empty(), what) {
        (true, None) => None,
        (true, Some(w)) => Some(w),
        (false, None) => Some(s.short.clone()),
        (false, Some(w)) => Some(format!("{} · {w}", s.short)),
    }
}

fn row(s: &Session, trailing: Trailing, detail_mode: &'static str) -> Row {
    Row::Session {
        id: s.id.clone(),
        trailing,
        detail: detail(s),
        detail_mode,
    }
}

/// Newest first. A running turn keys on when it began, which holds still while it prints,
/// so two running rows never trade places under the pointer.
fn newest(s: &Session) -> std::cmp::Reverse<u64> {
    std::cmp::Reverse(if s.group == "running" {
        s.running_since.unwrap_or(s.updated)
    } else {
        s.updated
    })
}

fn build(state: &State) -> Sidebar {
    let open: Vec<&Session> = state.sessions.iter().filter(|s| !s.closed).collect();
    let mut need: Vec<&Session> = open.iter().copied().filter(|s| s.needs_you).collect();
    need.sort_by_key(|s| newest(s));
    let mut rest: Vec<&Session> = open
        .iter()
        .copied()
        .filter(|s| !s.needs_you && s.group != "stale")
        .collect();
    rest.sort_by_key(|s| newest(s));
    let mut stale: Vec<&Session> = open
        .iter()
        .copied()
        .filter(|s| !s.needs_you && s.group == "stale")
        .collect();
    stale.sort_by_key(|s| newest(s));

    let mut sections = vec![];
    if !need.is_empty() {
        sections.push(Section {
            id: "need",
            title: Some("需要你"),
            count: Some(need.len()),
            folded_default: false,
            rows: need
                .iter()
                .map(|s| row(s, Trailing::Age, "always"))
                .collect(),
        });
    }
    sections.push(Section {
        id: "rest",
        title: Some("最新在上"),
        count: Some(rest.len()),
        folded_default: false,
        rows: rest
            .iter()
            .map(|s| row(s, Trailing::Elapsed, "always"))
            .collect(),
    });
    // Gone quiet: one line, opened on demand.
    if !stale.is_empty() {
        let mut rows = vec![Row::Note {
            text: format!("很久没动 {}", stale.len()),
            action: Some("toggle"),
        }];
        rows.extend(stale.iter().map(|s| row(s, Trailing::Age, "never")));
        sections.push(Section {
            id: "stale",
            title: None,
            count: None,
            folded_default: true,
            rows,
        });
    }
    // Closed ones, newest first: a few to resume from here, the rest in 鸟瞰.
    if state.closed_total > 0 {
        let closed: Vec<&Session> = state.sessions.iter().filter(|s| s.closed).collect();
        let mut rows = vec![Row::Note {
            text: format!("已关闭 {}", state.closed_total),
            action: Some("toggle"),
        }];
        rows.extend(
            closed
                .iter()
                .take(CLOSED_LISTED)
                .map(|s| row(s, Trailing::Age, "never")),
        );
        if state.closed_total > CLOSED_LISTED {
            rows.push(Row::Note {
                text: format!("全部 {} 个 · 鸟瞰里接着聊", state.closed_total),
                action: Some("closed"),
            });
        }
        sections.push(Section {
            id: "closed",
            title: None,
            count: None,
            folded_default: true,
            rows,
        });
    }
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
