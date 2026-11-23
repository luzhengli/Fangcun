//! 更新（F6、6.3、6.7、M1-16）：上游检查、可更新的项、升级。
//!
//! - **上游检查**：已登记项目锁里有提交的 skill（GitHub、有 git 的本地来源），跟随分支的取那个分支
//!   最新的提交，否则取默认分支最新的提交；内容哈希和锁里的不同就是「可更新」。结果存在
//!   `~/.fangcun/state/upstream.toml`，新出现的版本各记一条待处理（触发方「上游检查」）。
//!   桌面应用启动时查一次，之后只在手动触发时查（9.3）。
//! - **升级**：锁换到那一版（`pin` 的项清单里的 `pin` 一起改），再同步。可以连带其他项目，
//!   每个项目各留一个快照（6.3「可更新」）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Kernel;
use crate::journal::{Record, RecordKind, RecordStatus, Subject, Trigger};
use crate::library::Library;
use crate::library::diff;
use crate::manifest::Lock;
use crate::materializer::{ChangePlan, PlanBuilder, SnapshotScope};
use crate::model::{PlanNote, ProjectId, SourceRef, Timestamp, VersionPolicy};
use crate::paths::FangcunHome;
use crate::resolver::{UpdateInfo, UpdateProject};
use crate::{Error, Result};

/// 上次上游检查的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct UpstreamState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<Timestamp>,
    #[serde(default, rename = "entry", skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<UpstreamEntry>,
}

/// 一个项目的一项 skill 上游有新版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct UpstreamEntry {
    pub project: PathBuf,
    pub asset: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    pub from_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_commit: Option<String>,
    pub to_commit: String,
    pub to_hash: String,
}

impl UpstreamState {
    fn file(home: &FangcunHome) -> PathBuf {
        home.state_dir().join("upstream.toml")
    }

    pub fn load(home: &FangcunHome) -> Result<Self> {
        let file = Self::file(home);
        match std::fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Parse {
                origin: file.display().to_string(),
                message: e.to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io { path: file, source }),
        }
    }

    fn save(&self, home: &FangcunHome) -> Result<()> {
        let file = Self::file(home);
        let text = toml::to_string(self).map_err(|e| Error::Parse {
            origin: "上游检查".to_owned(),
            message: e.to_string(),
        })?;
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        crate::materializer::write_atomic(&file, text.as_bytes(), None)
    }

    /// 还没升级的：项目的锁仍在检查时那一版。
    pub fn live(&self, asset: &str, to_commit: &str) -> Vec<&UpstreamEntry> {
        self.entries
            .iter()
            .filter(|e| e.asset == asset && e.to_commit == to_commit)
            .filter(|e| still_behind(e))
            .collect()
    }

    /// 这个项目的这一项现在能升到哪一版。
    pub fn for_project(&self, root: &Path, asset: &str) -> Option<&UpstreamEntry> {
        let wanted = crate::scanner::fsutil::canonical(root);
        self.entries.iter().find(|e| {
            e.asset == asset
                && crate::scanner::fsutil::canonical(&e.project) == wanted
                && still_behind(e)
        })
    }
}

fn still_behind(entry: &UpstreamEntry) -> bool {
    Lock::load(&entry.project)
        .ok()
        .flatten()
        .and_then(|l| {
            l.skills
                .get(&entry.asset)
                .map(|s| s.hash.0 == entry.from_hash)
        })
        .unwrap_or(false)
}

/// 一次上游检查的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpstreamReport {
    pub items: Vec<Updatable>,
    /// 没查到的来源，例如网络不通、私有仓库没有权限。
    pub errors: Vec<String>,
}

/// `fangcun update --dry-run` 和上游检查的结果里的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Updatable {
    pub asset: String,
    pub source: String,
    pub from: String,
    pub to: String,
    pub projects: Vec<String>,
    pub scripts: bool,
    pub files: Vec<diff::FileDiff>,
}

/// 上游检查去重的键：来源、仓库内目录、跟随的分支、锁里的哈希。
type CheckKey = (String, String, Option<String>, String);

fn short(commit: &str) -> String {
    crate::scanner::short_hash(commit)
}

