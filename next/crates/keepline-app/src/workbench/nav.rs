//! Where the user is: the space, tab and pane in view, or the overview.

/// Where the user is. Every view reads it; the shell moves it.
pub(super) struct Nav {
    pub(super) space: String,
    pub(super) tab: Option<String>,
    pub(super) focused: Option<String>,
    pub(super) pending_focus: bool,
    pub(super) zoomed: Option<String>,
    pub(super) overview: bool,
    pub(super) task_detail: Option<String>,
}
/// Space, tab, overview, task detail: the page the user chose.
pub(super) type Page = (String, Option<String>, bool, Option<String>);
impl Nav {
    /// The page the user chose: a change means they went somewhere themselves.
    pub(super) fn page(&self) -> Page {
        (
            self.space.clone(),
            self.tab.clone(),
            self.overview,
            self.task_detail.clone(),
        )
    }
}
