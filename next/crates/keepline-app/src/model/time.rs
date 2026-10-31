//! Time labels: how long ago something happened, in the words the views show.
use std::time::{SystemTime, UNIX_EPOCH};

const DAY: u64 = 86_400;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
/// Local wall-clock time of a Unix timestamp, as `13:04:00`.
pub fn clock(at: u64) -> String {
    let mut buf = [0u8; 16];
    // SAFETY: localtime_r and strftime write only into the locals handed to them.
    let len = unsafe {
        let at = at as libc::time_t;
        let mut tm = std::mem::zeroed::<libc::tm>();
        libc::localtime_r(&at, &mut tm);
        libc::strftime(
            buf.as_mut_ptr().cast(),
            buf.len(),
            c"%H:%M:%S".as_ptr(),
            &tm,
        )
    };
    String::from_utf8_lossy(&buf[..len]).into_owned()
}
/// When a future moment falls, as a person would say it: `今天 22:37`, `明天 09:00`,
/// `周五 07:15` within the week, `11月8日` beyond it.
pub fn upcoming(at: u64, now: u64) -> String {
    let local = |t: u64| {
        // SAFETY: localtime_r writes only into the local handed to it.
        unsafe {
            let t = t as libc::time_t;
            let mut tm = std::mem::zeroed::<libc::tm>();
            libc::localtime_r(&t, &mut tm);
            tm
        }
    };
    let (then, today) = (local(at), local(now));
    let hm = format!("{:02}:{:02}", then.tm_hour, then.tm_min);
    // Day numbers since the epoch in local time, so midnight is the boundary.
    let day = |t: u64, tm: &libc::tm| (t as i64 + tm.tm_gmtoff) / DAY as i64;
    match day(at, &then) - day(now, &today) {
        d if d <= 0 => format!("今天 {hm}"),
        1 => format!("明天 {hm}"),
        d if d < 7 => {
            let names = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
            format!("{} {hm}", names[then.tm_wday.clamp(0, 6) as usize])
        }
        _ => format!("{}月{}日", then.tm_mon + 1, then.tm_mday),
    }
}
pub fn age(at: u64) -> String {
    let minutes = now().saturating_sub(at) / 60;
    if minutes == 0 {
        "刚刚".into()
    } else if minutes < 60 {
        format!("{minutes} 分钟")
    } else if minutes < 1440 {
        format!("{} 小时", minutes / 60)
    } else {
        format!("{} 天", minutes / 1440)
    }
}
/// A stretch of time, as `86 分钟` or `3 小时 5 分`.
pub fn span_label(secs: u64) -> String {
    match secs / 60 {
        0 => format!("{secs} 秒"),
        m if m < 120 => format!("{m} 分钟"),
        m if m < 2 * 1440 => format!("{} 小时 {} 分", m / 60, m % 60),
        m => format!("{} 天", m / 1440),
    }
}
pub fn recency(updated: u64, now: u64) -> &'static str {
    match now.saturating_sub(updated) {
        age if age < DAY => "最近 24 小时",
        age if age < 7 * DAY => "最近 7 天",
        _ => "更早",
    }
}
pub fn ago(updated: u64, now: u64) -> String {
    match now.saturating_sub(updated) {
        age if age < 60 => "刚刚".into(),
        age if age < 3600 => format!("{} 分钟前", age / 60),
        age if age < DAY => format!("{} 小时前", age / 3600),
        age if age < 30 * DAY => format!("{} 天前", age / DAY),
        _ => "超过 30 天".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_time_labels() {
        assert_eq!(ago(1000, 1030), "刚刚");
        assert_eq!(ago(0, 720), "12 分钟前");
        assert_eq!(ago(0, 3 * 3600), "3 小时前");
        assert_eq!(recency(0, 3600), "最近 24 小时");
        assert_eq!(span_label(5160), "86 分钟");
        assert_eq!(span_label(3 * 3600 + 300), "3 小时 5 分");
        assert_eq!(span_label(30), "30 秒");
        assert_eq!(span_label(5 * 86_400), "5 天");
        assert_eq!(recency(0, 3 * 86_400), "最近 7 天");
        assert_eq!(recency(0, 30 * 86_400), "更早");
    }
    #[test]
    fn upcoming_labels_count_local_days() {
        let now = 1_793_000_000;
        assert!(upcoming(now + 60, now).ends_with(&clock(now + 60)[..5]));
        assert!(upcoming(now + 2 * DAY, now).starts_with('周'));
        assert!(upcoming(now + 30 * DAY, now).ends_with('日'));
    }
}
