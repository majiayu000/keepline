//! Plugin actions (`keepline_core::action`): offered in a pane's right-click menu and in
//! ⌘P, run for that pane in a new split, a new tab, or typed into it without Enter.
use super::*;
use keepline_core::action::{self, Action, Context as ActionContext, Run};

impl Workbench {
    /// What a pane offers an action's template: its directory, and its Agent session,
    /// transcript, task and Agent when it has them.
    /// `cx` reads the pane's live directory; without it the directory it started in.
    pub(super) fn action_context(&self, terminal: &str, cx: Option<&gpui::App>) -> ActionContext {
        let session = self.workspace.current_agent(terminal);
        let cwd = cx
            .and_then(|cx| {
                self.layout
                    .panes
                    .get(terminal)
                    .and_then(|p| p.read(cx).cwd().map(|c| c.display().to_string()))
            })
            .or_else(|| {
                self.workspace
                    .terminals
                    .iter()
                    .find(|t| t.id == terminal)
                    .map(|t| t.cwd.clone())
            })
            .unwrap_or_default();
        let task = session
            .and_then(|s| s.task.clone())
            .or_else(|| {
                self.workspace
                    .terminal_tab(terminal)
                    .and_then(|t| t.default_task.clone())
            })
            .and_then(|id| self.workspace.tasks.iter().find(|t| t.id == id))
            .map(|t| t.title.clone());
        ActionContext {
            cwd,
            session_id: session.map(|s| s.cli_session_id.clone()),
            transcript: session
                .map(|s| s.transcript.clone())
                .filter(|t| !t.is_empty()),
            task,
            agent: session.map(|s| s.runtime.name().to_owned()),
        }
    }
    /// The loaded actions a pane offers, in plugin order. Offering depends on its session
    /// and task only, never on where its shell has wandered.
    pub(super) fn actions_for(&self, terminal: &str) -> Vec<&'static Action> {
        let ctx = self.action_context(terminal, None);
        action::all().iter().filter(|a| a.offered(&ctx)).collect()
    }
    /// Runs one action for a pane; a value the pane lacks is reported, never run blank.
    pub(super) fn run_action(&mut self, terminal: &str, action: &Action, cx: &mut Context<Self>) {
        self.layout.menu = None;
        let ctx = self.action_context(terminal, Some(cx));
        let command = match action.command(&ctx) {
            Ok(c) => c,
            Err(why) => {
                self.error = Some(format!("{}：{why}", action.title));
                cx.notify();
                return;
            }
        };
        match action.run {
            Run::Paste => {
                if let Some(pane) = self.layout.panes.get(terminal) {
                    pane.update(cx, |p, cx| p.place_text(command, cx));
                }
            }
            Run::Split | Run::Tab => {
                let split = action.run == Run::Split;
                self.layout.pending_command = Some((command, ctx.cwd));
                if split {
                    self.nav.zoomed = None;
                    self.layout.split_target = Some((terminal.to_owned(), Axis::Right));
                }
                self.launch(split, None, cx);
            }
        }
        cx.notify();
    }
}
