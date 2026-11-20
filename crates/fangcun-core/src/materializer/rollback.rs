//! 回滚（F6、6.7「回滚规则」、M1-11）：撤销一条写入记录，按它留的快照逐字节还原。
//!
//! - 只动那次变更写过的路径（规则 1）。回滚本身也走变更计划、留快照，可以再撤销（规则 2）。
//! - 一条记录跨几个范围时一起回滚（规则 3）。
//! - 快照里每个路径记着写入后的样子（`after`）。现在的样子和它不同，说明后来别的变更动过：
//!   这些行写明「快照 #N 之后也改过这个文件」，计划不能应用，提示先回滚后面那条（规则 6）。
//! - 资产库里的正本不随回滚删除（规则 5）：回滚只还原项目和工具目录里的文件。

use std::collections::BTreeSet;
use std::path::Path;

use super::snapshot::{self, Saved, Snapshot, SnapshotScope};
use super::{PlanBuilder, State};
use crate::journal::{Record, RecordKind, RecordStatus};
use crate::model::{PlanNote, SnapshotId};
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 记录留下的快照：范围和编号。旧记录只有 `snapshot` 和 `project` 两个字段。
pub fn scopes_of(record: &Record) -> Vec<(SnapshotScope, SnapshotId)> {
    if !record.snapshots.is_empty() {
        return record
            .snapshots
            .iter()
            .map(|s| {
                let scope = match &s.project {
                    Some(p) => SnapshotScope::Project(p.0.clone().into()),
                    None => SnapshotScope::Global,
                };
                (scope, s.id)
            })
            .collect();
    }
    match (record.snapshot, &record.project) {
        (Some(id), Some(p)) => vec![(SnapshotScope::Project(p.0.clone().into()), id)],
        (Some(id), None) => vec![(SnapshotScope::Global, id)],
        _ => Vec::new(),
    }
}

/// 回滚前先看能不能回滚（规则 7、密钥、已回滚、快照过期）。
pub fn check(home: &FangcunHome, record: &Record) -> Result<Vec<(SnapshotScope, Snapshot)>> {
    if record.kind == RecordKind::Secret {
        return Err(Error::Blocked(
            "密钥记录不能回滚，钥匙串条目保持现状。要撤销，用「替换」或「删除」再记一条".to_owned(),
        ));
    }
    if record.status == RecordStatus::RolledBack {
        return Err(Error::Blocked(
            "这次变更已被回滚，磁盘状态与上一快照一致".to_owned(),
        ));
    }
    let scopes = scopes_of(record);
    if scopes.is_empty() || record.status != RecordStatus::Applied {
        return Err(Error::Blocked("这条记录没有写入文件，不能回滚".to_owned()));
    }
    let mut out = Vec::new();
    for (scope, id) in scopes {
        let snapshot = snapshot::load(home, &scope, id).map_err(|_| {
            Error::Blocked(format!(
                "快照 #{} 已过期，备份已清理。快照默认保留 30 天，每个项目至少保留最近 20 个，可在设置里调整。记录仍在",
                id.0
            ))
        })?;
        out.push((scope, snapshot));
    }
    Ok(out)
}

/// 后来哪个快照也动过这个路径：给出那条记录的标题和快照编号。参数是范围、路径、这次的快照编号。
pub type LaterWrite<'a> = &'a dyn Fn(&SnapshotScope, &Path, u32) -> Option<(String, u32)>;

