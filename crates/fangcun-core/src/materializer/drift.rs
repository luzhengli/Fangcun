//! 漂移（F4、6.3、6.9、M1-15）：受管副本被改了，和锁里的内容哈希对不上。
//!
//! - **入库模式**：项目里 `.agents/skills/<名称>` 是实体目录，比它和台账里记的哈希。
//! - **链接模式**：项目里是指向资产库正本的软链，Agent 改的其实是正本，所有链到它的项目一起变。
//!   正本目录的名字就是它该有的哈希，比这两个（9.3：监听正本，处理时先保存再还原）。
//! - 开发模式（`[dev]`）链到本地来源目录，不报漂移（7.4）。
//!
//! 三种处理都出变更计划（9.3）：
//! - **丢弃**：入库模式按资产库正本写回；链接模式从来源按锁取回原版，写回正本。
//!   改过的内容进快照，对这条记录回滚就能找回（6.7 回滚规则第 4 条）。
//! - **分叉**：清单和锁里去掉这一项，`.agents/skills/<名称>` 变成项目自己的实体目录，提交进仓库；
//!   链接模式的正本还原成原版，免得别的项目跟着变。
//! - **回流**：只对 `local:` 来源。修改写回来源目录、在那里提交，本项目 pin 到新提交；
//!   分支在应用后才挪（[`crate::library::source::Sources::finish_promote`]）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::ledger::{Ledger, PathKind};
use super::{PlanBuilder, State};
use crate::Result;
use crate::library::content::{hash_dir, hashed_files};
use crate::model::ContentHash;

/// 一处漂移。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drifted {
    pub asset: String,
    /// 项目里受管的那个路径，相对项目根目录。
    pub path: String,
    /// 链接模式：被改的资产库正本。入库模式为空。
    pub object: Option<PathBuf>,
    /// 锁里（台账里）记的哈希。
    pub locked: ContentHash,
    pub current: ContentHash,
}

impl Drifted {
    /// 改过之后的内容在哪：链接模式是正本，入库模式是项目里的目录。
    pub fn modified_dir(&self, root: &Path) -> PathBuf {
        self.object.clone().unwrap_or_else(|| root.join(&self.path))
    }
}

/// 漂移的三种处理（F4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DriftChoice {
    /// 回流到来源。
    Promote,
    /// 丢弃修改。
    Discard,
    /// 分叉为项目专属。
    Fork,
}

/// 按台账找这个项目的漂移。`objects` 是资产库的正本目录。
pub fn detect(root: &Path, ledger: &Ledger, objects: &Path) -> Vec<Drifted> {
    let mut out = Vec::new();
    for owned in &ledger.paths {
        match owned.kind {
            PathKind::SkillDir => {
                let Some(locked) = &owned.hash else {
                    continue;
                };
                let path = root.join(&owned.path);
                if State::read(&path).ok() != Some(State::Dir) {
                    continue;
                }
                let current = hash_dir(&path);
                if current.0 != *locked {
                    out.push(Drifted {
                        asset: owned.asset.clone(),
                        path: owned.path.clone(),
                        object: None,
                        locked: ContentHash(locked.clone()),
                        current,
                    });
                }
            }
            PathKind::SkillLink => {
                let Some(target) = owned.target.as_deref().map(PathBuf::from) else {
                    continue;
                };
                if !target.starts_with(objects) || !target.is_dir() {
                    continue;
                }
                let Some(name) = target.file_name().map(|n| n.to_string_lossy().into_owned())
                else {
                    continue;
                };
                let current = hash_dir(&target);
                if current.0 != name {
                    out.push(Drifted {
                        asset: owned.asset.clone(),
                        path: owned.path.clone(),
                        object: Some(target),
                        locked: ContentHash(name),
                        current,
                    });
                }
            }
            PathKind::ToolLink | PathKind::Mirror => {}
        }
    }
    out
}

