//! A tab's only placement authority. Ratios use basis points for exact persistence.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Right,
    Down,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    Leaf(String),
    Split {
        axis: Axis,
        ratio: u16,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}
impl Layout {
    pub fn ids(&self) -> Vec<String> {
        match self {
            Self::Leaf(id) => vec![id.clone()],
            Self::Split { first, second, .. } => {
                let mut ids = first.ids();
                ids.extend(second.ids());
                ids
            }
        }
    }
    pub fn contains(&self, id: &str) -> bool {
        match self {
            Self::Leaf(v) => v == id,
            Self::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }
    pub fn split(&mut self, target: &str, new: String, axis: Axis) -> bool {
        match self {
            Self::Leaf(id) if id == target => {
                *self = Self::Split {
                    axis,
                    ratio: 5000,
                    first: Box::new(self.clone()),
                    second: Box::new(Self::Leaf(new)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                if first.contains(target) {
                    first.split(target, new, axis)
                } else {
                    second.split(target, new, axis)
                }
            }
            _ => false,
        }
    }
    pub fn replace(&mut self, target: &str, new: &str) -> bool {
        match self {
            Self::Leaf(id) if id == target => {
                *id = new.into();
                true
            }
            Self::Split { first, second, .. } => {
                first.replace(target, new) || second.replace(target, new)
            }
            _ => false,
        }
    }
    pub fn without(self, target: &str) -> Option<Self> {
        match self {
            Self::Leaf(id) => (id != target).then_some(Self::Leaf(id)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.without(target), second.without(target)) {
                (Some(a), Some(b)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (a, b) => a.or(b),
            },
        }
    }
    pub fn at_mut(&mut self, path: &[bool]) -> Option<&mut Self> {
        if path.is_empty() {
            return Some(self);
        }
        match self {
            Self::Split { first, second, .. } => {
                if path[0] { second } else { first }.at_mut(&path[1..])
            }
            _ => None,
        }
    }
    pub fn valid_ratios(&self) -> bool {
        match self {
            Self::Leaf(_) => true,
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => (1..10000).contains(ratio) && first.valid_ratios() && second.valid_ratios(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_split_close_preserves_sibling_ratio_and_replace_keeps_geometry() {
        let mut tree = Layout::Leaf("a".into());
        assert!(tree.split("a", "b".into(), Axis::Right));
        assert!(tree.split("b", "c".into(), Axis::Down));
        if let Some(Layout::Split { ratio, .. }) = tree.at_mut(&[true]) {
            *ratio = 6700;
        }
        let before = tree.clone();
        assert!(tree.replace("c", "d"));
        assert_eq!(tree.ids(), ["a", "b", "d"]);
        let reduced = tree.without("a").unwrap();
        assert!(matches!(
            reduced,
            Layout::Split {
                axis: Axis::Down,
                ratio: 6700,
                ..
            }
        ));
        assert_eq!(before.ids(), ["a", "b", "c"]);
    }
    #[test]
    fn eight_and_more_panes_have_no_fixed_cap() {
        let mut tree = Layout::Leaf("0".into());
        for n in 1..32 {
            assert!(tree.split(&(n - 1).to_string(), n.to_string(), Axis::Right));
        }
        assert_eq!(tree.ids().len(), 32);
        assert!(tree.valid_ratios());
        for n in 0..31 {
            tree = tree.without(&n.to_string()).unwrap();
        }
        assert_eq!(tree, Layout::Leaf("31".into()));
        assert_eq!(tree.without("31"), None);
    }
}
