//! `hooks install|uninstall|status`: the Agent CLIs' own hook settings, written only by
//! `install` and `uninstall`. `status` reads them and says how they differ from what
//! `install` would write now.
use keepline_core::model::Agent;
use keeplined::hooks::{self, Status};
use std::io;

const USAGE: &str =
    "usage: hooks install|uninstall|status [--agent <cli>] [--json] [--outdated-only]";

/// Every CLI that reads hooks when none is named. Prints one line per CLI: what changed
/// and in which file, or for `status` its state and every difference below it.
pub fn run(mut args: impl Iterator<Item = String>) -> io::Result<()> {
    let action = args.next().unwrap_or_default();
    let (mut agents, mut json, mut outdated_only) = (vec![], false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--agent" => match args.next().as_deref().and_then(Agent::from_executable) {
                Some(agent) => agents.push(agent),
                None => return Err(io::Error::other(USAGE)),
            },
            "--json" if action == "status" => json = true,
            "--outdated-only" if action == "status" => outdated_only = true,
            _ => return Err(io::Error::other(USAGE)),
        }
    }
    let home = std::path::PathBuf::from(
        std::env::var_os("HOME").ok_or_else(|| io::Error::other("HOME unavailable"))?,
    );
    let named = !agents.is_empty();
    if !named {
        agents = Agent::all();
    }
    let mut rows = vec![];
    for agent in agents {
        let target = match hooks::target(agent, &home) {
            Ok(target) => target,
            // Unnamed, a CLI that needs no hooks is simply not listed.
            Err(_) if !named => continue,
            Err(e) => return Err(e),
        };
        let path = target.settings.display().to_string();
        let line = match action.as_str() {
            "install" if hooks::install(agent, &target)? => format!("已安装：{path}"),
            "install" => format!("已是最新：{path}"),
            "uninstall" if hooks::uninstall(&target)? => format!("已移除：{path}"),
            "uninstall" => format!("本来就没装：{path}"),
            // A settings file that cannot be read is reported, and the other CLIs still are.
            "status" => {
                rows.push((agent, path, hooks::status(agent, &target)));
                continue;
            }
            _ => return Err(io::Error::other(USAGE)),
        };
        println!("{} · {line}", agent.name());
    }
    if outdated_only {
        rows.retain(|(.., s)| !matches!(s, Ok(Status::Current)));
    }
    if json {
        let rows: Vec<_> = rows.iter().map(to_json).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).map_err(io::Error::other)?
        );
    } else {
        for (agent, path, status) in &rows {
            println!("{} · {}：{path}", agent.name(), state(status).1);
            for line in differences(status) {
                println!("  {line}");
            }
        }
    }
    Ok(())
}
/// The machine word and the line `status` prints for one CLI.
fn state(status: &io::Result<Status>) -> (&'static str, &'static str) {
    match status {
        Ok(Status::Current) => ("current", "已安装且是最新"),
        Ok(Status::Outdated(_)) => ("outdated", "已安装但过期"),
        Ok(Status::Missing) => ("missing", "未安装"),
        Err(_) => ("error", "无法读取"),
    }
}
fn differences(status: &io::Result<Status>) -> Vec<String> {
    match status {
        Ok(Status::Outdated(lines)) => lines.clone(),
        Err(e) => vec![e.to_string()],
        Ok(_) => vec![],
    }
}
fn to_json((agent, path, status): &(Agent, String, io::Result<Status>)) -> serde_json::Value {
    serde_json::json!({
        "agent": agent.executable(),
        "name": agent.name(),
        "settings": path,
        "state": state(status).0,
        "differences": differences(status),
    })
}