impl Kernel {
    /// 上游检查（M1-16）。会联网拉 GitHub 来源。新出现的版本各记一条待处理。
    pub fn check_upstream(&self) -> Result<UpstreamReport> {
        let library = Library::open(&self.home)?;
        let sources = self.sources();
        let old = UpstreamState::load(&self.home).unwrap_or_default();
        let mut state = UpstreamState {
            checked: Some(Timestamp::now()),
            entries: Vec::new(),
        };
        let mut errors = BTreeSet::new();
        // 同一个来源、目录、约束、起点只查一次。
        let mut seen: BTreeMap<CheckKey, Option<(String, String)>> = BTreeMap::new();
        for registered in crate::registry::load(&self.home)? {
            let Ok(Some(lock)) = Lock::load(&registered.path) else {
                continue;
            };
            for (asset, locked) in &lock.skills {
                let Some(from_commit) = locked.commit.clone() else {
                    continue;
                };
                let key = (
                    locked.source.clone(),
                    locked.path.clone(),
                    locked.track.clone(),
                    locked.hash.0.clone(),
                );
                let found = match seen.get(&key) {
                    Some(found) => found.clone(),
                    None => {
                        let source = SourceRef(locked.source.clone());
                        let found = (|| -> Result<Option<(String, String)>> {
                            let to = sources.upstream_commit(&source, locked.track.as_deref())?;
                            if to == from_commit {
                                return Ok(None);
                            }
                            let resolved = sources.resolve_skill(
                                &library,
                                asset,
                                &source,
                                Some(&VersionPolicy::Pin(to.clone())),
                                Some(&locked.path),
                            )?;
                            Ok((resolved.version.hash != locked.hash)
                                .then_some((to, resolved.version.hash.0)))
                        })();
                        let found = found.unwrap_or_else(|e| {
                            errors.insert(format!("{asset}（{}）：{e}", locked.source));
                            None
                        });
                        seen.insert(key, found.clone());
                        found
                    }
                };
                if let Some((to_commit, to_hash)) = found {
                    state.entries.push(UpstreamEntry {
                        project: registered.path.clone(),
                        asset: asset.clone(),
                        source: locked.source.clone(),
                        track: locked.track.clone(),
                        from_hash: locked.hash.0.clone(),
                        from_commit: Some(from_commit.clone()),
                        to_commit,
                        to_hash,
                    });
                }
            }
        }
        state.save(&self.home)?;

        // 按资产和目标版本归组；新出现的记一条待处理。
        let mut groups: BTreeMap<(String, String), Vec<&UpstreamEntry>> = BTreeMap::new();
        for entry in &state.entries {
            groups
                .entry((entry.asset.clone(), entry.to_commit.clone()))
                .or_default()
                .push(entry);
        }
        let mut out = Vec::new();
        for ((asset, to), entries) in groups {
            let first = entries[0];
            let files = library
                .locate(&crate::model::ContentHash(first.from_hash.clone()))
                .ok()
                .flatten()
                .zip(
                    library
                        .locate(&crate::model::ContentHash(first.to_hash.clone()))
                        .ok()
                        .flatten(),
                )
                .map(|(a, b)| diff::diff_dirs(Some(&a), &b))
                .unwrap_or_default();
            let scripts = files.iter().any(|f| f.script);
            let projects: Vec<String> = entries
                .iter()
                .map(|e| crate::scanner::project_name(&e.project))
                .collect();
            let from = first.from_commit.as_deref().map(short).unwrap_or_default();
            let known = old
                .entries
                .iter()
                .any(|e| e.asset == asset && e.to_commit == to);
            if !known {
                let _ = self
                    .journal
                    .record(updatable_record(&asset, &from, &to, &entries, scripts));
            }
            out.push(Updatable {
                asset,
                source: first.source.clone(),
                from,
                to: short(&to),
                projects,
                scripts,
                files,
            });
        }
        Ok(UpstreamReport {
            items: out,
            errors: errors.into_iter().collect(),
        })
    }

