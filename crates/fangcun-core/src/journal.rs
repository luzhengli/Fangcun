//! 活动记录（6.7、7.2）：`~/.fangcun/journal`，活动页和 `fangcun log` 读的就是它。
//!
//! 只追加。桌面应用、命令行、文件监听产生的记录都落在这里。
//!
//! M0 不写任何文件（D-15 还没定），所以这一期的记录只留在内存里：
//! 本次运行做过的体检。`journal/records.jsonl` 存在时也会读出来，格式是一行一条 [`Record`] 的 JSON，
//! 写入在 M1 实现。

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{ProjectId, SnapshotId, Timestamp};
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 记录文件名，在 `~/.fangcun/journal/` 下。
pub const RECORDS_FILE: &str = "records.jsonl";

/// 记录的类型（6.7「类型」表）。顺序与活动页的筛选芯片一致，「可更新」没有芯片。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RecordKind {
    /// 同步：按清单与锁物化，包括登记项目。
    Sync,
    /// 漂移：受管副本被修改。
    Drift,
    /// 升级：资产换到新版本。
    Upgrade,
    /// 可更新：来源有新版本，等确认。
    Updatable,
    /// 收编：副本归并进资产库。
    Adopt,
    /// 回滚：撤销某次写入。
    Rollback,
    /// 体检：只读扫描。
    Checkup,
    /// 校验：在本机运行 `fangcun sync --frozen` 的结果。
    Verify,
    /// 密钥：存入、替换、删除钥匙串条目。只记名称和位置，从不记值。
    Secret,
}

/// 记录的状态（6.7「状态」表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum RecordStatus {
    /// 已应用：已写入文件并留了快照。
    Applied,
    /// 待处理：等人决定，置顶显示。
    Pending,
    /// 失败：没有写入。
    Failed,
    /// 已回滚：这次写入后来被回滚了。
    RolledBack,
    /// 已记录：监听到的变化，没有写入任何文件。
    Recorded,
    /// 只读：体检扫描。
    ReadOnly,
}

/// 触发方，五种（6.7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Trigger {
    /// 桌面应用。
    Desktop,
    /// 命令行。
    Cli,
    /// 文件监听。
    Watcher,
    /// 上游检查。
    UpstreamCheck,
    /// 首次启动。
    FirstLaunch,
}

/// 写入了哪些文件里一行的符号：`+` 新增、`~` 修改、`−` 删除、`=` 只读或未改动。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FileOp {
    Add,
    Modify,
    Remove,
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecordFile {
    pub op: FileOp,
    pub path: String,
    pub note: Option<String>,
}

/// 一条活动记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Record {
    pub id: String,
    pub at: Timestamp,
    pub kind: RecordKind,
    pub title: String,
    /// 跨项目或全局层的记录没有单一项目。
    pub project: Option<ProjectId>,
    /// 列表第二行的项目写法：项目名、「evals_studio 等 5 个项目」或「全局层」。
    pub project_label: String,
    pub trigger: Trigger,
    pub snapshot: Option<SnapshotId>,
    pub status: RecordStatus,
    /// 检查器里的一句说明。
    pub summary: String,
    /// 写入了哪些文件。没有快照时是「涉及的文件」。
    pub files: Vec<RecordFile>,
    /// 做同一件事的命令。
    pub command: Option<String>,
    /// 命令的输出。
    pub output: Option<String>,
}

/// 活动页的筛选：类型与项目可以叠加。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecordFilter {
    pub kind: Option<RecordKind>,
    pub project: Option<ProjectId>,
}

impl RecordFilter {
    pub fn matches(&self, record: &Record) -> bool {
        let kind_ok = match self.kind {
            None => true,
            // 「升级」芯片同时包含「可更新」（6.7）。
            Some(RecordKind::Upgrade) => {
                matches!(record.kind, RecordKind::Upgrade | RecordKind::Updatable)
            }
            Some(kind) => record.kind == kind,
        };
        let project_ok = self
            .project
            .as_ref()
            .is_none_or(|p| record.project.as_ref() == Some(p));
        kind_ok && project_ok
    }
}

#[derive(Debug)]
pub struct Journal {
    file: PathBuf,
    memory: Mutex<Vec<Record>>,
}

impl Journal {
    /// 只算路径，不创建目录。
    pub fn open(home: &FangcunHome) -> Self {
        Self {
            file: home.journal_dir().join(RECORDS_FILE),
            memory: Mutex::new(Vec::new()),
        }
    }

    /// 新的在前。
    pub fn read(&self, filter: &RecordFilter) -> Result<Vec<Record>> {
        let mut records = self.persisted()?;
        if let Ok(memory) = self.memory.lock() {
            records.extend(memory.iter().cloned());
        }
        records.retain(|r| filter.matches(r));
        records.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
        Ok(records)
    }

    /// 待处理总数。侧栏「活动」旁的数字和菜单栏角标用它，不受筛选影响。
    pub fn pending_count(&self) -> Result<u32> {
        let pending = self
            .read(&RecordFilter::default())?
            .iter()
            .filter(|r| r.status == RecordStatus::Pending)
            .count();
        Ok(u32::try_from(pending).unwrap_or(u32::MAX))
    }

    /// 只留在本次运行的内存里，不落盘。M0 的体检记录走这里。
    pub fn remember(&self, record: Record) {
        if let Ok(mut memory) = self.memory.lock() {
            memory.push(record);
        }
    }

    /// 本次运行有没有记过任何东西。
    pub fn is_empty(&self) -> bool {
        self.memory.lock().map(|m| m.is_empty()).unwrap_or(true)
    }

    /// 写进 `~/.fangcun/journal`。M1。
    pub fn append(&self, _record: &Record) -> Result<()> {
        Err(Error::NotImplemented("journal::Journal::append"))
    }

    fn persisted(&self) -> Result<Vec<Record>> {
        let Ok(file) = fs::File::open(&self.file) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (index, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source| Error::Io {
                path: self.file.clone(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record = serde_json::from_str(&line).map_err(|e| Error::Parse {
                origin: format!("{} 第 {} 行", self.file.display(), index + 1),
                message: e.to_string(),
            })?;
            out.push(record);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, at: &str, kind: RecordKind) -> Record {
        Record {
            id: id.to_owned(),
            at: Timestamp(at.to_owned()),
            kind,
            title: String::new(),
            project: None,
            project_label: "全局层".to_owned(),
            trigger: Trigger::Cli,
            snapshot: None,
            status: RecordStatus::ReadOnly,
            summary: String::new(),
            files: Vec::new(),
            command: None,
            output: None,
        }
    }

    #[test]
    fn memory_records_come_back_newest_first_without_touching_disk() {
        let home = FangcunHome::at("/nonexistent/fangcun-journal-test");
        let journal = Journal::open(&home);
        journal.remember(record("a", "2026-11-13T01:00:00Z", RecordKind::Checkup));
        journal.remember(record("b", "2026-11-14T01:00:00Z", RecordKind::Checkup));
        let ids: Vec<String> = journal
            .read(&RecordFilter::default())
            .expect("读内存记录")
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["b", "a"]);
        assert!(!home.root().exists());
    }

    #[test]
    fn upgrade_chip_includes_updatable() {
        let filter = RecordFilter {
            kind: Some(RecordKind::Upgrade),
            project: None,
        };
        assert!(filter.matches(&record("x", "t", RecordKind::Updatable)));
        assert!(!filter.matches(&record("y", "t", RecordKind::Sync)));
    }
}
