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
    /// The pane next to `from` towards `to`, as Ghostty's goto_split picks it: of the
    /// panes across that edge which share some of its length, the nearest, then the one
    /// sharing the most, then the earlier in layout order. None at the tab's edge.
    pub fn neighbour(&self, from: &str, to: Direction) -> Option<String> {
        let mut rects = Vec::new();
        self.rects([0., 0., 1., 1.], &mut rects);
        let (_, f) = rects.iter().find(|(id, _)| id == from)?;
        const EPS: f64 = 1e-9;
        // (distance across the edge, length shared along it) for a pane past that edge.
        let measure = |c: &[f64; 4]| -> Option<(f64, f64)> {
            let (gap, lo, hi, clo, chi) = match to {
                Direction::Left => (f[0] - c[2], f[1], f[3], c[1], c[3]),
                Direction::Right => (c[0] - f[2], f[1], f[3], c[1], c[3]),
                Direction::Up => (f[1] - c[3], f[0], f[2], c[0], c[2]),
                Direction::Down => (c[1] - f[3], f[0], f[2], c[0], c[2]),
            };
            let shared = hi.min(chi) - lo.max(clo);
            (gap > -EPS && shared > EPS).then_some((gap, shared))
        };
        let mut best: Option<(&str, f64, f64)> = None;
        for (id, c) in &rects {
            let Some((gap, shared)) = measure(c) else {
                continue;
            };
            let better = best.is_none_or(|(_, g, s)| {
                gap < g - EPS || ((gap - g).abs() <= EPS && shared > s + EPS)
            });
            if better {
                best = Some((id, gap, shared));
            }
        }
        best.map(|(id, ..)| id.to_owned())
    }
    /// Each pane's share of the tab as left, top, right, bottom in 0..1.
    fn rects(&self, r: [f64; 4], out: &mut Vec<(String, [f64; 4])>) {
        match self {
            Self::Leaf(id) => out.push((id.clone(), r)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let k = f64::from(*ratio) / 10000.;
                let (a, b) = match axis {
                    Axis::Right => {
                        let x = r[0] + (r[2] - r[0]) * k;
                        ([r[0], r[1], x, r[3]], [x, r[1], r[2], r[3]])
                    }
                    Axis::Down => {
                        let y = r[1] + (r[3] - r[1]) * k;
                        ([r[0], r[1], r[2], y], [r[0], y, r[2], r[3]])
                    }
                };
                first.rects(a, out);
                second.rects(b, out);
            }
        }
    }
}
/// Which way ⌘⌥ and an arrow moves input between the panes of a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn neighbour_follows_geometry_not_layout_order() {
        // a | (b over c), the right column split 30/70.
        let mut tree = Layout::Leaf("a".into());
        assert!(tree.split("a", "b".into(), Axis::Right));
        assert!(tree.split("b", "c".into(), Axis::Down));
        if let Some(Layout::Split { ratio, .. }) = tree.at_mut(&[true]) {
            *ratio = 3000;
        }
        let n = |from: &str, to| tree.neighbour(from, to);
        assert_eq!(n("c", Direction::Up).as_deref(), Some("b"));
        assert_eq!(n("b", Direction::Down).as_deref(), Some("c"));
        assert_eq!(n("b", Direction::Left).as_deref(), Some("a"));
        assert_eq!(n("c", Direction::Left).as_deref(), Some("a"));
        // From a, c shares more of a's right edge than b does.
        assert_eq!(n("a", Direction::Right).as_deref(), Some("c"));
        assert_eq!(n("a", Direction::Left), None);
        assert_eq!(n("a", Direction::Up), None);
        assert_eq!(n("b", Direction::Up), None);
        assert_eq!(n("missing", Direction::Up), None);
        assert_eq!(Layout::Leaf("a".into()).neighbour("a", Direction::Right), None);
    }
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