    /// 检查器可更新块的数据：这个项目的这一项能升到哪一版、差异、别的项目。
    pub(super) fn update_info(&self, root: &Path, asset: &str) -> Option<UpdateInfo> {
        let state = UpstreamState::load(&self.home).ok()?;
        let entry = state.for_project(root, asset)?;
        let library = Library::open(&self.home).ok()?;
        let files = match (
            library
                .locate(&crate::model::ContentHash(entry.from_hash.clone()))
                .ok()
                .flatten(),
            library
                .locate(&crate::model::ContentHash(entry.to_hash.clone()))
                .ok()
                .flatten(),
        ) {
            (Some(a), Some(b)) => diff::diff_dirs(Some(&a), &b),
            _ => Vec::new(),
        };
        let wanted = crate::scanner::fsutil::canonical(root);
        let others = state
            .live(asset, &entry.to_commit)
            .into_iter()
            .filter(|e| crate::scanner::fsutil::canonical(&e.project) != wanted)
            .filter(|e| e.from_hash == entry.from_hash)
            .map(|e| {
                let mode = crate::manifest::project_basics(&e.project)
                    .ok()
                    .flatten()
                    .and_then(|b| b.mode)
                    .unwrap_or(crate::model::SyncMode::Link);
                let policy = match &e.track {
                    Some(t) => format!("跟随 {t}"),
                    None => "固定版本".to_owned(),
                };
                let mode = match mode {
                    crate::model::SyncMode::Link => "本机软链",
                    crate::model::SyncMode::Vendor => "拷入仓库",
                };
                UpdateProject {
                    project: ProjectId(e.project.to_string_lossy().into_owned()),
                    name: crate::scanner::project_name(&e.project),
                    note: format!("{policy} · {mode}"),
                }
            })
            .collect();
        Some(UpdateInfo {
            from: entry.from_commit.as_deref().map(short).unwrap_or_default(),
            to: short(&entry.to_commit),
            scripts: files.iter().any(|f| f.script),
            files,
            others,
        })
    }

    /// 改清单文本和锁：把这几项换到上游检查查到的那一版。返回计划里的写法，例如「tdd a1b2c3 → b7e9f0」。
    pub(crate) fn apply_upgrades(
        &self,
        builder: &mut PlanBuilder,
        root: &Path,
        project: &crate::manifest::Project,
        text: &mut String,
        lock: &mut Lock,
        assets: &[String],
    ) -> Result<Vec<String>> {
        let state = UpstreamState::load(&self.home)?;
        let mut rows = Vec::new();
        for asset in assets {
            let Some(entry) = state.for_project(root, asset) else {
                builder.note(PlanNote::info(format!(
                    "{asset} 现在没有可升级的版本（已经升过，或要先做上游检查）"
                )));
                continue;
            };
            let Some(locked) = lock.skills.get_mut(asset) else {
                continue;
            };
            let to = short(&entry.to_commit);
            if locked.pin.is_some() {
                if !project.manifest.skills.contains_key(asset) {
                    builder.note(PlanNote::warning(format!(
                        "{asset} 固定在套装里的版本，升级要改套装，这次没有升"
                    )));
                    continue;
                }
                *text =
                    crate::manifest::set_skill_pin(crate::paths::MANIFEST_FILE, text, asset, &to)?;
                locked.pin = Some(to.clone());
            }
            rows.push(format!(
                "{asset} {} → {to}",
                entry.from_commit.as_deref().map(short).unwrap_or_default()
            ));
            locked.commit = Some(entry.to_commit.clone());
            locked.tag = None;
            locked.hash = crate::model::ContentHash(entry.to_hash.clone());
        }
        Ok(rows)
    }

    /// 把一个项目的几项升到上游检查查到的那一版，填进计划（清单的 pin、锁、同步）。
    /// 没有可升的项返回 `false`。
    pub(crate) fn upgrade_into(
        &self,
        builder: &mut PlanBuilder,
        root: &Path,
        assets: &[String],
        library: &Library,
        sources: &crate::library::source::Sources,
    ) -> Result<bool> {
        let (mut project, mut lock) = self.manifest_and_lock(root)?;
        let manifest_file = root.join(crate::paths::MANIFEST_FILE);
        let original = std::fs::read_to_string(&manifest_file).map_err(|source| Error::Io {
            path: manifest_file.clone(),
            source,
        })?;
        let mut text = original.clone();
        let rows = self.apply_upgrades(builder, root, &project, &mut text, &mut lock, assets)?;
        if rows.is_empty() {
            return Ok(false);
        }
        if text != original {
            project.manifest = crate::manifest::parse_manifest(crate::paths::MANIFEST_FILE, &text)?;
            builder.write_file(
                &manifest_file,
                text.into_bytes(),
                format!("pin 改到 {}", rows.join("、")),
            )?;
        }
        builder.write_file(
            &root.join(crate::paths::LOCK_FILE),
            lock.to_toml()?.into_bytes(),
            format!("升级 {}", rows.join("、")),
        )?;
        self.sync_into(builder, &project, &lock, library, sources, &[])?;
        Ok(true)
    }

