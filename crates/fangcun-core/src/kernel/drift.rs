//! 漂移的用例（F4、6.3、6.9、M1-15）：检测、给检查器的差异、三种处理的计划、待处理记录。
//! 规则见 [`crate::materializer::drift`]。

use std::path::{Path, PathBuf};

use super::Kernel;
use crate::journal::{FileOp, Record, RecordFile, RecordKind, RecordStatus, Subject, Trigger};
use crate::library::Library;
use crate::library::content::hash_dir;
use crate::library::diff;
use crate::library::source::{Source, Sources};
use crate::manifest::{Lock, LockedSkill};
use crate::materializer::drift::{self, DriftChoice, Drifted};
use crate::materializer::ledger::Ledger;
use crate::materializer::{ChangePlan, PlanBuilder, SnapshotScope};
use crate::model::{PlanNote, ProjectId, SourceRef, Timestamp};
use crate::resolver::DriftInfo;
use crate::watcher::KernelEvent;
use crate::{Error, Result};

/// 原版：入库模式是资产库正本，链接模式是从来源导出的临时目录（用完删掉）。
pub(super) struct Original {
    pub dir: PathBuf,
    temp: bool,
}

impl Drop for Original {
    fn drop(&mut self) {
        if self.temp {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// 锁里这一项的版本怎么写：标签、短提交，都没有时短哈希。
pub(super) fn version_label(locked: Option<&LockedSkill>, hash: &str) -> String {
    match locked {
        Some(l) if l.tag.is_some() => l.tag.clone().unwrap_or_default(),
        Some(l) if l.commit.is_some() => {
            crate::scanner::short_hash(l.commit.as_deref().unwrap_or_default())
        }
        _ => crate::scanner::short_hash(hash),
    }
}

impl Kernel {
    /// 一个已登记项目现在的漂移。
    pub(crate) fn drifts(&self, root: &Path) -> Result<Vec<Drifted>> {
        let ledger = Ledger::load(&self.home, root)?;
        let library = Library::open(&self.home)?;
        Ok(drift::detect(root, &ledger, &library.objects_dir()))
    }

    /// 锁定的那一版原样在哪。取不回时为空（本地来源没有提交、来源里已经没有这个提交）。
    pub(super) fn original(
        &self,
        library: &Library,
        sources: &Sources,
        drifted: &Drifted,
        locked: Option<&LockedSkill>,
    ) -> Option<Original> {
        if drifted.object.is_none() {
            let object = library.locate(&drifted.locked).ok()??;
            return (hash_dir(&object) == drifted.locked).then_some(Original {
                dir: object,
                temp: false,
            });
        }
        let locked = locked?;
        let commit = locked.commit.as_deref()?;
        let staging = library.staging().ok()?;
        let original = Original {
            dir: staging,
            temp: true,
        };
        sources
            .export_version(
                &SourceRef(locked.source.clone()),
                commit,
                &locked.path,
                &original.dir,
            )
            .ok()?;
        (hash_dir(&original.dir) == drifted.locked).then_some(original)
    }

    /// 同样用这一项（同名、同来源）的其他已登记项目。
    fn others_using(
        &self,
        root: &Path,
        asset: &str,
        source: Option<&str>,
    ) -> Vec<(PathBuf, LockedSkill)> {
        let wanted = crate::scanner::fsutil::canonical(root);
        let Ok(registered) = crate::registry::load(&self.home) else {
            return Vec::new();
        };
        registered
            .into_iter()
            .filter(|p| crate::scanner::fsutil::canonical(&p.path) != wanted)
            .filter_map(|p| {
                let lock = Lock::load(&p.path).ok()??;
                let locked = lock.skills.get(asset)?.clone();
                (source.is_none_or(|s| s == locked.source)).then_some((p.path, locked))
            })
            .collect()
    }

    /// 命令行 `fangcun drift`：本项目的漂移和差异（第 5 节）。写了 `asset` 时只看这一项，
    /// 它没有漂移时报「找不到」。
    pub fn drift_report(
        &self,
        root: &Path,
        asset: Option<&str>,
    ) -> Result<Vec<(String, DriftInfo)>> {
        let _ = self.refresh_drift();
        let lock = Lock::load(root)?;
        let drifts = self.drifts(root)?;
        if let Some(a) = asset
            && !drifts.iter().any(|d| d.asset == a)
        {
            return Err(Error::NotFound(format!(
                "{a} 的本地改动（文件与锁定版本一致）"
            )));
        }
        Ok(drifts
            .iter()
            .filter(|d| asset.is_none_or(|a| d.asset == a))
            .map(|d| (d.asset.clone(), self.drift_info(root, d, lock.as_ref())))
            .collect())
    }

    /// 检查器漂移块的数据。
    pub(super) fn drift_info(
        &self,
        root: &Path,
        drifted: &Drifted,
        lock: Option<&Lock>,
    ) -> DriftInfo {
        let locked = lock.and_then(|l| l.skills.get(&drifted.asset));
        let label = version_label(locked, &drifted.locked.0);
        let sources = self.sources();
        let (files, original) = match Library::open(&self.home) {
            Ok(library) => {
                let original = self.original(&library, &sources, drifted, locked);
                let files = diff::diff_dirs(
                    original.as_ref().map(|o| o.dir.as_path()),
                    &drifted.modified_dir(root),
                );
                (files, original.is_some())
            }
            Err(_) => (Vec::new(), false),
        };
        let names: Vec<&str> = files.iter().map(|f| f.path.as_str()).take(3).collect();
        let what = if names.is_empty() {
            "副本".to_owned()
        } else {
            names.join("、")
        };
        let mut summary = format!(
            "Agent 在本项目修改了 {what}，相对 {label} {}。",
            diff::summary(&files)
        );
        if drifted.object.is_some() {
            summary.push_str("改的是资产库里的正本，链到它的项目都跟着变了。");
        }
        if !original {
            summary.push_str("原版无法取回，只能写回源仓库或转为项目独立文件。");
        }
        let promote_blocked = match locked.map(|l| Source::parse(&SourceRef(l.source.clone()))) {
            Some(Ok(Source::Local { .. })) => None,
            Some(Ok(Source::Github { .. })) => {
                Some("GitHub 来源暂不支持写回源仓库，可还原锁定版或转为项目独立文件".to_owned())
            }
            _ => Some("锁里未记录这一项的来源，无法写回源仓库".to_owned()),
        };
        let others = self
            .others_using(root, &drifted.asset, locked.map(|l| l.source.as_str()))
            .into_iter()
            .map(|(p, _)| crate::scanner::project_name(&p))
            .collect();
        DriftInfo {
            locked: label,
            summary,
            files,
            shared: drifted.object.is_some(),
            original,
            promote_blocked,
            others,
        }
    }

    /// 处理一处漂移：回流、丢弃或分叉（F4）。都先出计划。
    pub fn plan_drift(
        &self,
        project_root: &Path,
        asset: &str,
        choice: DriftChoice,
    ) -> Result<ChangePlan> {
        self.require_write("drift")?;
        let root = project_root;
        let (mut project, lock) = self.manifest_and_lock(root)?;
        let drifted = self
            .drifts(root)?
            .into_iter()
            .find(|d| d.asset == asset)
            .ok_or_else(|| Error::NotFound(format!("{asset} 的本地改动（文件与锁定版本一致）")))?;
        let locked = lock
            .skills
            .get(asset)
            .cloned()
            .ok_or_else(|| Error::NotFound(format!("锁里的 {asset}")))?;
        let library = Library::open(&self.home)?;
        let sources = self.sources();
        let label = version_label(Some(&locked), &locked.hash.0);
        let name = crate::scanner::project_name(root);
        let original = self.original(&library, &sources, &drifted, Some(&locked));
        let modified = drifted.modified_dir(root);
        let title = match choice {
            DriftChoice::Discard => format!("还原 {asset} 为锁定版"),
            DriftChoice::Fork => format!("将 {asset} 转为本项目独立文件"),
            DriftChoice::Promote => format!("将 {asset} 写回 {}", locked.source),
        };
        let verb = match choice {
            DriftChoice::Promote => "promote",
            DriftChoice::Discard => "discard",
            DriftChoice::Fork => "fork",
        };
        // 活动页给每条写入附上对应的命令（6.7、第 5 节）。
        let mut builder = PlanBuilder::new(
            title.clone(),
            RecordKind::Drift,
            SnapshotScope::Project(root.to_path_buf()),
        )
        .command(format!("fangcun drift {asset} {verb}"));
        builder.subject(asset, None);
        let in_manifest = project.manifest.skills.contains_key(asset);
        let manifest_file = root.join(crate::paths::MANIFEST_FILE);
        let lock_file = root.join(crate::paths::LOCK_FILE);

        match choice {
            DriftChoice::Discard => {
                let original = original.ok_or_else(|| {
                    Error::Blocked(
                        "原版无法取回（本地来源没有提交），只能写回源仓库或转为项目独立文件"
                            .to_owned(),
                    )
                })?;
                let detail = if drifted.object.is_some() {
                    format!("恢复资产库里的正本 {label}，链到它的项目一起还原；本地改动已存进快照")
                } else {
                    format!("还原为锁定版 {label}；本地改动已存进快照")
                };
                drift::replace_dir(&mut builder, &modified, &original.dir, detail)?;
                builder.note(PlanNote::info(
                    "如需找回被还原的本地修改，在「活动」页对此记录执行回滚即可",
                ));
            }
            DriftChoice::Fork => {
                if !in_manifest {
                    return Err(Error::Blocked(format!(
                        "{asset} 来自套装，暂不支持转为独立文件：先在套装里去掉它"
                    )));
                }
                let text = std::fs::read_to_string(&manifest_file).map_err(|source| Error::Io {
                    path: manifest_file.clone(),
                    source,
                })?;
                let edited =
                    crate::manifest::remove_skill(crate::paths::MANIFEST_FILE, &text, asset)?;
                project.manifest =
                    crate::manifest::parse_manifest(crate::paths::MANIFEST_FILE, &edited)?;
                builder.write_file(
                    &manifest_file,
                    edited.into_bytes(),
                    format!("[skills] 去掉 {asset}，脱离资产库管理"),
                )?;
                let mut new_lock = lock.clone();
                new_lock.skills.remove(asset);
                builder.write_file(
                    &lock_file,
                    new_lock.to_toml()?.into_bytes(),
                    format!("去掉 {asset}"),
                )?;
                if let Some(object) = &drifted.object {
                    let dest = root.join(&drifted.path);
                    drift::replace_dir(
                        &mut builder,
                        &dest,
                        object,
                        "转为本项目独立实体文件，随 Git 提交".to_owned(),
                    )?;
                    match &original {
                        Some(original) => {
                            drift::replace_dir(
                                &mut builder,
                                object,
                                &original.dir,
                                format!("资产库正本恢复为 {label}，其他项目不受影响"),
                            )?;
                        }
                        None => builder.note(PlanNote::warning(
                            "原版无法取回，资产库正本保留修改后的状态，其他软链项目也会看到",
                        )),
                    }
                }
                self.sync_into(
                    &mut builder,
                    &project,
                    &new_lock,
                    &library,
                    &sources,
                    &[asset.to_owned()],
                )?;
            }
            DriftChoice::Promote => {
                let source = SourceRef(locked.source.clone());
                let Ok(Source::Local { name: local }) = Source::parse(&source) else {
                    return Err(Error::Blocked(
                        "GitHub 来源暂不支持写回源仓库，可还原锁定版或转为独立文件".to_owned(),
                    ));
                };
                let Some(decl) = project.manifest.skills.get(asset).cloned() else {
                    return Err(Error::Blocked(format!(
                        "{asset} 来自套装，暂不支持写回源仓库"
                    )));
                };
                let promotion = sources.prepare_promote(
                    &source,
                    &locked.path,
                    &modified,
                    &format!("fangcun: 从 {name} 写回 {asset}"),
                )?;
                let short = promotion.commit[..7].to_owned();
                let src_dir = sources.local_dir(&local)?.join(&locked.path);
                drift::replace_dir(
                    &mut builder,
                    &src_dir,
                    &modified,
                    format!("把 {asset} 的改动写回源仓库，生成新版本 {short}"),
                )?;
                builder.promotes(promotion.clone());
                let version = library.ingest(
                    asset,
                    crate::model::AssetKind::Skill,
                    &modified,
                    &crate::library::Origin {
                        source: Some(source.clone()),
                        commit: Some(promotion.commit.clone()),
                        tag: None,
                    },
                )?;
                let mut new_locked = locked.clone();
                new_locked.commit = Some(promotion.commit.clone());
                new_locked.tag = None;
                new_locked.hash = version.hash;
                if decl.pin.is_some() {
                    let text =
                        std::fs::read_to_string(&manifest_file).map_err(|source| Error::Io {
                            path: manifest_file.clone(),
                            source,
                        })?;
                    let edited = crate::manifest::set_skill_pin(
                        crate::paths::MANIFEST_FILE,
                        &text,
                        asset,
                        &short,
                    )?;
                    project.manifest =
                        crate::manifest::parse_manifest(crate::paths::MANIFEST_FILE, &edited)?;
                    builder.write_file(
                        &manifest_file,
                        edited.into_bytes(),
                        format!("{asset} 改为固定 {short}"),
                    )?;
                    new_locked.pin = Some(short.clone());
                }
                let mut new_lock = lock.clone();
                new_lock.skills.insert(asset.to_owned(), new_locked);
                builder.write_file(
                    &lock_file,
                    new_lock.to_toml()?.into_bytes(),
                    format!("{asset} 锁到 {short}"),
                )?;
                if let Some(object) = &drifted.object {
                    match &original {
                        Some(original) => {
                            drift::replace_dir(
                                &mut builder,
                                object,
                                &original.dir,
                                format!("资产库里的旧正本恢复为 {label}"),
                            )?;
                        }
                        None => builder.note(PlanNote::warning(
                            "原版取不回，资产库里的旧正本保持改过的样子",
                        )),
                    }
                }
                self.sync_into(&mut builder, &project, &new_lock, &library, &sources, &[])?;
                let others: Vec<String> = self
                    .others_using(root, asset, Some(&locked.source))
                    .into_iter()
                    .map(|(p, _)| crate::scanner::project_name(&p))
                    .collect();
                if !others.is_empty() {
                    builder.note(PlanNote::info(format!(
                        "另有 {} 个项目也用 {asset}：{}。写回后上游检查会把它们列进「可更新」，逐个决定是否跟进",
                        others.len(),
                        others.join("、")
                    )));
                }
                builder.note(PlanNote::info(
                    "来源仓库里的提交不随回滚撤销；回滚只还原文件",
                ));
            }
        }
        builder.set_title(title);
        Ok(self.issue(builder))
    }

    /// 重新检测全部已登记项目的漂移，新出现的各记一条待处理（6.7）。返回要推给界面的事件。
    /// 文件监听回调、活动页、侧栏的待处理数都先调它。
    pub fn refresh_drift(&self) -> Result<Vec<KernelEvent>> {
        let records = self
            .journal
            .read(&crate::journal::RecordFilter::default())?;
        let mut events = Vec::new();
        for registered in crate::registry::load(&self.home)? {
            let root = registered.path;
            let Ok(found) = self.drifts(&root) else {
                continue;
            };
            for drifted in found {
                if live_pending(&records, &root, &drifted.asset) {
                    continue;
                }
                let lock = Lock::load(&root).ok().flatten();
                let info = self.drift_info(&root, &drifted, lock.as_ref());
                let record = drift_record(&root, &drifted, &info, self.watch_trigger());
                let _ = self.journal.record(record);
                events.push(KernelEvent::DriftDetected {
                    project: ProjectId(root.to_string_lossy().into_owned()),
                    asset: drifted.asset.clone(),
                });
            }
        }
        if !events.is_empty() {
            events.push(KernelEvent::JournalChanged);
        }
        Ok(events)
    }

    /// 生效上下文里受管的 skill 挂上漂移块与可更新块（6.3）。
    pub(super) fn attach_drift_update(
        &self,
        context: &mut crate::resolver::EffectiveContext,
        root: &Path,
    ) {
        let drifts = self.drifts(root).unwrap_or_default();
        let lock = Lock::load(root).ok().flatten();
        for item in context.items.iter_mut().filter(|i| {
            i.kind == crate::model::AssetKind::Skill
                && matches!(
                    i.scope,
                    crate::model::Scope::Project | crate::model::Scope::Bundle
                )
                && i.overridden_by.is_none()
        }) {
            if let Some(drifted) = drifts.iter().find(|d| d.asset == item.name) {
                let info = self.drift_info(root, drifted, lock.as_ref());
                item.state = Some(crate::model::AssetState::Drifted);
                item.label = "本地已改".to_owned();
                item.reason = format!(
                    "项目清单固定使用 {} 版本。\n但项目内这份文件已被本地修改，与锁定的版本不一致。",
                    info.locked
                );
                item.drift = Some(info);
                continue;
            }
            if let Some(update) = self.update_info(root, &item.name) {
                item.state = Some(crate::model::AssetState::Updatable);
                item.label = format!("可更新 ↑ {}", update.to);
                item.update = Some(update);
            }
        }
    }

    /// 监听器要盯的目录：资产库的正本（链接模式的漂移改在这里）和各已登记项目的
    /// `.agents/skills`（入库模式的实体）。
    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Ok(library) = Library::open(&self.home) {
            out.push(library.objects_dir());
        }
        if let Ok(registered) = crate::registry::load(&self.home) {
            for p in registered {
                out.push(p.path.join(crate::materializer::sync::SHARED_SKILLS));
            }
        }
        out
    }

    /// 文件监听发现的记作「文件监听」，命令行顺带发现的记作「命令行」。
    fn watch_trigger(&self) -> Trigger {
        match self.origin {
            Trigger::Cli => Trigger::Cli,
            _ => Trigger::Watcher,
        }
    }

    /// 读记录时，待处理的漂移、可更新按现状判断：已经处理过的显示「已记录」（6.7）。
    pub(super) fn settle_pending(&self, records: &mut [Record]) {
        let snapshot = records.to_vec();
        let upstream = super::upstream::UpstreamState::load(&self.home).unwrap_or_default();
        for record in records.iter_mut() {
            if record.status != RecordStatus::Pending {
                continue;
            }
            let Some(subject) = &record.subject else {
                continue;
            };
            let still = match record.kind {
                RecordKind::Drift => record.project.as_ref().is_some_and(|p| {
                    let root = PathBuf::from(&p.0);
                    newest_live(&snapshot, &root, &subject.asset).as_deref()
                        == Some(record.id.as_str())
                        && self
                            .drifts(&root)
                            .is_ok_and(|d| d.iter().any(|d| d.asset == subject.asset))
                }),
                RecordKind::Updatable => subject
                    .version
                    .as_deref()
                    .is_some_and(|to| !upstream.live(&subject.asset, to).is_empty()),
                _ => true,
            };
            if !still {
                record.status = RecordStatus::Recorded;
            }
        }
    }
}

/// 这个项目这一项最新的待处理漂移记录，后面没有处理它的写入记录时才算。
fn newest_live(records: &[Record], root: &Path, asset: &str) -> Option<String> {
    let wanted = crate::scanner::fsutil::canonical(root);
    let same = |r: &Record| {
        r.kind == RecordKind::Drift
            && r.subject.as_ref().is_some_and(|s| s.asset == asset)
            && r.project
                .as_ref()
                .is_some_and(|p| crate::scanner::fsutil::canonical(Path::new(&p.0)) == wanted)
    };
    // 新的在前：先碰到处理记录就说明最新的待处理已经处理过了。
    for record in records.iter().filter(|r| same(r)) {
        match record.status {
            RecordStatus::Pending => return Some(record.id.clone()),
            RecordStatus::Applied => return None,
            _ => {}
        }
    }
    None
}

fn live_pending(records: &[Record], root: &Path, asset: &str) -> bool {
    newest_live(records, root, asset).is_some()
}

fn drift_record(root: &Path, drifted: &Drifted, info: &DriftInfo, trigger: Trigger) -> Record {
    let at = Timestamp::now();
    Record {
        id: format!("drift-{}-{}-{}", at.0, std::process::id(), drifted.asset),
        at,
        kind: RecordKind::Drift,
        title: format!("{} 的本地文件被修改", drifted.asset),
        project: Some(ProjectId(root.to_string_lossy().into_owned())),
        project_label: crate::scanner::project_name(root),
        trigger,
        snapshot: None,
        snapshots: Vec::new(),
        reverts: None,
        status: RecordStatus::Pending,
        summary: format!(
            "{}请到项目页选择处理方式：写回源仓库、还原锁定版或转为项目独立文件。",
            info.summary
        ),
        files: info
            .files
            .iter()
            .map(|f| RecordFile {
                op: if f.op == FileOp::Read {
                    FileOp::Modify
                } else {
                    f.op
                },
                path: format!("{}/{}", drifted.path, f.path),
                note: Some(format!("+{} −{}", f.added, f.removed)),
            })
            .collect(),
        // 待处理的漂移附「看差异」的命令，三个动作在它的输出里（第 5 节）。
        command: Some(format!("fangcun drift {}", drifted.asset)),
        output: None,
        subject: Some(Subject {
            asset: drifted.asset.clone(),
            version: None,
        }),
        finding: None,
    }
}
