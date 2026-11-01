//! SVGs drawn by `gpui::svg()`: the board's small marks, and each CLI's own mark from the
//! agent registry. All are one fill, tinted by the element's text color.
use keepline_core::model::Agent;
use std::borrow::Cow;

pub struct Assets;

/// `icons/<name>.svg` for the app's marks; `agent/<key>` for a CLI's.
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/arc.svg",
        include_bytes!("../../assets/icons/arc.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../../assets/icons/check.svg"),
    ),
    (
        "icons/clock.svg",
        include_bytes!("../../assets/icons/clock.svg"),
    ),
    (
        "icons/dashed.svg",
        include_bytes!("../../assets/icons/dashed.svg"),
    ),
    (
        "icons/half.svg",
        include_bytes!("../../assets/icons/half.svg"),
    ),
    (
        "icons/ring.svg",
        include_bytes!("../../assets/icons/ring.svg"),
    ),
    (
        "icons/target.svg",
        include_bytes!("../../assets/icons/target.svg"),
    ),
    (
        "icons/turn.svg",
        include_bytes!("../../assets/icons/turn.svg"),
    ),
];

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        if let Some(key) = path.strip_prefix("agent/") {
            return Ok(Agent::from_key(key).icon_svg().map(Cow::Borrowed));
        }
        Ok(ICONS
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }
    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| (*p).into())
            .collect())
    }
}

/// The asset path of a CLI's mark, when it has one.
pub fn agent_icon(agent: Agent) -> Option<gpui::SharedString> {
    agent
        .icon_svg()
        .map(|_| format!("agent/{}", agent.key()).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    #[test]
    fn every_mark_loads() {
        for (path, _) in ICONS {
            assert!(Assets.load(path).unwrap().is_some(), "{path}");
        }
        assert!(Assets.load("icons/missing.svg").unwrap().is_none());
        assert!(Assets.load("agent/no-such-cli").unwrap().is_none());
        for agent in Agent::all() {
            if let Some(path) = agent_icon(agent) {
                assert!(Assets.load(&path).unwrap().is_some(), "{path}");
            }
        }
    }
}