    /// `fangcun update [资产] --projects …`：把这些项目的可更新项升级（`asset` 为空表示全部），
    /// 一份计划，每个项目各留一个快照。`projects` 为空时是命令行的当前项目。
    pub fn plan_update(&self, asset: Option<&str>, projects: &[ProjectId]) -> Result<ChangePlan> {
        self.require_write("update")?;
        let roots: Vec<PathBuf> = if projects.is_empty() {
            self.extra_projects.first().cloned().into_iter().collect()
        } else {
            projects.iter().map(|p| PathBuf::from(&p.0)).collect()
        };
        let first = roots
            .first()
            .cloned()
            .ok_or_else(|| Error::NotFound("要升级的项目".to_owned()))?;
        let state = UpstreamState::load(&self.home)?;
        let library = Library::open(&self.home)?;
        let sources = self.sources();
        let mut builder = PlanBuilder::new(
            String::new(),
            RecordKind::Upgrade,
            SnapshotScope::Project(first),
        );
        let mut assets_all = BTreeSet::new();
        let mut any = false;
        for root in &roots {
            let assets: Vec<String> = match asset {
                Some(a) => vec![a.to_owned()],
                None => state
                    .entries
                    .iter()
                    .filter(|e| {
                        crate::scanner::fsutil::canonical(&e.project)
                            == crate::scanner::fsutil::canonical(root)
                    })
                    .filter(|e| still_behind(e))
                    .map(|e| e.asset.clone())
                    .collect(),
            };
            if self.upgrade_into(&mut builder, root, &assets, &library, &sources)? {
                any = true;
                assets_all.extend(assets);
            }
        }
        let names: Vec<String> = assets_all.into_iter().collect();
        if let [one] = names.as_slice() {
            builder.subject(one.clone(), None);
        }
        let mut command = String::from("fangcun update");
        if let Some(a) = asset {
            command.push(' ');
            command.push_str(a);
        }
        let title = match (any, roots.len()) {
            (false, _) => "没有可升级的项。先运行上游检查（fangcun update --dry-run）".to_owned(),
            (true, 1) => format!("升级 {}", names.join("、")),
            (true, n) => format!("升级 {}（{n} 个项目）", names.join("、")),
        };
        builder.set_title(title);
        let builder = builder.command(command);
        Ok(self.issue(builder))
    }
}

fn updatable_record(
    asset: &str,
    from: &str,
    to: &str,
    entries: &[&UpstreamEntry],
    scripts: bool,
) -> Record {
    let at = Timestamp::now();
    let names: Vec<String> = entries
        .iter()
        .map(|e| crate::scanner::project_name(&e.project))
        .collect();
    let label = match names.as_slice() {
        [one] => one.clone(),
        [a, b] => format!("{a}、{b}"),
        [first, ..] => format!("{first} 等 {} 个项目", names.len()),
        [] => String::new(),
    };
    let short_to = short(to);
    Record {
        id: format!("updatable-{}-{asset}-{short_to}", at.0),
        at,
        kind: RecordKind::Updatable,
        title: format!("{asset} 上游有新提交 {short_to}"),
        project: match entries {
            [one] => Some(ProjectId(one.project.to_string_lossy().into_owned())),
            _ => None,
        },
        project_label: label,
        trigger: Trigger::UpstreamCheck,
        snapshot: None,
        snapshots: Vec::new(),
        reverts: None,
        status: RecordStatus::Pending,
        summary: format!(
            "{} → {short_to}{}。到项目页看差异，审阅后加入同步计划。",
            from,
            if scripts { "，含脚本变更" } else { "" }
        ),
        files: Vec::new(),
        command: Some(format!("fangcun update {asset} --dry-run")),
        output: None,
        subject: Some(Subject {
            asset: asset.to_owned(),
            version: Some(to.to_owned()),
        }),
        finding: None,
    }
}
