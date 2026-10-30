//! Measures the plugin prototype against the gates in specs/plugins/plugin-system.md.
//! Usage: bench <plugin.wasm> [sessions]
use keepline_plugin_host::{Plugin, engine, precompile, start_ticker};
use std::time::Instant;

fn state(n: usize) -> Vec<u8> {
    let groups = ["waiting", "error", "running", "review", "paused", "stale"];
    let sessions: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let g = groups[i % groups.len()];
            serde_json::json!({
                "id": format!("s{i:04}"),
                "title": format!("这个诊断的 ui 感觉很丑啊 第 {i} 个会话，看看标题够长时会怎样"),
                "agent": if i % 2 == 0 { "claude" } else { "codex" },
                "group": g,
                "cwd": "/Users/lifcc/Desktop/code/AI/tools/keepline",
                "space": "工作空间",
                "updated": 1_793_336_000u64 - i as u64 * 97,
                "running_since": if g == "running" { Some(1_793_336_000u64) } else { None },
                "stopped": if g == "paused" || g == "error" { Some("managed_session 测试超时，等你决定要不要加长等待") } else { None },
                "done": if g == "review" { Some("诊断面板改成图表，已提交") } else { None },
                "needs_you": g == "waiting" || g == "error",
                "closed": i % 9 == 0,
                "selected": i == 3,
            })
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({ "sessions": sessions, "spaces": [], "tasks": [], "now": 1_793_336_640u64 })).unwrap()
}

fn main() -> wasmtime::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let wasm = std::fs::read(&args[1]).expect("read plugin");
    let n: usize = args.get(2).map_or(300, |s| s.parse().unwrap());

    let t = Instant::now();
    let engine = engine()?;
    start_ticker(&engine);
    let engine_ms = t.elapsed().as_secs_f64() * 1e3;

    let t = Instant::now();
    let compiled = precompile(&engine, &wasm)?;
    let compile_ms = t.elapsed().as_secs_f64() * 1e3;

    let t = Instant::now();
    // SAFETY: produced by `precompile` above with this engine.
    let mut plugin = unsafe { Plugin::from_precompiled(&engine, &compiled)? };
    let load_ms = t.elapsed().as_secs_f64() * 1e3;

    let input = state(n);
    let first = plugin.render(&input)?;
    let mut times: Vec<f64> = (0..500)
        .map(|_| {
            let t = Instant::now();
            plugin.render(&input).unwrap();
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    let pct = |p: f64| times[((times.len() as f64 - 1.) * p) as usize];

    println!("wasm plugin size        {:>8} bytes", wasm.len());
    println!("precompiled size        {:>8} bytes", compiled.len());
    println!("engine init             {engine_ms:>8.2} ms");
    println!("first compile (cold)    {compile_ms:>8.2} ms");
    println!("load precompiled+inst.  {load_ms:>8.2} ms   <- startup cost with a disk cache");
    println!(
        "state in / sidebar out  {:>8} / {} bytes ({n} sessions)",
        input.len(),
        first.len()
    );
    println!(
        "render p50 / p95 / max  {:>8.3} / {:.3} / {:.3} ms",
        pct(0.5),
        pct(0.95),
        pct(1.0)
    );

    // Failure isolation: a runaway export must be interrupted, and the plugin stays usable
    // or is replaced; the host must not crash.
    let t = Instant::now();
    let spun = plugin.call_raw("spin");
    println!(
        "runaway call            {} after {:.1} ms",
        match &spun {
            Ok(_) => "RETURNED (unexpected)".to_string(),
            Err(e) => format!("interrupted: {e}"),
        },
        t.elapsed().as_secs_f64() * 1e3
    );
    let bad = plugin.render(b"not json");
    println!(
        "garbage input           {}",
        if bad.is_ok() {
            "plugin replied with an error object (handled)"
        } else {
            "error returned (handled)"
        }
    );
    // SAFETY: as above.
    let mut fresh = unsafe { Plugin::from_precompiled(&engine, &compiled)? };
    fresh.render(&input)?;
    println!("after failures          host alive, fresh instance renders OK");
    Ok(())
}
