//! A short, read-only host measurement. These observations are not workspace facts.
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Write};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Diagnostics {
    pub started_at: u64,
    pub elapsed_secs: f64,
    pub complete: bool,
    pub process_count: usize,
    /// Logical cores, so the view can say how many of them the ranking explains.
    #[serde(default)]
    pub cpu_cores: Option<usize>,
    /// All cores together: 100% means every core busy.
    pub cpu_percent: Option<f64>,
    /// Latest device reading, not a sum of the per-app shares below.
    pub gpu_percent: Option<f64>,
    pub apps: Vec<AppUsage>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppUsage {
    pub name: String,
    pub cpu_percent: f64,
    /// Peak of the measured intervals, not an instantaneous peak.
    pub cpu_peak_percent: f64,
    pub rss_bytes: u64,
    pub processes: usize,
    /// Share of observed GPU counter growth; never a device utilization percentage.
    pub gpu_share_percent: Option<f64>,
    pub terminals: Vec<String>,
}

/// How many apps the ranking shows; the summary, the view and the core stack share it.
pub const RANKED: usize = 15;

impl Diagnostics {
    /// Apps with measured CPU, highest first, as ranked everywhere.
    pub fn ranked(&self) -> impl Iterator<Item = &AppUsage> {
        self.apps.iter().filter(|a| a.cpu_percent > 0.).take(RANKED)
    }
    /// Cores busy outside the ranked apps: apps below the cut, or processes that started
    /// and exited between samples. None when the core count or machine CPU is unreadable.
    pub fn unattributed_cores(&self) -> Option<f64> {
        let busy = self.cpu_percent? / 100. * self.cpu_cores? as f64;
        let listed: f64 = self.ranked().map(|a| a.cpu_percent).sum::<f64>() / 100.;
        Some((busy - listed).max(0.))
    }
    /// The text handed to an Agent. `started` is the local start time; `titles` names the
    /// workbench terminals so the Agent reads a tab title, not an internal id.
    pub fn summary(&self, started: &str, titles: &HashMap<String, String>) -> String {
        let mut text = format!(
            "本机资源诊断 · 开始于 {} · 采样 {:.1} 秒 · {}\n",
            started,
            self.elapsed_secs,
            if self.complete {
                "采样结束"
            } else {
                "采样中（部分结果）"
            },
        );
        let percent = |p: Option<f64>| p.map_or_else(|| "不可读".into(), |v| format!("{v:.1}%"));
        let _ = writeln!(
            text,
            "进程数：{}；CPU 核心：{}",
            self.process_count,
            self.cpu_cores
                .map_or_else(|| "不可读".into(), |c| c.to_string())
        );
        let _ = writeln!(
            text,
            "整机 CPU 平均：{}（所有核心合计）；最新 GPU 设备活跃度：{}",
            percent(self.cpu_percent),
            percent(self.gpu_percent)
        );
        let listed: f64 = self.ranked().map(|a| a.cpu_percent).sum::<f64>() / 100.;
        if let (Some(cores), Some(rest)) = (self.cpu_cores, self.unattributed_cores()) {
            let _ = writeln!(
                text,
                "排行前 {} 项合计约 {listed:.1} 核；整机忙碌中约 {rest:.1} / {cores} 核没有归到任何应用（排行之外或采样间隙里的短命进程）。",
                self.ranked().count(),
            );
        }
        text.push_str("\nCPU 排行（100% = 一个核心；内存为当前 RSS，不能当成物理内存总用量）\n");
        for app in self.ranked() {
            let _ = write!(text, "{}：平均 {:.1}%", app.name, app.cpu_percent);
            // One interval gives a peak equal to the average; repeating it is noise.
            if app.cpu_peak_percent > app.cpu_percent + 0.05 {
                let _ = write!(text, "，区间峰值 {:.1}%", app.cpu_peak_percent);
            }
            let _ = write!(
                text,
                "，RSS {}，{} 个进程",
                mib(app.rss_bytes),
                app.processes
            );
            let named: Vec<&str> = app
                .terminals
                .iter()
                .filter_map(|id| titles.get(id).map(String::as_str))
                .collect();
            if !named.is_empty() {
                let _ = write!(text, "，关联终端：{}", named.join("、"));
            }
            let hidden = app.terminals.len() - named.len();
            if hidden > 0 {
                let _ = write!(
                    text,
                    "{}另有 {hidden} 个终端未在工作台打开",
                    if named.is_empty() { "，" } else { "；" }
                );
            }
            text.push('\n');
        }
        text.push_str("\nGPU 活动排行（只占已观测计数增量的份额，不是整颗 GPU 利用率）\n");
        let gpu = self.gpu_ranked();
        if gpu.is_empty() {
            text.push_str("没有可比较的 GPU 计数增量；不代表 GPU 空闲。\n");
        }
        for app in gpu {
            let _ = writeln!(
                text,
                "{}：已观测份额 {:.1}%",
                app.name,
                app.gpu_share_percent.unwrap_or_default()
            );
        }
        for note in &self.notes {
            let _ = writeln!(text, "说明：{note}");
        }
        text.push_str("\n请根据以上测量分析占用来源，区分已证实、推测与未知。需要解释根因时先收集对应日志、流量或调用栈，不要仅凭排行判断卡死或直接结束进程。\n");
        text
    }
    /// Apps with a GPU share, largest first, at most ten.
    pub fn gpu_ranked(&self) -> Vec<&AppUsage> {
        let mut gpu: Vec<_> = self
            .apps
            .iter()
            .filter(|a| a.gpu_share_percent.is_some())
            .collect();
        gpu.sort_by(|a, b| {
            b.gpu_share_percent
                .unwrap_or_default()
                .total_cmp(&a.gpu_share_percent.unwrap_or_default())
        });
        gpu.truncate(10);
        gpu
    }
}

/// RSS as people read it: MB below a gigabyte, one decimal of GB above.
pub fn mib(bytes: u64) -> String {
    let mib = bytes as f64 / 1_048_576.;
    if mib >= 1024. {
        format!("{:.1} GB", mib / 1024.)
    } else {
        format!("{mib:.0} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, cpu: f64, terminals: &[&str]) -> AppUsage {
        AppUsage {
            name: name.into(),
            cpu_percent: cpu,
            cpu_peak_percent: cpu,
            rss_bytes: 2048 * 1_048_576,
            processes: 3,
            terminals: terminals.iter().map(|t| t.to_string()).collect(),
            ..AppUsage::default()
        }
    }

    #[test]
    fn summary_names_terminals_and_unattributed_cores() {
        let report = Diagnostics {
            cpu_cores: Some(16),
            cpu_percent: Some(100.),
            apps: vec![
                app("ffmpeg", 191.5, &[]),
                app("claude", 28.7, &["t1", "t2"]),
            ],
            ..Diagnostics::default()
        };
        let titles = HashMap::from([("t1".to_string(), "keepline".to_string())]);
        let text = report.summary("13:04:00", &titles);
        assert!(text.contains("开始于 13:04:00"));
        assert!(text.contains("关联终端：keepline；另有 1 个终端未在工作台打开"));
        assert!(!text.contains("t1"));
        assert!(!text.contains("区间峰值"), "equal peak is omitted");
        assert!(text.contains("约 13.8 / 16 核没有归到任何应用"));
        assert!(text.contains("RSS 2.0 GB"));
        assert_eq!(
            report.unattributed_cores().map(|c| (c * 10.).round()),
            Some(138.)
        );
    }

    #[test]
    fn unknown_core_count_leaves_no_attribution_claim() {
        let report = Diagnostics {
            cpu_percent: Some(100.),
            apps: vec![app("ffmpeg", 50., &[])],
            ..Diagnostics::default()
        };
        assert_eq!(report.unattributed_cores(), None);
        assert!(!report.summary("", &HashMap::new()).contains("没有归到"));
    }
}
