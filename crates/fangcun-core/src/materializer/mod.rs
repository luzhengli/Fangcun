//! 物化器（F4、7.2）：计划、应用、快照、回滚，并记录所有权。
//!
//! 这是内核里唯一允许改动「方寸目录以外」文件的模块。规则：
//!
//! 1. 预览先于写入。任何落盘动作先得到一份 [`ChangePlan`]，确认后才 [`apply`]。
//! 2. 一切可撤销。每次 `apply` 之前留快照，受影响的每个范围各留一个。
//! 3. 只动自己的东西。写入前查所有权；共享的 JSON 与 TOML 按键合并，不整文件覆盖。
//! 4. 写入范围限定在已登记项目与适配器声明的工具配置目录（7.5），见 [`WriteScope`]。
//! 5. 每次写入记一条到活动记录。
//!
//! M1-04 有的是骨架：计划的动作、范围检查、应用前核对磁盘、快照、写入、失败时还原、记活动。
//! 具体的用例（同步、收编、回滚……）往计划里填动作，见 [`PlanBuilder`]。
//! 所有权台账（`~/.fangcun/state/`）和按键合并随 M1-10 补上。

pub mod adopt;
pub mod ledger;
pub mod rollback;
mod scope;
pub mod snapshot;
pub mod sync;

pub use scope::{Target, WriteScope};
pub use snapshot::SnapshotScope;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::journal::{
    FileOp, Journal, Record, RecordFile, RecordKind, RecordSnapshot, RecordStatus, Trigger,
};
use crate::model::{ContentHash, PlanNote, ProjectId, SnapshotId, Timestamp};
use crate::paths::FangcunHome;
use crate::{Error, Result};

use snapshot::{io, sha256_hex};

/// 变更计划里一行的操作，对应 6.8 线框图的 `+`、`~`、`−`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChangeOp {
    Add,
    Modify,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Change {
    pub op: ChangeOp,
    pub path: PathBuf,
    /// 这一行做什么，例如「链接到 tdd@a1b2c3」「新增键 mcpServers.github」。
    pub detail: String,
}

/// 变更计划（6.8）。同步、收编、升级、回滚、转成套装、从全局移除都用它。
///
/// 界面和命令行看到的是标题和每一行；要执行的动作留在内核里，按 `id` 找回。
/// 所以计划经过 IPC 来回一趟也不会被改写：`apply` 只认内核自己出的计划。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangePlan {
    /// 内核给的编号。应用时凭它找回动作。
    pub id: u32,
    /// 标题，例如「即将对 evals_studio 做 4 处变更」。
    pub title: String,
    pub changes: Vec<Change>,
    /// 计划上方的提示：没纳入的项、要人留意的事。
    pub notes: Vec<PlanNote>,
    /// 不能应用：例如回滚时有文件被后来的变更改过（6.7 回滚规则第 6 条）。原因在 `notes` 里，
    /// 界面把「应用」置灰。
    pub blocked: bool,
    #[serde(skip)]
    #[ts(skip)]
    pub(crate) body: Option<PlanBody>,
}

/// 计划里不给界面看的部分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlanBody {
    pub kind: RecordKind,
    /// 写 `~/.fangcun` 里的文件时，快照记在哪个范围下。
    pub primary: SnapshotScope,
    /// 这份计划登记的项目：应用时算在写入范围里。
    pub registers: Vec<PathBuf>,
    pub command: Option<String>,
    /// 回滚计划撤销的是哪条记录。
    pub reverts: Option<String>,
    pub actions: Vec<Action>,
}

/// 一个路径从什么样变成什么样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Action {
    pub path: PathBuf,
    /// 出计划时磁盘上的样子。应用前再看一遍，不一样就拒绝。
    pub before: State,
    pub after: After,
    /// 上级目录在这份计划里由软链或文件换成目录：这个路径原来「不存在」是推出来的
    /// （磁盘上读会顺着旧软链读到别处），不核对、不另进快照，回滚时由上级目录的快照还原。
    pub implied: bool,
}

/// 一个路径现在的样子。文件只记哈希。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum State {
    Absent,
    File(ContentHash),
    Symlink(PathBuf),
    Dir,
}