/// 填回滚计划：把每个范围的快照里的路径还原成写入前的样子。
pub fn plan(
    home: &FangcunHome,
    secrets: &dyn crate::secrets::SecretStore,
    record: &Record,
    snapshots: &[(SnapshotScope, Snapshot)],
    later_titles: LaterWrite<'_>,
    builder: &mut PlanBuilder,
) -> Result<()> {
    builder.reverts(record.id.clone());
    builder.note(PlanNote::info(format!(
        "撤销「{}」。预览先于写入，只动这次变更写过的文件。回滚本身也会留一个快照，可以再撤销",
        record.title
    )));
    let mut conflicts: Vec<(String, Option<(String, u32)>)> = Vec::new();
    for (scope, snapshot) in snapshots {
        let paths: BTreeSet<&Path> = snapshot.entries.iter().map(|e| e.path.as_path()).collect();
        for entry in snapshot.entries.iter().rev() {
            let path = &entry.path;
            if builder.touches(path) {
                continue;
            }
            let current = snapshot::current(path)?;
            // 是快照里某个目录下面的路径：由那个目录那一行概括，不单独列。
            let nested = path.ancestors().skip(1).any(|a| paths.contains(a));
            let changed_later = entry.after.as_ref().is_some_and(|after| *after != current);
            let mut detail = describe(&entry.state, &current);
            if changed_later {
                let later = later_titles(scope, path, snapshot.id);
                detail = match &later {
                    Some((_, id)) => format!("快照 #{id} 之后也改过这个文件"),
                    None => format!("快照 #{} 之后也改过这个文件", snapshot.id),
                };
                conflicts.push((path.display().to_string(), later));
            }
            // 上级目录这次要从软链还原成目录时，现在读到的是软链那头的东西，照样要写。
            if current == entry.state && !changed_later && !builder.under_replaced(path) {
                continue;
            }
            builder.set_quiet(nested && !changed_later);
            let result = restore_one(home, secrets, builder, entry, &current, detail);
            builder.set_quiet(false);
            match result {
                // 换过明文的文件，钥匙串里已经没有那个密钥：这一行还原不了，整份计划挡住。
                Err(Error::Blocked(why)) => {
                    builder.block();
                    builder.note(PlanNote::warning(why));
                }
                other => other?,
            }
        }
    }
    if !conflicts.is_empty() {
        builder.block();
        let first_later = conflicts.iter().find_map(|(_, later)| later.clone());
        let text = match first_later {
            Some((title, id)) => format!(
                "{} 个文件被后来的变更改过。先回滚「{title}」（快照 #{id}），再回滚这一条。不做自动合并",
                conflicts.len()
            ),
            None => format!(
                "{} 个文件在这次变更之后被改过（不是方寸改的）。不做自动合并，先把它们改回去或另行处理",
                conflicts.len()
            ),
        };
        builder.note(PlanNote::warning(text));
    }
    Ok(())
}

/// 这一行怎么写，例如「移除链接」「还原文件」。
fn describe(saved: &Saved, current: &Saved) -> String {
    match (saved, current) {
        (Saved::Absent, Saved::Symlink { .. }) => "移除链接".to_owned(),
        (Saved::Absent, Saved::Dir) => "移除目录".to_owned(),
        (Saved::Absent, _) => "移除".to_owned(),
        (Saved::Symlink { target }, _) => format!("还原链接 → {}", target.display()),
        (Saved::Dir, Saved::Symlink { .. }) => "还原原来的目录".to_owned(),
        (Saved::Dir, _) => "还原目录".to_owned(),
        (Saved::File { .. }, Saved::Absent) => "恢复文件".to_owned(),
        (Saved::File { .. }, _) => "还原成这次变更之前的内容".to_owned(),
    }
}

fn restore_one(
    home: &FangcunHome,
    secrets: &dyn crate::secrets::SecretStore,
    builder: &mut PlanBuilder,
    entry: &snapshot::Entry,
    current: &Saved,
    detail: String,
) -> Result<()> {
    let path = entry.path.as_path();
    let saved = &entry.state;
    // 现在是目录、要还原成别的：先删里面的（不列行）。目录是这次变更建的，里面的东西也是。
    if *current == Saved::Dir && *saved != Saved::Dir {
        builder.set_quiet(true);
        let result = remove_tree(builder, path);
        builder.set_quiet(false);
        result?;
    }
    match saved {
        Saved::Absent => builder.remove(path, detail),
        Saved::Dir => builder.dir(path, detail),
        Saved::Symlink { target } => builder.symlink(path, target, detail),
        Saved::File { mode, .. } => {
            let bytes = snapshot::content(home, entry, secrets)?;
            builder.write_file_mode(path, bytes, *mode, detail)
        }
    }
}

/// 删掉目录里的全部东西，子目录先删里面的。计划里已经有的路径跳过。
fn remove_tree(builder: &mut PlanBuilder, dir: &Path) -> Result<()> {
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|e| e.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for entry in entries {
        if builder.touches(&entry) {
            continue;
        }
        if State::read(&entry)? == State::Dir {
            remove_tree(builder, &entry)?;
        }
        builder.remove(&entry, String::new())?;
    }
    Ok(())
}
