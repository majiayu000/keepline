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
        assert_eq!(recency(0, 3 * 86_400), "最近 7 天");
        assert_eq!(recency(0, 30 * 86_400), "更早");
    }
}