/// 把 `dest` 目录改成和 `from` 一样：多出来的文件删掉，不同的改写，一样的不动。
/// 只列一行 `detail`，里面的文件不单列。返回改了几个文件。
pub fn replace_dir(
    builder: &mut PlanBuilder,
    dest: &Path,
    from: &Path,
    detail: String,
) -> Result<usize> {
    let files = hashed_files(from);
    let wanted: BTreeSet<PathBuf> = files.iter().map(|f| dest.join(&f.rel)).collect();
    let current = State::read(dest)?;
    let mut changed = 0;
    builder.set_quiet(true);
    let result = (|| -> Result<()> {
        if current == State::Dir {
            remove_extra(builder, dest, &wanted, &mut changed)?;
        }
        Ok(())
    })();
    builder.set_quiet(false);
    result?;
    // 目录这一行：原来是软链时换成目录，原来就是目录时不另加动作，把说明挂在第一个文件上。
    let as_dir = current != State::Dir;
    if as_dir {
        builder.dir(dest, detail.clone())?;
    }
    let mut first = !as_dir;
    builder.set_quiet(!first);
    for file in &files {
        let target = dest.join(&file.rel);
        let same = current == State::Dir
            && State::read(&target).ok()
                == Some(State::File(crate::library::content::hash_bytes(
                    &file.bytes,
                )));
        if same {
            continue;
        }
        let mode = super::snapshot::file_mode(&file.source);
        let line = if first { detail.clone() } else { String::new() };
        builder.write_file_mode(&target, file.bytes.clone(), mode, line)?;
        changed += 1;
        if first {
            first = false;
            builder.set_quiet(true);
        }
    }
    builder.set_quiet(false);
    Ok(changed)
}

fn remove_extra(
    builder: &mut PlanBuilder,
    dir: &Path,
    keep: &BTreeSet<PathBuf>,
    changed: &mut usize,
) -> Result<()> {
    // 不参与内容哈希的东西（`.git`、`__pycache__`、`metadata.json`）不算漂移，留着。
    const KEPT: &[&str] = &[".git", "__pycache__", "__pypackages__", "metadata.json"];
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|e| e.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for entry in entries {
        if entry
            .file_name()
            .is_some_and(|n| KEPT.contains(&n.to_string_lossy().as_ref()))
        {
            continue;
        }
        match State::read(&entry)? {
            State::Dir => {
                remove_extra(builder, &entry, keep, changed)?;
                if !keep.iter().any(|k| k.starts_with(&entry)) {
                    builder.remove(&entry, String::new())?;
                }
            }
            State::Absent => {}
            _ => {
                if !keep.contains(&entry) {
                    builder.remove(&entry, String::new())?;
                    *changed += 1;
                }
            }
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::materializer::ledger::OwnedPath;
    use std::fs;

    #[test]
    fn finds_vendor_and_link_drift() {
        let base = std::env::temp_dir().join(format!("fangcun-drift-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let root = base.join("proj");
        let objects = base.join("objects");
        let vendored = root.join(".agents/skills/v");
        fs::create_dir_all(&vendored).unwrap();
        fs::write(vendored.join("SKILL.md"), "v1\n").unwrap();
        let v_hash = hash_dir(&vendored);
        let tmp = base.join("tmp");
        fs::create_dir_all(&tmp).unwrap();
        fs::write(tmp.join("SKILL.md"), "l1\n").unwrap();
        let l_hash = hash_dir(&tmp);
        let object = objects.join(&l_hash.0);
        fs::create_dir_all(objects.parent().unwrap()).unwrap();
        fs::create_dir_all(&objects).unwrap();
        fs::rename(&tmp, &object).unwrap();
        let ledger = Ledger {
            project: root.clone(),
            paths: vec![
                OwnedPath {
                    path: ".agents/skills/v".to_owned(),
                    asset: "v".to_owned(),
                    kind: PathKind::SkillDir,
                    target: None,
                    hash: Some(v_hash.0.clone()),
                },
                OwnedPath {
                    path: ".agents/skills/l".to_owned(),
                    asset: "l".to_owned(),
                    kind: PathKind::SkillLink,
                    target: Some(object.to_string_lossy().into_owned()),
                    hash: Some(l_hash.0.clone()),
                },
            ],
            ..Ledger::default()
        };
        assert!(detect(&root, &ledger, &objects).is_empty());
        fs::write(vendored.join("SKILL.md"), "v2\n").unwrap();
        fs::write(object.join("SKILL.md"), "l2\n").unwrap();
        let found = detect(&root, &ledger, &objects);
        let names: Vec<&str> = found.iter().map(|d| d.asset.as_str()).collect();
        assert_eq!(names, ["v", "l"]);
        assert!(found[1].object.is_some());
        fs::remove_dir_all(&base).unwrap();
    }
}