impl State {
    /// 不跟随软链。
    pub fn read(path: &Path) -> Result<Self> {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::Absent),
            Err(source) => {
                return Err(Error::Io {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        if meta.file_type().is_symlink() {
            return Ok(Self::Symlink(fs::read_link(path).map_err(io(path))?));
        }
        if meta.is_dir() {
            return Ok(Self::Dir);
        }
        let bytes = fs::read(path).map_err(io(path))?;
        Ok(Self::File(ContentHash(sha256_hex(&bytes))))
    }
}

/// 应用后的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum After {
    Absent,
    /// `mode` 为空时：改已有文件保留原权限，新文件用默认权限。
    File {
        bytes: Vec<u8>,
        mode: Option<u32>,
    },
    Symlink(PathBuf),
    Dir,
}

/// 往计划里填动作。每加一个路径就读一次它现在的样子，作为应用前核对的依据。
///
/// 一份计划里一个路径只出现一次。
#[derive(Debug)]
pub struct PlanBuilder {
    title: String,
    body: PlanBody,
    changes: Vec<Change>,
    notes: Vec<PlanNote>,
    blocked: bool,
    /// 打开时加的动作不单独列一行：例如替换整个目录时里面的每个文件，由目录那一行概括。
    /// 动作照样进快照、照样核对。
    quiet: bool,
}

impl PlanBuilder {
    pub fn new(title: impl Into<String>, kind: RecordKind, primary: SnapshotScope) -> Self {
        Self {
            title: title.into(),
            body: PlanBody {
                kind,
                primary,
                registers: Vec::new(),
                command: None,
                reverts: None,
                actions: Vec::new(),
            },
            changes: Vec::new(),
            notes: Vec::new(),
            blocked: false,
            quiet: false,
        }
    }

    /// 加一条计划上方的提示。
    pub fn note(&mut self, note: PlanNote) {
        self.notes.push(note);
    }

    /// 改标题，例如数完变更再写「同步 3 处变更」。
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// 列出来的行数。
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// 之后加的动作不列行（见 `quiet` 字段），直到再调一次 `set_quiet(false)`。
    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    /// 这份计划不能应用，原因写在提示里。
    pub fn block(&mut self) {
        self.blocked = true;
    }

    /// 这是一次回滚，撤销的是 `record` 那条记录。
    pub fn reverts(&mut self, record: impl Into<String>) {
        self.body.reverts = Some(record.into());
    }

    /// 计划里已经有动作的路径。
    pub fn touches(&self, path: &Path) -> bool {
        self.body.actions.iter().any(|a| a.path == path)
    }

    /// 计划里有没有要写的东西。
    pub fn is_empty(&self) -> bool {
        self.body.actions.is_empty()
    }

    #[must_use]
    pub fn command(mut self, command: impl Into<String>) -> Self {
        self.body.command = Some(command.into());
        self
    }

    /// 这份计划会登记这个项目，应用时它算在写入范围里。
    pub fn registers(&mut self, root: impl Into<PathBuf>) {
        self.body.registers.push(root.into());
    }

    pub fn write_file(
        &mut self,
        path: &Path,
        bytes: Vec<u8>,
        detail: impl Into<String>,
    ) -> Result<()> {
        self.push(path, After::File { bytes, mode: None }, detail.into())
    }

    /// 写文件并指定权限，例如入库模式复制 skill 里的脚本。
    pub fn write_file_mode(
        &mut self,
        path: &Path,
        bytes: Vec<u8>,
        mode: u32,
        detail: impl Into<String>,
    ) -> Result<()> {
        let after = After::File {
            bytes,
            mode: Some(mode),
        };
        self.push(path, after, detail.into())
    }

    /// 建一个目录（原来是软链或文件时先删掉）。
    pub fn dir(&mut self, path: &Path, detail: impl Into<String>) -> Result<()> {
        self.push(path, After::Dir, detail.into())
    }

    pub fn symlink(&mut self, path: &Path, target: &Path, detail: impl Into<String>) -> Result<()> {
        self.push(path, After::Symlink(target.to_path_buf()), detail.into())
    }

    /// 删一个文件、软链或空目录。删整棵目录树时，由调用方先逐个删里面的。
    pub fn remove(&mut self, path: &Path, detail: impl Into<String>) -> Result<()> {
        self.push(path, After::Absent, detail.into())
    }

    fn push(&mut self, path: &Path, after: After, detail: String) -> Result<()> {
        if !matches!(after, After::Absent) {
            self.ensure_parents(path)?;
        }
        let implied = self.under_replaced(path);
        let before = if implied {
            State::Absent
        } else {
            State::read(path)?
        };
        let op = match (&before, &after) {
            (State::Absent, _) => ChangeOp::Add,
            (_, After::Absent) => ChangeOp::Remove,
            _ => ChangeOp::Modify,
        };
        if !self.quiet {
            self.changes.push(Change {
                op,
                path: path.to_path_buf(),
                detail,
            });
        }
        self.body.actions.push(Action {
            path: path.to_path_buf(),
            before,
            after,
            implied,
        });
        Ok(())
    }

