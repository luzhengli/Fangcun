//! 活动记录（6.7、7.2）：`~/.fangcun/journal`，活动页和 `fangcun log` 读的就是它。
//!
//! 只追加。桌面应用、命令行、文件监听产生的记录都落在这里。
//!
//! 格式是 `journal/records.jsonl`，一行一条 [`Record`] 的 JSON。M0 只写这一个目录（D-15）：
//! 记的是体检。目录要等第一条记录才创建；写不进去时（例如没有权限），记录退回到本次运行的内存里，
//! 不让体检因此失败。

use std::fs;
use std::io::{BufRead, BufReader, Write};
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
    /// 列表里显示的快照编号：跨范围的记录取第一个（6.7「快照」）。
    pub snapshot: Option<SnapshotId>,
    /// 这条记录留的全部快照，每个范围一个。检查器里写全，回滚按它找。
    #[serde(default)]
    pub snapshots: Vec<RecordSnapshot>,
    /// 回滚记录：撤销的是哪一条。被撤销的那条读出来时状态是「已回滚」。
    #[serde(default)]
    pub reverts: Option<String>,
    pub status: RecordStatus,
    /// 检查器里的一句说明。
    pub summary: String,
    /// 写入了哪些文件。没有快照时是「涉及的文件」。
    pub files: Vec<RecordFile>,
    /// 做同一件事的命令。
    pub command: Option<String>,
    /// 命令的输出。
    pub output: Option<String>,
    /// 待处理的记录（漂移、可更新）指向哪项资产。读的时候按现状判断还待不待处理，
    /// 处理过了显示「已记录」（6.7「待处理」）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Subject>,
    /// 体检报告一键动作留下的记录：处理的是哪条发现（M1-28）。没回滚就算已处理。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
}

/// 待处理记录指向的资产。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Subject {
    pub asset: String,
    /// 可更新：上游那一版的提交。漂移为空。
    pub version: Option<String>,
}

/// 记录留的一个快照：哪个范围、几号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecordSnapshot {
    /// 项目根目录；全局层为空。
    pub project: Option<ProjectId>,
    pub id: SnapshotId,
}

/// 项目页「历史」页签的一行（6.3）：这个项目的一个快照，和留下它的那条记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    /// 这个项目的快照编号。
    pub snapshot: SnapshotId,
    /// 留下它的活动记录。
    pub record: String,
    pub title: String,
    pub at: Timestamp,
    pub kind: RecordKind,
    pub status: RecordStatus,
    /// 磁盘现在就是这一条写完后的样子：最新的、还算数的那次写入（回滚记录本身不算）。
    pub current: bool,
}

/// 从全部记录（从新到旧）里挑出给这个项目留过快照的，标出「当前磁盘状态」。
/// `same_project` 判断快照的范围是不是这个项目（调用方按真实路径比）。
pub fn history_of(
    newest_first: &[Record],
    same_project: impl Fn(&ProjectId) -> bool,
) -> Vec<HistoryEntry> {
    let mut entries: Vec<HistoryEntry> = newest_first
        .iter()
        .filter_map(|r| {
            let snap = r
                .snapshots
                .iter()
                .find(|s| s.project.as_ref().is_some_and(&same_project))?;
            Some(HistoryEntry {
                snapshot: snap.id,
                record: r.id.clone(),
                title: r.title.clone(),
                at: r.at.clone(),
                kind: r.kind,
                status: r.status,
                current: false,
            })
        })
        .collect();
    if let Some(current) = entries
        .iter_mut()
        .find(|e| e.status == RecordStatus::Applied && e.kind != RecordKind::Rollback)
    {
        current.current = true;
    }
    entries
}

/// 从新到旧走一遍：一条回滚记录自己没被撤销时，它撤销的那条算「已回滚」。
/// 撤销一次回滚（回滚那条回滚记录），原来那条就回到「已应用」。
fn mark_rolled_back(newest_first: &mut [Record]) {
    let mut reverted = std::collections::BTreeSet::new();
    for record in newest_first.iter_mut() {
        let undone = reverted.contains(&record.id);
        if undone && record.status == RecordStatus::Applied {
            record.status = RecordStatus::RolledBack;
        }
        if !undone && let Some(target) = &record.reverts {
            reverted.insert(target.clone());
        }
    }
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
    dir: PathBuf,
    file: PathBuf,
    /// 没写进磁盘的记录。
    memory: Mutex<Vec<Record>>,
}

impl Journal {
    /// 只算路径，不创建目录。
    pub fn open(home: &FangcunHome) -> Self {
        let dir = home.journal_dir();
        Self {
            file: dir.join(RECORDS_FILE),
            dir,
            memory: Mutex::new(Vec::new()),
        }
    }

    /// 新的在前。被回滚过的写入状态是「已回滚」：记录只追加，状态从后来的回滚记录算出来。
    pub fn read(&self, filter: &RecordFilter) -> Result<Vec<Record>> {
        let mut records = self.persisted()?;
        if let Ok(memory) = self.memory.lock() {
            records.extend(memory.iter().cloned());
        }
        // 时间只精确到秒：同一秒里的按写入的先后，后写的在前。
        records.reverse();
        records.sort_by(|a, b| b.at.cmp(&a.at));
        mark_rolled_back(&mut records);
        records.retain(|r| filter.matches(r));
        Ok(records)
    }

