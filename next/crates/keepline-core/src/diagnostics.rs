//! A short, read-only host measurement. These observations are not workspace facts.
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Diagnostics {
    pub started_at: u64,
    pub elapsed_secs: f64,
    pub complete: bool,
    pub process_count: usize,
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

impl Diagnostics {
    pub fn summary(&self) -> String {
        let mut text = format!(
            "本机资源诊断 · Unix 时间 {} · 采样 {:.1} 秒 · {}\n进程数：{}\n",
            self.started_at,
            self.elapsed_secs,
            if self.complete {
                "采样结束"
            } else {
                "采样中（部分结果）"
            },
            self.process_count,
        );
        let percent = |p: Option<f64>| p.map_or_else(|| "不可读".into(), |v| format!("{v:.1}%"));
        let _ = writeln!(
            text,
            "整机 CPU 平均：{}；最新 GPU 设备活跃度：{}",
            percent(self.cpu_percent),
            percent(self.gpu_percent)
        );
        text.push_str("\nCPU 排行（100% = 一个核心；内存为当前 RSS，不能当成物理内存总用量）\n");
        for app in self.apps.iter().filter(|a| a.cpu_percent > 0.).take(15) {
            let _ = writeln!(
                text,
                "{}：平均 {:.1}%，区间峰值 {:.1}%，RSS {:.0} MiB，{} 个进程{}",
                app.name,
                app.cpu_percent,
                app.cpu_peak_percent,
                app.rss_bytes as f64 / 1_048_576.,
                app.processes,
                if app.terminals.is_empty() {
                    String::new()
                } else {
                    format!("，关联终端 {}", app.terminals.join(", "))
                }
            );
        }
        text.push_str("\nGPU 活动排行（只占已观测计数增量的份额，不是整颗 GPU 利用率）\n");
        let mut gpu: Vec<_> = self
            .apps
            .iter()
            .filter(|a| a.gpu_share_percent.is_some())
            .collect();
        gpu.sort_by(|a, b| {
            b.gpu_share_percent
                .unwrap()
                .total_cmp(&a.gpu_share_percent.unwrap())
        });
        if gpu.is_empty() {
            text.push_str("没有可比较的 GPU 计数增量；不代表 GPU 空闲。\n");
        }
        for app in gpu.into_iter().take(10) {
            let _ = writeln!(
                text,
                "{}：已观测份额 {:.1}%",
                app.name,
                app.gpu_share_percent.unwrap()
            );
        }
        for note in &self.notes {
            let _ = writeln!(text, "说明：{note}");
        }
        text.push_str("\n请根据以上测量分析占用来源，区分已证实、推测与未知。需要解释根因时先收集对应日志、流量或调用栈，不要仅凭排行判断卡死或直接结束进程。\n");
        text
    }
}