    /// 某个上级目录在这份计划里由软链或文件换成了目录。
    pub(crate) fn under_replaced(&self, path: &Path) -> bool {
        path.ancestors().skip(1).any(|dir| {
            self.body.actions.iter().any(|a| {
                a.path == dir
                    && a.after == After::Dir
                    && matches!(a.before, State::Symlink(_) | State::File(_))
            })
        })
    }

    /// 缺的上层目录也写成动作，这样它们进快照，回滚时一起删掉。
    fn ensure_parents(&mut self, path: &Path) -> Result<()> {
        let mut missing = Vec::new();
        for dir in path.ancestors().skip(1) {
            let planned = self
                .body
                .actions
                .iter()
                .any(|a| a.path == dir && a.after == After::Dir);
            if planned {
                break;
            }
            let replaced = self.under_replaced(dir);
            if !replaced && !matches!(State::read(dir)?, State::Absent) {
                break;
            }
            missing.push((dir.to_path_buf(), replaced));
        }
        for (dir, implied) in missing.into_iter().rev() {
            self.body.actions.push(Action {
                path: dir,
                before: State::Absent,
                after: After::Dir,
                implied,
            });
        }
        Ok(())
    }

    pub fn build(self, id: u32) -> ChangePlan {
        ChangePlan {
            id,
            title: self.title,
            changes: self.changes,
            notes: self.notes,
            blocked: self.blocked,
            body: Some(self.body),
        }
    }
}

/// 应用一份计划的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Applied {
    /// 受影响的每个范围各一个快照。
    pub snapshots: Vec<SnapshotId>,
}

/// 应用时要用到的内核状态。
pub(crate) struct ApplyContext<'a> {
    pub home: &'a FangcunHome,
    pub scope: &'a WriteScope,
    pub journal: &'a Journal,
    pub trigger: Trigger,
}

/// 应用计划：查范围、核对磁盘、留快照、写入、记活动。
///
/// 任何一步不通过，一个文件都不写。写到一半出错时，按快照把已经写过的还原。
pub(crate) fn apply(plan: &ChangePlan, ctx: &ApplyContext<'_>) -> Result<Applied> {
    let body = plan.body.as_ref().ok_or(Error::PlanNotFound(plan.id))?;
    if plan.blocked {
        let why = plan
            .notes
            .iter()
            .find(|n| n.tone != crate::model::NoteTone::Info)
            .or(plan.notes.first());
        return Err(Error::Blocked(
            why.map_or_else(|| plan.title.clone(), |n| n.text.clone()),
        ));
    }

    // 1. 范围：计划登记的项目也算。
    let mut scope = ctx.scope.clone();
    scope.add_projects(body.registers.iter().cloned());
    let mut groups: BTreeMap<SnapshotScope, Vec<&Action>> = BTreeMap::new();
    for action in &body.actions {
        let creates_dir = action.before == State::Absent && action.after == After::Dir;
        // 推出来的路径照样查范围，但不进快照（见 `Action::implied`）。
        if action.implied {
            scope
                .check_create_dir(&action.path)
                .or_else(|_| scope.check(&action.path))?;
            continue;
        }
        let target = if creates_dir {
            scope.check_create_dir(&action.path)?
        } else {
            scope.check(&action.path)?
        };
        let group = match target {
            Target::Own => body.primary.normalized(),
            Target::Project(root) => SnapshotScope::Project(root),
            Target::Global => SnapshotScope::Global,
        };
        groups.entry(group).or_default().push(action);
    }

    // 2. 核对：出计划之后磁盘变了，让调用方重新出计划。
    for action in body.actions.iter().filter(|a| !a.implied) {
        if State::read(&action.path)? != action.before {
            return Err(Error::StalePlan(action.path.clone()));
        }
    }

    // 3. 快照：每个范围一个。
    let mut snapshots = Vec::new();
    let mut saved = Vec::new();
    for (group, actions) in &groups {
        let paths: Vec<&Path> = actions.iter().map(|a| a.path.as_path()).collect();
        let id = snapshot::capture(ctx.home, group, &plan.title, &paths)?;
        saved.extend(snapshot::load(ctx.home, group, id)?.entries);
        snapshots.push((group.clone(), id));
    }

    // 4. 写入。出错时倒着还原已经动过的。
    for (done, action) in body.actions.iter().enumerate() {
        if let Err(error) = execute(action) {
            for undo in body.actions[..=done].iter().rev() {
                if let Some(entry) = saved.iter().find(|e| e.path == undo.path) {
                    let _ = snapshot::restore(ctx.home, entry);
                }
            }
            return Err(error);
        }
    }

    // 5. 把写入后的样子补进快照：回滚时据此判断后来有没有别的变更动过（6.7 回滚规则第 6 条）。
    for (group, id) in &snapshots {
        let _ = snapshot::record_after(ctx.home, group, *id);
    }

    // 6. 活动记录。写不进去时留在内存里，文件已经写了，不算失败。
    let _ = ctx
        .journal
        .record(record(plan, body, &snapshots, ctx.trigger));
    Ok(Applied {
        snapshots: snapshots.into_iter().map(|(_, id)| id).collect(),
    })
}

