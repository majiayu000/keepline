//! Durable content: session cards, subagent links and saved parser state in SQLite.
use super::*;

pub(super) fn db_err(e: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(e)
}
/// What a parent's subagents are doing, from `subagent_links`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Children {
    pub running: u32,
    pub waiting: Option<String>,
}
/// A subagent whose log has not moved for this long no longer counts as running: its
/// process may have died mid-turn.
pub(super) const CHILD_FRESH: u64 = 900;
pub fn link_child(db: &Connection, path: &str, card: &Content) -> io::Result<()> {
    db.prepare_cached("INSERT INTO subagent_links VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET parent=excluded.parent,name=excluded.name,turn=excluded.turn,waiting=excluded.waiting,updated=excluded.updated")
        .map_err(db_err)?
        .execute(params![
            path,
            card.parent,
            card.agent_name,
            format!("{:?}", card.turn),
            card.waiting,
            i64::try_from(card.updated).map_err(db_err)?
        ])
        .map_err(db_err)?;
    Ok(())
}
pub fn children(db: &Connection, parent: &str, now: u64) -> io::Result<Children> {
    let mut q = db
        .prepare_cached("SELECT name,turn,waiting FROM subagent_links WHERE parent=?1 AND updated>=?2 ORDER BY updated DESC")
        .map_err(db_err)?;
    let since = i64::try_from(now.saturating_sub(CHILD_FRESH)).map_err(db_err)?;
    let mut rows = q.query(params![parent, since]).map_err(db_err)?;
    let mut kids = Children::default();
    while let Some(r) = rows.next().map_err(db_err)? {
        let name: String = r.get(0).map_err(db_err)?;
        let turn: String = r.get(1).map_err(db_err)?;
        match turn.as_str() {
            "Running" => kids.running += 1,
            "Waiting" if kids.waiting.is_none() => {
                let waiting: String = r.get(2).map_err(db_err)?;
                let name = if name.is_empty() {
                    "子 Agent".into()
                } else {
                    format!("子 Agent {name}")
                };
                kids.waiting = Some(format!("{name}：{}", short(&waiting, 500)));
                kids.running += 1;
            }
            _ => (),
        }
    }
    Ok(kids)
}
pub fn schema(db: &Connection) -> io::Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS session_content(path TEXT PRIMARY KEY,title TEXT NOT NULL,turn TEXT NOT NULL,updated INTEGER NOT NULL,projection TEXT NOT NULL,parser TEXT NOT NULL); CREATE TABLE IF NOT EXISTS subagent_links(path TEXT PRIMARY KEY,parent TEXT NOT NULL,name TEXT NOT NULL,turn TEXT NOT NULL,waiting TEXT NOT NULL,updated INTEGER NOT NULL); CREATE INDEX IF NOT EXISTS subagent_parent ON subagent_links(parent); CREATE TABLE IF NOT EXISTS session_records(path TEXT NOT NULL,inode INTEGER NOT NULL,offset INTEGER NOT NULL,ordinal INTEGER NOT NULL,at INTEGER NOT NULL,kind TEXT NOT NULL,text TEXT NOT NULL,call_id TEXT NOT NULL,exit_code INTEGER,PRIMARY KEY(path,inode,offset,ordinal));").map_err(db_err)
}
pub fn load(db: &Connection, path: &str) -> io::Result<Parser> {
    let value: Option<String> = db
        .prepare_cached("SELECT parser FROM session_content WHERE path=?1")
        .map_err(db_err)?
        .query_row([path], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    value
        .map(|s| serde_json::from_str(&s).map_err(db_err))
        .unwrap_or_else(|| Ok(Parser::default()))
}
pub fn card(db: &Connection, path: &str) -> io::Result<Option<Content>> {
    let value: Option<String> = db
        .prepare_cached("SELECT projection FROM session_content WHERE path=?1")
        .map_err(db_err)?
        .query_row([path], |r| r.get(0))
        .optional()
        .map_err(db_err)?;
    value
        .map(|s| serde_json::from_str(&s).map_err(db_err))
        .transpose()
}
/// With `KEEPLINE_STATUS_TRACE=<file>`, appends one line per turn change for regression
/// checks against a transcript. Paths and states only; no prompt or reply text.
pub(super) fn trace(path: &str, parser: &Parser) {
    let Some(file) = std::env::var_os("KEEPLINE_STATUS_TRACE") else {
        return;
    };
    if parser.transitions.is_empty() {
        return;
    }
    let lines: String = parser
        .transitions
        .iter()
        .map(|t| format!("{} {path} {t}\n", keepline_core::content::now()))
        .collect();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .and_then(|mut f| std::io::Write::write_all(&mut f, lines.as_bytes()));
}
pub fn save(db: &Connection, path: &str, parser: &Parser) -> io::Result<()> {
    trace(path, parser);
    let mut encoded = serde_json::to_value(parser).map_err(db_err)?;
    encoded["_rules"] = json!(12);
    db.prepare_cached("INSERT INTO session_content VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(path) DO UPDATE SET title=excluded.title,turn=excluded.turn,updated=excluded.updated,projection=excluded.projection,parser=excluded.parser").map_err(db_err)?.execute(params![path,parser.card.title,format!("{:?}",parser.card.turn),i64::try_from(parser.card.updated).map_err(db_err)?,serde_json::to_string(&parser.card).map_err(db_err)?,serde_json::to_string(&encoded).map_err(db_err)?]).map_err(db_err)?;
    let mut ordinals = BTreeMap::new();
    for r in &parser.records {
        let i = ordinals
            .entry((r.source.inode, r.source.offset))
            .or_insert(0u64);
        let ordinal = *i;
        *i += 1;
        db.prepare_cached(
            "INSERT OR IGNORE INTO session_records VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )
        .map_err(db_err)?
        .execute(params![
            path,
            i64::try_from(r.source.inode).map_err(db_err)?,
            i64::try_from(r.source.offset).map_err(db_err)?,
            ordinal as i64,
            i64::try_from(r.at).map_err(db_err)?,
            r.kind,
            r.text,
            r.call_id,
            r.code
        ])
        .map_err(db_err)?;
    }
    Ok(())
}
