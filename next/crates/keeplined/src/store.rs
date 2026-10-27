use keepline_core::model::*;
use rusqlite::{params, Connection};
use std::{io, path::Path};

pub struct Store(Connection);
fn db_error(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}
fn enum_name<T: serde::Serialize>(value: &T) -> io::Result<String> {
    match serde_json::to_value(value).map_err(db_error)? {
        serde_json::Value::String(name) => Ok(name),
        _ => Err(io::Error::other("expected scalar enum")),
    }
}
fn enum_value<T: serde::de::DeserializeOwned>(name: String) -> io::Result<T> {
    serde_json::from_value(serde_json::Value::String(name)).map_err(db_error)
}

impl Store {
    pub fn open(root: &Path) -> io::Result<(Self, Workspace)> {
        Self::open_keeping(root, &Default::default())
    }
    /// Like `open`, except that terminals in `kept` were handed over alive by the previous
    /// host: they stay running and keep their current Agent.
    pub fn open_keeping(
        root: &Path,
        kept: &std::collections::HashSet<String>,
    ) -> io::Result<(Self, Workspace)> {
        let db = Connection::open(root.join("workbench.sqlite")).map_err(db_error)?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        let old: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='workspace')",
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let old_grid: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('terminals') WHERE name='position')",
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if old_grid {
            return Err(io::Error::other(
                "旧固定网格库不可用于分割树构建；请指定新的隔离 runtime，不自动迁移或删除",
            ));
        }
        if old {
            return Err(io::Error::other(
                "旧实验库仍使用 workspace JSON；请指定新的隔离目录，不自动迁移或删除",
            ));
        }
        db.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS meta (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL);
            INSERT OR IGNORE INTO meta VALUES(1,0);
            CREATE TABLE IF NOT EXISTS app_settings (id INTEGER PRIMARY KEY CHECK(id=1), notifications INTEGER NOT NULL, stale_days INTEGER NOT NULL, away_minutes INTEGER NOT NULL, default_agent TEXT NOT NULL);
            INSERT OR IGNORE INTO app_settings VALUES(1,0,3,30,'Codex');
            CREATE TABLE IF NOT EXISTS notification_receipts (session_id TEXT PRIMARY KEY, fact_key TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS client_state (id INTEGER PRIMARY KEY CHECK(id=1), last_left INTEGER NOT NULL);
            INSERT OR IGNORE INTO client_state VALUES(1,0);
            CREATE TABLE IF NOT EXISTS spaces (id TEXT PRIMARY KEY, name TEXT NOT NULL, cwd TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES spaces(id), title TEXT NOT NULL, description TEXT NOT NULL, runtime TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS outcome_history (sequence INTEGER PRIMARY KEY, session_id TEXT NOT NULL, before_json TEXT NOT NULL, after_json TEXT NOT NULL, at INTEGER NOT NULL, reason TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS task_outcomes (task_id TEXT PRIMARY KEY REFERENCES tasks(id), outcome TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS pending_tasks (terminal_id TEXT PRIMARY KEY REFERENCES terminals(id), task_id TEXT REFERENCES tasks(id));
            CREATE TABLE IF NOT EXISTS task_criteria (task_id TEXT NOT NULL REFERENCES tasks(id), id TEXT NOT NULL, position INTEGER NOT NULL,
                description TEXT NOT NULL, evidence TEXT, PRIMARY KEY(task_id,id), UNIQUE(task_id,position));
            CREATE TABLE IF NOT EXISTS tabs (id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES spaces(id), default_task_id TEXT REFERENCES tasks(id), title TEXT NOT NULL, closed INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS terminals (id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES spaces(id), argv TEXT NOT NULL, cwd TEXT NOT NULL, pid INTEGER, process_state TEXT NOT NULL, exit_code INTEGER, error TEXT);
            CREATE TABLE IF NOT EXISTS tab_layout_nodes (tab_id TEXT NOT NULL REFERENCES tabs(id), path TEXT NOT NULL, terminal_id TEXT UNIQUE REFERENCES terminals(id), axis TEXT, ratio INTEGER, PRIMARY KEY(tab_id,path), CHECK ((terminal_id IS NOT NULL AND axis IS NULL AND ratio IS NULL) OR (terminal_id IS NULL AND axis IN ('Right','Down') AND ratio BETWEEN 1 AND 9999)));
            CREATE TABLE IF NOT EXISTS agent_sessions (id TEXT PRIMARY KEY, runtime TEXT NOT NULL, cli_session_id TEXT NOT NULL,
                terminal_id TEXT REFERENCES terminals(id), is_current INTEGER NOT NULL, task_id TEXT REFERENCES tasks(id),
                outcome TEXT NOT NULL, deferred_reason TEXT, deferred_until INTEGER, updated INTEGER NOT NULL, cwd TEXT NOT NULL, transcript TEXT NOT NULL,
                UNIQUE(runtime,cli_session_id));
            CREATE UNIQUE INDEX IF NOT EXISTS one_current_agent_per_terminal ON agent_sessions(terminal_id) WHERE is_current=1;")
            .map_err(db_error)?;
        let mut store = Self(db);
        let mut workspace = store.load()?;
        let before = workspace.clone();
        for terminal in &mut workspace.terminals {
            if !kept.contains(&terminal.id)
                && matches!(
                    terminal.state,
                    ProcessState::Starting | ProcessState::Running | ProcessState::Draining
                )
            {
                terminal.state = ProcessState::Interrupted;
                terminal.error = Some("后台已重启，原 PTY 不可重新连接；可用原 CLI 恢复".into());
            }
        }
        for session in &mut workspace.agent_sessions {
            if !session.terminal.as_ref().is_some_and(|t| kept.contains(t)) {
                session.current = false;
            }
        }
        workspace.revision += 1;
        store.save(&before, &workspace)?;
        Ok((store, workspace))
    }
    pub fn claim_notification(&self, session: &str, key: &str) -> io::Result<bool> {
        self.0.execute("INSERT INTO notification_receipts VALUES(?1,?2) ON CONFLICT(session_id) DO UPDATE SET fact_key=excluded.fact_key WHERE fact_key<>excluded.fact_key",params![session,key]).map(|n|n>0).map_err(db_error)
    }
    pub fn transcripts(&self) -> io::Result<Vec<crate::recognition::Transcript>> {
        crate::log_index::cached(&self.0)
    }
    pub fn subscription_error(&self) -> io::Result<Option<String>> {
        use rusqlite::OptionalExtension;
        Ok(self
            .0
            .query_row("SELECT error FROM log_index WHERE id=1", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .optional()
            .map_err(db_error)?
            .flatten())
    }
    fn load(&self) -> io::Result<Workspace> {
        let mut w = Workspace {
            revision: u64::try_from(
                self.0
                    .query_row("SELECT revision FROM meta WHERE id=1", [], |r| {
                        r.get::<_, i64>(0)
                    })
                    .map_err(db_error)?,
            )
            .map_err(db_error)?,
            ..Workspace::default()
        };
        w.settings=self.0.query_row("SELECT notifications,stale_days,away_minutes,default_agent FROM app_settings WHERE id=1",[],|r| Ok((r.get::<_,bool>(0)?,r.get::<_,u32>(1)?,r.get::<_,u32>(2)?,r.get::<_,String>(3)?))).map_err(db_error).and_then(|(notifications,stale_days,away_minutes,agent)|Ok(Settings {notifications,stale_days,away_minutes,default_agent:enum_value(agent)?}))?;
        w.last_left = u64::try_from(
            self.0
                .query_row("SELECT last_left FROM client_state WHERE id=1", [], |r| {
                    r.get::<_, i64>(0)
                })
                .map_err(db_error)?,
        )
        .map_err(db_error)?;
        for row in self.0.prepare("SELECT session_id,before_json,after_json,at,reason FROM outcome_history ORDER BY sequence").map_err(db_error)?.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,String>(4)?))).map_err(db_error)? {
            let (session,before,after,at,reason)=row.map_err(db_error)?;
            w.outcome_history.push(OutcomeChange {session,before:serde_json::from_str(&before).map_err(db_error)?,after:serde_json::from_str(&after).map_err(db_error)?,at:u64::try_from(at).map_err(db_error)?,reason});
        }
        for row in self
            .0
            .prepare("SELECT task_id,outcome FROM task_outcomes")
            .map_err(db_error)?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_error)?
        {
            let (id, value) = row.map_err(db_error)?;
            w.task_outcomes
                .insert(id, serde_json::from_str(&value).map_err(db_error)?);
        }
        for row in self
            .0
            .prepare("SELECT terminal_id,task_id FROM pending_tasks")
            .map_err(db_error)?
            .query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })
            .map_err(db_error)?
        {
            let (id, task) = row.map_err(db_error)?;
            w.pending_tasks.insert(id, task);
        }
        let mut statement = self
            .0
            .prepare("SELECT id,name,cwd FROM spaces ORDER BY rowid")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |r| {
                Ok(Space {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    cwd: r.get(2)?,
                })
            })
            .map_err(db_error)?;
        for row in rows {
            w.spaces.push(row.map_err(db_error)?);
        }
        let mut statement = self
            .0
            .prepare("SELECT id,space_id,title,description,runtime FROM tasks ORDER BY rowid")
            .map_err(db_error)?;
        let mut rows = statement.query([]).map_err(db_error)?;
        while let Some(r) = rows.next().map_err(db_error)? {
            w.tasks.push(Task {
                id: r.get(0).map_err(db_error)?,
                space: r.get(1).map_err(db_error)?,
                title: r.get(2).map_err(db_error)?,
                description: r.get(3).map_err(db_error)?,
                agent: enum_value(r.get(4).map_err(db_error)?)?,
                acceptance: vec![],
            });
        }
        let mut statement=self.0.prepare("SELECT task_id,id,description,evidence FROM task_criteria ORDER BY task_id,position").map_err(db_error)?;
        let mut rows = statement.query([]).map_err(db_error)?;
        while let Some(r) = rows.next().map_err(db_error)? {
            let task: String = r.get(0).map_err(db_error)?;
            w.tasks
                .iter_mut()
                .find(|t| t.id == task)
                .ok_or_else(|| io::Error::other("criterion has no task"))?
                .acceptance
                .push(AcceptanceCriterion {
                    id: r.get(1).map_err(db_error)?,
                    description: r.get(2).map_err(db_error)?,
                    evidence: r.get(3).map_err(db_error)?,
                });
        }
        // Tabs are deleted when closed. Rows an older build only marked closed, and tabs
        // left without panes, are dropped here; their terminals died with that host.
        self.0
            .execute_batch(
                "DELETE FROM tab_layout_nodes WHERE tab_id IN (SELECT id FROM tabs WHERE closed<>0);
                 DELETE FROM tabs WHERE closed<>0 OR id NOT IN (SELECT tab_id FROM tab_layout_nodes);",
            )
            .map_err(db_error)?;
        let mut statement = self
            .0
            .prepare("SELECT id,space_id,default_task_id,title FROM tabs ORDER BY rowid")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |r| {
                Ok(Tab {
                    id: r.get(0)?,
                    space: r.get(1)?,
                    default_task: r.get(2)?,
                    title: r.get(3)?,
                    layout: None,
                })
            })
            .map_err(db_error)?;
        for row in rows {
            w.tabs.push(row.map_err(db_error)?);
        }
        let mut statement=self.0.prepare("SELECT id,space_id,argv,cwd,pid,process_state,exit_code,error FROM terminals ORDER BY rowid").map_err(db_error)?;
        let mut rows = statement.query([]).map_err(db_error)?;
        while let Some(r) = rows.next().map_err(db_error)? {
            let argv: String = r.get(2).map_err(db_error)?;
            w.terminals.push(Terminal {
                id: r.get(0).map_err(db_error)?,
                space: r.get(1).map_err(db_error)?,
                argv: serde_json::from_str(&argv).map_err(db_error)?,
                cwd: r.get(3).map_err(db_error)?,
                pid: r.get(4).map_err(db_error)?,
                state: enum_value(r.get(5).map_err(db_error)?)?,
                exit_code: r.get(6).map_err(db_error)?,
                error: r.get(7).map_err(db_error)?,
            });
        }
        for tab in &mut w.tabs {
            tab.layout = load_layout(&self.0, &tab.id, "")?;
        }
        let mut statement=self.0.prepare("SELECT id,runtime,cli_session_id,terminal_id,is_current,task_id,outcome,deferred_reason,deferred_until,updated,cwd,transcript FROM agent_sessions ORDER BY rowid").map_err(db_error)?;
        let mut rows = statement.query([]).map_err(db_error)?;
        while let Some(r) = rows.next().map_err(db_error)? {
            let kind: String = r.get(6).map_err(db_error)?;
            let outcome = match kind.as_str() {
                "Open" => Outcome::Open,
                "Completed" => Outcome::Completed,
                "Abandoned" => Outcome::Abandoned,
                "Deferred" => Outcome::Deferred {
                    reason: r.get(7).map_err(db_error)?,
                    until: u64::try_from(r.get::<_, i64>(8).map_err(db_error)?)
                        .map_err(db_error)?,
                },
                _ => return Err(io::Error::other("unknown stored outcome")),
            };
            w.agent_sessions.push(AgentSession {
                id: r.get(0).map_err(db_error)?,
                runtime: enum_value(r.get(1).map_err(db_error)?)?,
                cli_session_id: r.get(2).map_err(db_error)?,
                terminal: r.get(3).map_err(db_error)?,
                current: r.get(4).map_err(db_error)?,
                task: r.get(5).map_err(db_error)?,
                outcome,
                updated: u64::try_from(r.get::<_, i64>(9).map_err(db_error)?).map_err(db_error)?,
                cwd: r.get(10).map_err(db_error)?,
                transcript: r.get(11).map_err(db_error)?,
            });
        }
        Ok(w)
    }
    pub fn save(&mut self, before: &Workspace, w: &Workspace) -> io::Result<()> {
        let tx = self.0.transaction().map_err(db_error)?;
        if before.settings != w.settings {
            tx.execute("UPDATE app_settings SET notifications=?1,stale_days=?2,away_minutes=?3,default_agent=?4 WHERE id=1",params![w.settings.notifications,w.settings.stale_days,w.settings.away_minutes,enum_name(&w.settings.default_agent)?]).map_err(db_error)?;
        }
        if before.last_left != w.last_left {
            tx.execute(
                "UPDATE client_state SET last_left=?1 WHERE id=1",
                [i64::try_from(w.last_left).map_err(db_error)?],
            )
            .map_err(db_error)?;
        }

        for s in changed(&before.spaces, &w.spaces, |s| &s.id) {
            tx.execute("INSERT INTO spaces VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,cwd=excluded.cwd",params![s.id,s.name,s.cwd]).map_err(db_error)?;
        }
        for t in changed(&before.tasks, &w.tasks, |t| &t.id) {
            tx.execute("INSERT INTO tasks VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET space_id=excluded.space_id,title=excluded.title,description=excluded.description,runtime=excluded.runtime",params![t.id,t.space,t.title,t.description,enum_name(&t.agent)?]).map_err(db_error)?;
            tx.execute("DELETE FROM task_criteria WHERE task_id=?1", [&t.id])
                .map_err(db_error)?;
            for (position, c) in t.acceptance.iter().enumerate() {
                tx.execute(
                    "INSERT INTO task_criteria VALUES(?1,?2,?3,?4,?5)",
                    params![
                        t.id,
                        c.id,
                        i64::try_from(position).map_err(db_error)?,
                        c.description,
                        c.evidence
                    ],
                )
                .map_err(db_error)?;
            }
        }
        for t in removed(&before.tabs, &w.tabs, |t| &t.id) {
            tx.execute("DELETE FROM tab_layout_nodes WHERE tab_id=?1", [&t.id])
                .map_err(db_error)?;
            tx.execute("DELETE FROM tabs WHERE id=?1", [&t.id])
                .map_err(db_error)?;
        }
        for t in changed(&before.tabs, &w.tabs, |t| &t.id) {
            tx.execute("INSERT INTO tabs VALUES(?1,?2,?3,?4,0) ON CONFLICT(id) DO UPDATE SET space_id=excluded.space_id,default_task_id=excluded.default_task_id,title=excluded.title",params![t.id,t.space,t.default_task,t.title]).map_err(db_error)?;
        }
        for t in changed(&before.terminals, &w.terminals, |t| &t.id) {
            tx.execute("INSERT INTO terminals VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET space_id=excluded.space_id,argv=excluded.argv,cwd=excluded.cwd,pid=excluded.pid,process_state=excluded.process_state,exit_code=excluded.exit_code,error=excluded.error",params![t.id,t.space,serde_json::to_string(&t.argv).map_err(db_error)?,t.cwd,t.pid,enum_name(&t.state)?,t.exit_code,t.error]).map_err(db_error)?;
        }
        let tabs = changed(&before.tabs, &w.tabs, |t| &t.id);
        for tab in &tabs {
            tx.execute("DELETE FROM tab_layout_nodes WHERE tab_id=?1", [&tab.id])
                .map_err(db_error)?;
        }
        for tab in tabs {
            if let Some(tree) = &tab.layout {
                save_layout(&tx, &tab.id, "", tree)?;
            }
        }
        let sessions = changed(&before.agent_sessions, &w.agent_sessions, |s| &s.id);
        for s in &sessions {
            tx.execute(
                "UPDATE agent_sessions SET is_current=0 WHERE id=?1",
                [&s.id],
            )
            .map_err(db_error)?;
        }
        for s in sessions {
            let (kind, reason, until) = match &s.outcome {
                Outcome::Open => ("Open", None, None),
                Outcome::Completed => ("Completed", None, None),
                Outcome::Abandoned => ("Abandoned", None, None),
                Outcome::Deferred { reason, until } => (
                    "Deferred",
                    Some(reason.as_str()),
                    Some(i64::try_from(*until).map_err(db_error)?),
                ),
            };
            tx.execute("INSERT INTO agent_sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12) ON CONFLICT(id) DO UPDATE SET runtime=excluded.runtime,cli_session_id=excluded.cli_session_id,terminal_id=excluded.terminal_id,is_current=excluded.is_current,task_id=excluded.task_id,outcome=excluded.outcome,deferred_reason=excluded.deferred_reason,deferred_until=excluded.deferred_until,updated=excluded.updated,cwd=excluded.cwd,transcript=excluded.transcript",params![s.id,enum_name(&s.runtime)?,s.cli_session_id,s.terminal,s.current,s.task,kind,reason,until,i64::try_from(s.updated).map_err(db_error)?,s.cwd,s.transcript]).map_err(db_error)?;
        }
        for h in w.outcome_history.iter().skip(before.outcome_history.len()) {
            tx.execute("INSERT INTO outcome_history(session_id,before_json,after_json,at,reason) VALUES(?1,?2,?3,?4,?5)",params![h.session,serde_json::to_string(&h.before).map_err(db_error)?,serde_json::to_string(&h.after).map_err(db_error)?,i64::try_from(h.at).map_err(db_error)?,h.reason]).map_err(db_error)?;
        }
        if before.task_outcomes != w.task_outcomes {
            for (id, outcome) in &w.task_outcomes {
                if before.task_outcomes.get(id) != Some(outcome) {
                    tx.execute("INSERT INTO task_outcomes VALUES(?1,?2) ON CONFLICT(task_id) DO UPDATE SET outcome=excluded.outcome", params![id,serde_json::to_string(outcome).map_err(db_error)?]).map_err(db_error)?;
                }
            }
        }
        if before.pending_tasks != w.pending_tasks {
            for id in before
                .pending_tasks
                .keys()
                .filter(|id| !w.pending_tasks.contains_key(*id))
            {
                tx.execute("DELETE FROM pending_tasks WHERE terminal_id=?1", [id])
                    .map_err(db_error)?;
            }
            for (id, task) in &w.pending_tasks {
                if before.pending_tasks.get(id) != Some(task) {
                    tx.execute("INSERT INTO pending_tasks VALUES(?1,?2) ON CONFLICT(terminal_id) DO UPDATE SET task_id=excluded.task_id", params![id,task]).map_err(db_error)?;
                }
            }
        }
        tx.execute(
            "UPDATE meta SET revision=?1 WHERE id=1",
            [i64::try_from(w.revision).map_err(db_error)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)
    }
}

