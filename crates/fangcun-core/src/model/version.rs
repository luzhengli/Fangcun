use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 内容哈希：SHA-256，小写十六进制。目录按相对路径排序后依次喂入路径与内容，
/// 见 `scanner::fsutil::hash_dir`。界面显示前 7 位。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ContentHash(pub String);

/// 版本 = 来源提交 + 内容哈希（4.4）。有 git 标签显示标签，没有显示日期与短哈希。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Version {
    pub hash: ContentHash,
    /// 来源提交。来源未知或不是 git 时为空。
    pub commit: Option<String>,
    pub tag: Option<String>,
}

/// 跟随方式（4.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case", tag = "policy", content = "value")]
#[ts(export)]
pub enum VersionPolicy {
    /// 固定：锁死在某个版本，只有手动才变。
    Pin(String),
    /// 跟随：跟着某个分支，有新版本时提醒，确认后才更新。
    Track(String),
}

/// 物化方式（F4），清单里的 `mode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SyncMode {
    /// 链接：项目目录里是指向资产库的软链。
    Link,
    /// 入库：实体文件提交进仓库。
    Vendor,
}