    /// 按编号找一条，状态已经算好。
    pub fn find(&self, id: &str) -> Result<Option<Record>> {
        Ok(self
            .read(&RecordFilter::default())?
            .into_iter()
            .find(|r| r.id == id))
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

    /// 记一条：先写进 `~/.fangcun/journal`，写不进去就留在本次运行的内存里。
    /// 返回写盘的错误，调用方可以不理会，记录不会丢。
    pub fn record(&self, record: Record) -> Result<()> {
        match self.append(&record) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.remember(record);
                Err(error)
            }
        }
    }

    /// 只留在本次运行的内存里，不落盘。
    pub fn remember(&self, record: Record) {
        if let Ok(mut memory) = self.memory.lock() {
            memory.push(record);
        }
    }

    /// 一共有几条，包括以前运行留下的。新记录的编号用它保证唯一。
    pub fn count(&self) -> usize {
        self.all().len()
    }

    /// 有没有任何一条某类记录，包括以前运行留下的。
    pub fn has_kind(&self, kind: RecordKind) -> bool {
        self.all().iter().any(|r| r.kind == kind)
    }

    /// 磁盘上的加内存里的。磁盘上的读不出来时只算内存里的。
    fn all(&self) -> Vec<Record> {
        let mut records = self.persisted().unwrap_or_default();
        if let Ok(memory) = self.memory.lock() {
            records.extend(memory.iter().cloned());
        }
        records
    }

    /// 追加一行到 `journal/records.jsonl`，目录不在就建。
    pub fn append(&self, record: &Record) -> Result<()> {
        let io = |path: &PathBuf| {
            let path = path.clone();
            move |source| Error::Io { path, source }
        };
        fs::create_dir_all(&self.dir).map_err(io(&self.dir))?;
        let mut line = serde_json::to_string(record).map_err(|e| Error::Parse {
            origin: "活动记录".to_owned(),
            message: e.to_string(),
        })?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)
            .map_err(io(&self.file))?;
        file.write_all(line.as_bytes()).map_err(io(&self.file))
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
            snapshots: Vec::new(),
            reverts: None,
            status: RecordStatus::ReadOnly,
            summary: String::new(),
            files: Vec::new(),
            command: None,
            output: None,
            subject: None,
            finding: None,
        }
    }

    #[test]
    fn a_rollback_marks_its_target_until_it_is_undone_itself() {
        let home = FangcunHome::at("/nonexistent/fangcun-journal-rollback");
        let journal = Journal::open(&home);
        let mut sync = record("sync", "2026-11-19T10:00:00Z", RecordKind::Sync);
        sync.status = RecordStatus::Applied;
        let mut undo = record("undo", "2026-11-19T11:00:00Z", RecordKind::Rollback);
        undo.status = RecordStatus::Applied;
        undo.reverts = Some("sync".to_owned());
        journal.remember(sync);
        journal.remember(undo);
        let status = |id: &str| journal.find(id).unwrap().unwrap().status;
        assert_eq!(status("sync"), RecordStatus::RolledBack);
        assert_eq!(status("undo"), RecordStatus::Applied);

        // 撤销这次回滚：回滚记录本身成了「已回滚」，原来那条回到「已应用」。
        let mut redo = record("redo", "2026-11-19T12:00:00Z", RecordKind::Rollback);
        redo.status = RecordStatus::Applied;
        redo.reverts = Some("undo".to_owned());
        journal.remember(redo);
        assert_eq!(status("undo"), RecordStatus::RolledBack);
        assert_eq!(status("sync"), RecordStatus::Applied);
    }

    #[test]
    fn history_marks_the_newest_write_that_still_counts_as_current() {
        let project = ProjectId("/p".to_owned());
        let with_snap = |id: &str, at: &str, kind: RecordKind, status: RecordStatus, n: u32| {
            let mut r = record(id, at, kind);
            r.status = status;
            r.snapshots = vec![RecordSnapshot {
                project: Some(project.clone()),
                id: SnapshotId(n),
            }];
            r
        };
        let records = vec![
            with_snap(
                "undo",
                "2026-11-20T03:00:00Z",
                RecordKind::Rollback,
                RecordStatus::Applied,
                3,
            ),
            with_snap(
                "b",
                "2026-11-20T02:00:00Z",
                RecordKind::Sync,
                RecordStatus::RolledBack,
                2,
            ),
            with_snap(
                "a",
                "2026-11-20T01:00:00Z",
                RecordKind::Sync,
                RecordStatus::Applied,
                1,
            ),
            record("checkup", "2026-11-20T00:00:00Z", RecordKind::Checkup),
        ];
        let history = history_of(&records, |p| p == &project);
        let rows: Vec<(&str, bool)> = history
            .iter()
            .map(|e| (e.record.as_str(), e.current))
            .collect();
        assert_eq!(rows, vec![("undo", false), ("b", false), ("a", true)]);
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
    fn recorded_entries_survive_a_new_journal() {
        let root = std::env::temp_dir().join(format!("fangcun-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = FangcunHome::at(&root);
        Journal::open(&home)
            .record(record("a", "2026-11-14T01:00:00Z", RecordKind::Checkup))
            .expect("写进临时目录");
        let reopened = Journal::open(&home);
        assert!(reopened.has_kind(RecordKind::Checkup));
        assert_eq!(reopened.count(), 1);
        let ids: Vec<String> = reopened
            .read(&RecordFilter::default())
            .expect("读记录")
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, ["a"]);
        // 只写了 journal 一个子目录。
        let entries: Vec<_> = fs::read_dir(&root).expect("列目录").collect();
        assert_eq!(entries.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unwritable_journal_falls_back_to_memory() {
        let home = FangcunHome::at("/nonexistent/fangcun-journal-test");
        let journal = Journal::open(&home);
        assert!(
            journal
                .record(record("a", "t", RecordKind::Checkup))
                .is_err()
        );
        assert_eq!(journal.read(&RecordFilter::default()).expect("读").len(), 1);
        assert_eq!(journal.count(), 1);
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