fn changed<'a, T: PartialEq>(before: &[T], after: &'a [T], id: impl Fn(&T) -> &str) -> Vec<&'a T> {
    let old: std::collections::HashMap<_, _> = before.iter().map(|item| (id(item), item)).collect();
    after
        .iter()
        .filter(|item| old.get(id(item)).is_none_or(|previous| *previous != *item))
        .collect()
}

fn removed<'a, T>(before: &'a [T], after: &[T], id: impl Fn(&T) -> &str) -> Vec<&'a T> {
    let kept: std::collections::HashSet<_> = after.iter().map(&id).collect();
    before
        .iter()
        .filter(|item| !kept.contains(id(item)))
        .collect()
}

fn load_layout(db: &Connection, tab: &str, path: &str) -> io::Result<Option<Layout>> {
    use rusqlite::OptionalExtension;
    let row: Option<(Option<String>, Option<String>, Option<u16>)> = db
        .query_row(
            "SELECT terminal_id,axis,ratio FROM tab_layout_nodes WHERE tab_id=?1 AND path=?2",
            params![tab, path],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    match row {
        None => Ok(None),
        Some((Some(id), None, None)) => Ok(Some(Layout::Leaf(id))),
        Some((None, Some(axis), Some(ratio))) => Ok(Some(Layout::Split {
            axis: enum_value(axis)?,
            ratio,
            first: Box::new(
                load_layout(db, tab, &format!("{path}0"))?
                    .ok_or_else(|| io::Error::other("missing first split child"))?,
            ),
            second: Box::new(
                load_layout(db, tab, &format!("{path}1"))?
                    .ok_or_else(|| io::Error::other("missing second split child"))?,
            ),
        })),
        _ => Err(io::Error::other("invalid stored layout node")),
    }
}
fn save_layout(db: &Connection, tab: &str, path: &str, tree: &Layout) -> io::Result<()> {
    match tree {
        Layout::Leaf(id) => {
            db.execute(
                "INSERT INTO tab_layout_nodes VALUES(?1,?2,?3,NULL,NULL)",
                params![tab, path, id],
            )
            .map_err(db_error)?;
        }
        Layout::Split {
            axis,
            ratio,
            first,
            second,
        } => {
            db.execute(
                "INSERT INTO tab_layout_nodes VALUES(?1,?2,NULL,?3,?4)",
                params![tab, path, enum_name(axis)?, ratio],
            )
            .map_err(db_error)?;
            save_layout(db, tab, &format!("{path}0"), first)?;
            save_layout(db, tab, &format!("{path}1"), second)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod milestone_tests {
    use super::*;
    #[test]
    fn settings_and_notification_receipts_survive_restart() {
        let dir = std::env::temp_dir().join(format!("kl-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut db, before) = Store::open(&dir).unwrap();
        assert!(!before.settings.notifications);
        let mut after = before.clone();
        after.settings = Settings {
            notifications: true,
            stale_days: 7,
            away_minutes: 45,
            default_agent: Agent::Claude,
        };
        db.save(&before, &after).unwrap();
        assert!(db.claim_notification("session", "turn1:waiting").unwrap());
        assert!(!db.claim_notification("session", "turn1:waiting").unwrap());
        drop(db);
        let (db, loaded) = Store::open(&dir).unwrap();
        assert_eq!(loaded.settings, after.settings);
        assert!(!db.claim_notification("session", "turn1:waiting").unwrap());
        assert!(db.claim_notification("session", "turn2:waiting").unwrap());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reopen_drops_tabs_an_older_build_only_hid_and_tabs_without_panes() {
        let dir = std::env::temp_dir().join(format!("kl-tabs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut db, before) = Store::open(&dir).unwrap();
        let mut after = before.clone();
        after.spaces.push(Space {
            id: "s".into(),
            name: "s".into(),
            cwd: "/tmp".into(),
        });
        for id in ["kept", "hidden"] {
            after.terminals.push(Terminal {
                id: id.into(),
                space: "s".into(),
                argv: vec!["/bin/sh".into()],
                cwd: "/tmp".into(),
                pid: None,
                state: ProcessState::Exited,
                exit_code: Some(0),
                error: None,
            });
            after.tabs.push(Tab {
                id: id.into(),
                space: "s".into(),
                default_task: None,
                title: String::new(),
                layout: Some(Layout::Leaf(id.into())),
            });
        }
        db.save(&before, &after).unwrap();
        db.0.execute_batch(
            "UPDATE tabs SET closed=1 WHERE id='hidden';
             INSERT INTO tabs VALUES('empty','s',NULL,'',0);",
        )
        .unwrap();
        drop(db);
        let (db, loaded) = Store::open(&dir).unwrap();
        assert_eq!(
            loaded
                .tabs
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["kept"]
        );
        let rows: i64 =
            db.0.query_row("SELECT count(*) FROM tab_layout_nodes", [], |r| r.get(0))
                .unwrap();
        assert_eq!(rows, 1);
        let mut closed = loaded.clone();
        closed.edit(Edit::CloseTab { id: "kept".into() }).unwrap();
        let mut db = db;
        db.save(&loaded, &closed).unwrap();
        drop(db);
        let (_, loaded) = Store::open(&dir).unwrap();
        assert!(loaded.tabs.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn client_departure_survives_reopen_without_changing_sessions() {
        let dir = std::env::temp_dir().join(format!("kl-visit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut db, before) = Store::open(&dir).unwrap();
        let mut after = before.clone();
        after.last_left = 12345;
        db.save(&before, &after).unwrap();
        drop(db);
        let (_, loaded) = Store::open(&dir).unwrap();
        assert_eq!(loaded.last_left, 12345);
        assert!(loaded.agent_sessions.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn task_outcome_and_pending_override_survive_reopen() {
        let root = std::env::temp_dir().join(format!("kl-task-store-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (mut store, mut w) = Store::open(&root).unwrap();
        let before = w.clone();
        w.spaces.push(Space {
            id: "s".into(),
            name: "s".into(),
            cwd: "/tmp".into(),
        });
        w.tasks.push(Task {
            id: "t".into(),
            space: "s".into(),
            title: "task".into(),
            description: "original".into(),
            agent: Agent::Codex,
            acceptance: vec![AcceptanceCriterion {
                id: "c".into(),
                description: "check".into(),
                evidence: Some("source:42".into()),
            }],
        });
        w.terminals.push(Terminal {
            id: "p".into(),
            space: "s".into(),
            argv: vec!["/bin/sh".into()],
            cwd: "/tmp".into(),
            pid: None,
            state: ProcessState::Exited,
            exit_code: Some(0),
            error: None,
        });
        w.edit(Edit::SetPendingTask {
            terminal: "p".into(),
            task: None,
        })
        .unwrap();
        w.edit(Edit::SetTaskOutcome {
            id: "t".into(),
            outcome: Outcome::Deferred {
                reason: "await review".into(),
                until: 1000,
            },
        })
        .unwrap();
        w.agent_sessions.push(AgentSession {
            id: "s1".into(),
            runtime: Agent::Codex,
            cli_session_id: "s1".into(),
            terminal: None,
            current: false,
            task: None,
            outcome: Outcome::Open,
            updated: 1,
            cwd: "/tmp".into(),
            transcript: "/fixture".into(),
        });
        w.change_outcome("s1", Outcome::Completed, 2, "用户操作")
            .unwrap();
        store.save(&before, &w).unwrap();
        drop(store);
        let (store, read) = Store::open(&root).unwrap();
        assert_eq!(read.tasks, w.tasks);
        assert_eq!(read.task_outcomes, w.task_outcomes);
        assert_eq!(read.outcome_history, w.outcome_history);
        assert_eq!(read.pending_tasks, w.pending_tasks);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
