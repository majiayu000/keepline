//! What a paired device may do. The machine enforces it; a client reads it from `hello`
//! to show only what will work.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Sees terminals, screens and events.
    View,
    /// Also answers approvals.
    Answer,
    /// Also types and presses keys.
    Full,
}

impl Role {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "view" => Some(Self::View),
            "answer" => Some(Self::Answer),
            "full" => Some(Self::Full),
            _ => None,
        }
    }
    /// As people read it.
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "只看",
            Self::Answer => "能答审批",
            Self::Full => "完全控制",
        }
    }
    pub fn allows(self, op: &str) -> bool {
        match op {
            "send" | "keys" => self == Self::Full,
            "answer" => self != Self::View,
            _ => true,
        }
    }
}

/// Ops that change something on the machine; the machine logs each one.
pub fn acts(op: &str) -> bool {
    matches!(op, "send" | "keys" | "answer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_allow_only_what_they_name() {
        for op in ["hello", "list", "read", "watch", "events", "close"] {
            assert!(Role::View.allows(op));
        }
        assert!(!Role::View.allows("answer") && !Role::View.allows("send"));
        assert!(Role::Answer.allows("answer") && !Role::Answer.allows("keys"));
        assert!(Role::Full.allows("send") && Role::Full.allows("keys"));
        assert_eq!(Role::parse("admin"), None);
        assert_eq!(serde_json::to_string(&Role::Answer).unwrap(), r#""answer""#);
        assert!(acts("keys") && !acts("watch"));
    }
}
