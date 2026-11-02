//! The JSON lines of the remote protocol, version [`VERSION`]: one object per line each
//! way, a line at most [`MAX_LINE`] bytes. A client sends [`Request`]s, `hello` first;
//! every line back is an [`Incoming`]. Unknown fields are ignored both ways, so either
//! side may add some without a new version.
use crate::role::Role;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const VERSION: u32 = 1;
pub const MAX_LINE: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub op: Op,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    Hello {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<u32>,
    },
    /// Replies [`List`].
    List,
    /// The last `lines` (default 50, at most 2000) of a screen as plain text: [`Text`].
    Read {
        terminal: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lines: Option<u64>,
    },
    /// Pastes `text`; `enter` then presses Enter.
    Send {
        terminal: String,
        text: String,
        #[serde(default)]
        enter: bool,
    },
    /// Presses named keys, all or none: `enter` `esc` `tab` `shift-tab` `backspace`
    /// `space` `up` `down` `left` `right` `home` `end` `page-up` `page-down` `delete`,
    /// `ctrl-a`…`ctrl-z`, or any single character.
    Keys {
        terminal: String,
        keys: Vec<String>,
    },
    /// Answers the approval asking `question`; refused if it is no longer on screen.
    Answer {
        terminal: String,
        question: String,
        allow: bool,
    },
    /// A conversation as chat: [`Chat`]. Names the `terminal` running it (its current
    /// conversation) or the `session`. Without `after` or `before`, the newest `limit`
    /// (default 50, at most 500); `after` gives those newer than a `seq`, `before` older.
    Chat {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        terminal: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        limit: Option<u64>,
    },
    /// Opens a stream of [`StreamBody::Event`]s, the current state first.
    Events,
    /// Opens a stream of one terminal's screen: [`StreamBody::Screen`], then output.
    Watch {
        terminal: String,
    },
    Close {
        stream: u64,
    },
}

impl Op {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "hello",
            Self::List => "list",
            Self::Read { .. } => "read",
            Self::Send { .. } => "send",
            Self::Keys { .. } => "keys",
            Self::Answer { .. } => "answer",
            Self::Chat { .. } => "chat",
            Self::Events => "events",
            Self::Watch { .. } => "watch",
            Self::Close { .. } => "close",
        }
    }
}

/// Every op, in the order `hello` lists them.
pub const OPS: &[&str] = &[
    "hello", "list", "read", "send", "keys", "answer", "chat", "events", "watch", "close",
];

/// A line from the machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Stream(StreamMessage),
    /// `ok`'s shape depends on the op: [`Hello`], [`List`], [`Text`], [`Opened`], or `{}`.
    Reply {
        id: u64,
        ok: Value,
    },
    Failed {
        id: u64,
        error: String,
    },
    /// A line that could not be read as a request.
    Error {
        error: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamMessage {
    /// The id of the request that opened the stream.
    pub stream: u64,
    #[serde(flatten)]
    pub body: StreamBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamBody {
    Event {
        event: Event,
    },
    /// The whole screen: base64 escape sequences that rebuild it, cursor, modes and
    /// alternate screen included, in a fresh emulator. Sent again after falling behind.
    Screen {
        cols: u16,
        rows: u16,
        vt: String,
    },
    /// Output after the screen, base64; feed it on.
    Output {
        vt: String,
    },
    /// Another client changed the size.
    Resized {
        cols: u16,
        rows: u16,
    },
    Exited {
        #[serde(default)]
        code: Option<i32>,
    },
    End {
        reason: String,
    },
}

/// One change, as `keepline events --json` prints it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// started, waiting, error, done, approval, approval_gone, terminal_started,
    /// terminal_exited, command_done, command_failed, or lagged (events were lost:
    /// `list` again).
    pub event: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    /// Stays the same across restarts.
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub version: u32,
    pub machine: Machine,
    /// The ops this client may use.
    pub ops: Vec<String>,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct List {
    pub machine: Machine,
    pub spaces: Vec<Space>,
    pub terminals: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub cwd: String,
}

/// A running terminal and what its Agent is doing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub space: String,
    pub tab: String,
    pub cwd: String,
    pub phase: Phase,
    pub agent: Option<String>,
    pub session: Option<String>,
    pub title: Option<String>,
    /// The approval it asks, when it asks one.
    pub approval: Option<String>,
    /// What it waits on the user for.
    pub waiting: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Working,
    Waiting,
    Idle,
    /// No Agent conversation: a shell or another program.
    Shell,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
}

/// The reply to `chat`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chat {
    pub session: String,
    /// Oldest first.
    pub messages: Vec<Message>,
    /// Older messages exist before the first: ask again with `before`.
    pub more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Grows with each message; the cursor for `after` and `before`.
    pub seq: i64,
    /// Seconds since the epoch.
    pub at: u64,
    pub role: Speaker,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    User,
    Agent,
}

/// The reply to `events` and `watch`: the stream is open.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Opened {
    pub stream: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_are_flat_objects_tagged_by_op() {
        let r = Request {
            id: 7,
            op: Op::Keys {
                terminal: "rt".into(),
                keys: vec!["up".into()],
            },
        };
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({ "id": 7, "op": "keys", "terminal": "rt", "keys": ["up"] })
        );
        let r: Request = serde_json::from_value(json!({ "id": 1, "op": "list" })).unwrap();
        assert_eq!(r.op, Op::List);
        let r: Request =
            serde_json::from_value(json!({ "id": 2, "op": "send", "terminal": "t", "text": "hi" }))
                .unwrap();
        assert_eq!(r.op.name(), "send");
        assert!(OPS
            .iter()
            .all(|op| op.chars().all(|c| c.is_ascii_lowercase())));
    }

    #[test]
    fn incoming_lines_sort_into_replies_failures_and_streams() {
        let line = |v: Value| serde_json::from_value::<Incoming>(v).unwrap();
        assert!(matches!(
            line(json!({ "id": 1, "ok": {} })),
            Incoming::Reply { id: 1, .. }
        ));
        assert!(matches!(
            line(json!({ "id": 1, "error": "x" })),
            Incoming::Failed { .. }
        ));
        assert!(matches!(
            line(json!({ "error": "x" })),
            Incoming::Error { .. }
        ));
        let Incoming::Stream(s) =
            line(json!({ "stream": 4, "type": "screen", "cols": 80, "rows": 24, "vt": "" }))
        else {
            panic!()
        };
        assert_eq!(
            s.body,
            StreamBody::Screen {
                cols: 80,
                rows: 24,
                vt: String::new()
            }
        );
        let Incoming::Stream(s) = line(
            json!({ "stream": 8, "type": "event", "event": { "event": "done", "terminal": "t", "at": 1 } }),
        ) else {
            panic!()
        };
        let StreamBody::Event { event } = s.body else {
            panic!()
        };
        assert_eq!(
            (event.event.as_str(), event.rest["at"].as_u64()),
            ("done", Some(1))
        );
    }
}