fn execute(action: &Action) -> Result<()> {
    let path = &action.path;
    match &action.after {
        After::Absent => snapshot::remove(path),
        After::Dir => {
            if matches!(action.before, State::File(_) | State::Symlink(_)) {
                snapshot::remove(path)?;
            }
            fs::create_dir_all(path).map_err(io(path))
        }
        After::Symlink(target) => {
            snapshot::remove(path)?;
            snapshot::symlink(target, path)
        }
        After::File { bytes, mode } => {
            // 没指定权限时，改已有的文件保留原来的权限。
            let mode = mode.or_else(|| {
                matches!(action.before, State::File(_)).then(|| snapshot::file_mode(path))
            });
            if matches!(action.before, State::Symlink(_) | State::Dir) {
                snapshot::remove(path)?;
            }
            write_atomic(path, bytes, mode)
        }
    }
}

/// 写到同目录的临时文件再改名。改名不跟随软链，目标是软链时替换的是链接本身。
pub(crate) fn write_atomic(path: &Path, bytes: &[u8], mode: Option<u32>) -> Result<()> {
    let name = path
        .file_name()
        .map_or_else(Default::default, |n| n.to_string_lossy());
    let tmp = path.with_file_name(format!(".{name}.fangcun-{}.tmp", std::process::id()));
    fs::write(&tmp, bytes).map_err(io(&tmp))?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode)).map_err(io(&tmp))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    fs::rename(&tmp, path).map_err(|source| {
        let _ = fs::remove_file(&tmp);
        Error::Io {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn record(
    plan: &ChangePlan,
    body: &PlanBody,
    snapshots: &[(SnapshotScope, SnapshotId)],
    trigger: Trigger,
) -> Record {
    let projects: Vec<&PathBuf> = snapshots
        .iter()
        .filter_map(|(scope, _)| match scope {
            SnapshotScope::Project(root) => Some(root),
            SnapshotScope::Global => None,
        })
        .collect();
    let has_global = snapshots.iter().any(|(s, _)| *s == SnapshotScope::Global);
    let name = |root: &PathBuf| {
        root.file_name().map_or_else(
            || root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    };
    let project_label = match (projects.as_slice(), has_global) {
        ([], _) => "全局层".to_owned(),
        ([one], false) => name(one),
        ([first, ..], _) => format!("{} 等 {} 个范围", name(first), snapshots.len()),
    };
    let at = Timestamp::now();
    Record {
        id: format!("{}-{}-{}", kind_slug(body.kind), at.0, plan.id),
        at,
        kind: body.kind,
        title: plan.title.clone(),
        project: match projects.as_slice() {
            [one] if !has_global => Some(ProjectId(one.to_string_lossy().into_owned())),
            _ => None,
        },
        project_label,
        trigger,
        // 跨范围的记录列表里显示第一个编号（6.7「快照」）。
        snapshot: snapshots.first().map(|(_, id)| *id),
        snapshots: snapshots
            .iter()
            .map(|(scope, id)| RecordSnapshot {
                project: match scope {
                    SnapshotScope::Project(root) => {
                        Some(ProjectId(root.to_string_lossy().into_owned()))
                    }
                    SnapshotScope::Global => None,
                },
                id: *id,
            })
            .collect(),
        reverts: body.reverts.clone(),
        status: RecordStatus::Applied,
        summary: format!("{} 处变更 · 已留快照", plan.changes.len()),
        files: plan
            .changes
            .iter()
            .map(|c| RecordFile {
                op: match c.op {
                    ChangeOp::Add => FileOp::Add,
                    ChangeOp::Modify => FileOp::Modify,
                    ChangeOp::Remove => FileOp::Remove,
                },
                path: c.path.display().to_string(),
                note: Some(c.detail.clone()),
            })
            .collect(),
        command: body.command.clone(),
        output: None,
    }
}

fn kind_slug(kind: RecordKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "write".to_owned())
}

#[cfg(all(test, unix))]
mod tests;
