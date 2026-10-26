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
