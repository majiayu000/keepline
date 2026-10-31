//! Plugin pane decorations (`keepline_core::decoration`): each command runs in the
//! background for the pane's directory and session, and the line it last printed is drawn
//! in the pane's title bar or its title's hover card. Drawing only reads what is cached;
//! a line due again is shown as it was until the new run lands.
use super::*;
use keepline_core::decoration::{self, Decoration, Pane as DecoratedPane, Place};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

struct Shown {
    ran: Instant,
    line: Option<String>,
    running: bool,
}
/// (decoration index, pane facts) -> its last run.
static SHOWN: LazyLock<Mutex<HashMap<(usize, DecoratedPane), Shown>>> =
    LazyLock::new(Default::default);
/// Runs not drawn for this long are forgotten once the cache grows.
const FORGET: Duration = Duration::from_secs(600);
const KEEP: usize = 256;

/// A pane's decoration lines, by where they go.
#[derive(Default)]
pub(super) struct Lines {
    pub(super) title: Vec<String>,
    pub(super) tip: Vec<String>,
}

impl Workbench {
    pub(super) fn pane_decorations(&self, terminal: &str, cx: &Context<Self>) -> Lines {
        let mut lines = Lines::default();
        if decoration::all().is_empty() {
            return lines;
        }
        let ctx = self.action_context(terminal, Some(cx));
        if ctx.cwd.is_empty() {
            return lines;
        }
        let pane = DecoratedPane {
            cwd: ctx.cwd,
            agent: ctx.agent,
            session: ctx.session_id,
        };
        for (i, d) in decoration::all().iter().enumerate() {
            if !d.shown(&pane) {
                continue;
            }
            if let Some(line) = self.decoration_line(i, d, &pane, cx) {
                match d.place {
                    Place::Title => lines.title.push(line),
                    Place::Tip => lines.tip.push(line),
                }
            }
        }
        lines
    }
    /// The cached line; a missing or due one is run in the background, which redraws when
    /// it lands and again when it is due, so the line stays current while it is shown.
    fn decoration_line(
        &self,
        i: usize,
        d: &'static Decoration,
        pane: &DecoratedPane,
        cx: &Context<Self>,
    ) -> Option<String> {
        let mut shown = SHOWN.lock().ok()?;
        let key = (i, pane.clone());
        let old = match shown.get(&key) {
            Some(s) if s.running || s.ran.elapsed() < d.refresh() => return s.line.clone(),
            Some(s) => s.line.clone(),
            None => None,
        };
        if shown.len() > KEEP {
            shown.retain(|_, s| s.running || s.ran.elapsed() < FORGET);
        }
        shown.insert(
            key.clone(),
            Shown {
                ran: Instant::now(),
                line: old.clone(),
                running: true,
            },
        );
        drop(shown);
        cx.spawn(async move |this, cx| {
            let pane = key.1.clone();
            let line = cx
                .background_spawn(async move {
                    d.run(&pane).unwrap_or_else(|e| {
                        eprintln!("窗格装饰 {}：{e}", d.id);
                        None
                    })
                })
                .await;
            let changed = SHOWN.lock().is_ok_and(|mut shown| {
                let before = shown.insert(
                    key,
                    Shown {
                        ran: Instant::now(),
                        line: line.clone(),
                        running: false,
                    },
                );
                before.and_then(|s| s.line) != line
            });
            if changed {
                let _ = this.update(cx, |_, cx| cx.notify());
            }
            // Due again: a redraw runs it once more if the pane is still on screen.
            cx.background_executor().timer(d.refresh()).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
        old
    }
}
