use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 资产的六种状态语义（6.10）。界面一律用「圆点加短标签」表达，颜色不新增。
///
/// 全局页、活动页、体检报告各有自己的状态集合，定义在对应模块里，
/// 但颜色都落在这六种语义对应的色上。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AssetState {
    /// 已同步。
    Synced,
    /// 漂移：受管副本被修改。
    Drifted,
    /// 可更新：来源有新版本。
    Updatable,
    /// 脚本变更：升级差异里脚本、hook、MCP 启动命令有变化。
    ScriptChanged,
    /// 来源未知：内容哈希与任何来源都不匹配。
    UnknownSource,
    /// 被覆盖：被下层同名项遮蔽。
    Overridden,
}

/// 估算的 token 数。永远是估算值，界面和命令行展示时必须标「估算」。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, TS,
)]
#[ts(export)]
pub struct TokenEstimate(pub u32);

/// 时间点，UTC 的 RFC 3339 字符串，精确到秒，例如 `2026-11-14T06:26:27Z`。
///
/// 没有引入时间库：内核只需要「现在」「N 天前」和字符串比较，见 DECISIONS 2026-11-14「M0 实现」。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Timestamp(pub String);

impl Timestamp {
    pub fn now() -> Self {
        Self::from_unix(unix_now())
    }

    /// 现在往前 `days` 天。
    pub fn days_ago(days: u32) -> Self {
        Self::from_unix(unix_now() - i64::from(days) * 86_400)
    }

    pub fn from_system_time(time: std::time::SystemTime) -> Self {
        let secs = match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
        };
        Self::from_unix(secs)
    }

    pub fn from_unix(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        let (y, m, d) = civil_from_days(days);
        Self(format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            rem / 3600,
            (rem % 3600) / 60,
            rem % 60
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// 自 1970-01-01 的天数换成公历年月日（Howard Hinnant 的算法）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_seconds_as_rfc3339() {
        assert_eq!(Timestamp::from_unix(0).0, "1970-01-01T00:00:00Z");
        assert_eq!(
            Timestamp::from_unix(1_794_610_127).0,
            "2026-11-13T22:48:47Z"
        );
    }

    #[test]
    fn timestamps_order_as_strings() {
        assert!(Timestamp::days_ago(30) < Timestamp::now());
    }
}
