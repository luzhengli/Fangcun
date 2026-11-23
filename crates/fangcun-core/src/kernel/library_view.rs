//! 资产库页（6.4）：每项资产一行，版本轨道上每一版固定在哪些项目，来源未知的等着收编。只读。
//!
//! - 资产来自资产库索引；「谁固定在哪一版」从各个已登记项目的锁算（M1-07）。
//! - 漂移来自各项目的台账（与项目页同一套检测），可更新来自上次上游检查（M1-16）。
//! - 来源未知的：全局层与已登记项目里扫描到、没有来源、正本也不在库里的 skill，按名字合成一行，
//!   决定卡片给「收编」（`plan_adopt`）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Kernel;
use super::item_state::recent_named;
use crate::global::KindCounts;
use crate::journal::RecordFilter;
use crate::library::Library;
use crate::manifest::Lock;
use crate::model::{AssetKind, Scope, Timestamp, TokenEstimate};
use crate::resolver::ItemActivity;
use crate::scanner::fsutil::{display_path, frontmatter_value};
use crate::scanner::{SkillCopy, short_hash};
use crate::{Result, estimator};

/// 资产库页的来源分组：来源下拉框按它分组（6.4「筛选」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SourceGroup {
    /// 本地开发：`local:<名字>`。
    Local,
    /// GitHub：`github:<owner>/<repo>` 或 `<owner>/<repo>`。
    Github,
    /// npm：启动命令是 `npx` 的 MCP。
    Npm,
    /// 待收编：来源未知。
    Unknown,
    /// 其他。
    Other,
}

/// 资产库页一行的状态（6.4「状态」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum LibraryStatus {
    /// N 处漂移。橙。
    Drift,
    /// 可更新。蓝。
    Updatable,
    /// 来源未知，还没收编。紫。
    UnknownSource,
    /// 已同步。绿。
    Synced,
    /// 已收编、来源未知：只换了链接，没进清单（F1）。绿。
    Adopted,
    /// 在库里，但没有已登记项目固定它。灰。
    Unused,
}

/// 版本轨道上一个节点的标记（6.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum VersionTag {
    /// 有项目的锁固定在这一版。绿。
    Locked,
    /// 来源上最新的一版，还没有项目固定。蓝。
    Latest,
    /// 分叉：某个项目里改出来、另存的一版（F4）。橙。内核还不出这个标记：分叉后方寸不再管
    /// 那一份，要等资产库记下分叉的来处再画。
    Fork,
    /// 旧版：库里有，没有项目固定，也不是最新。灰。
    Old,
    /// 来源未知、还没收编的那一份。紫。
    Unadopted,
}

/// 固定在某一版的一个使用方。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VersionUser {
    /// 项目名；全局层写「全局层」。
    pub name: String,
    /// 项目根目录。全局层为空。
    pub path: Option<String>,
    /// 物化方式：链接、入库、配置、全局层。
    pub mode: String,
    /// 这个项目里的副本被改过。
    pub drift: bool,
}

/// 版本轨道上的一个节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryVersion {
    /// 显示的版本号：标签、短提交，都没有时短哈希。
    pub label: String,
    pub hash: String,
    /// 版本号旁边的小字，例如存进来的日期、「上游新提交」「本地内容哈希」。
    pub note: String,
    pub tag: VersionTag,
    pub users: Vec<VersionUser>,
}

/// 资产库页的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryRow {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    /// 名称下面一行小字：来源写法；来源未知时是扫描到的位置；MCP 是启动命令。
    pub source: String,
    pub group: SourceGroup,
    pub status: LibraryStatus,
    /// 漂移的项目数。
    pub drift: u32,
    /// 固定了这项资产的已登记项目数；在全局层的算全部。
    pub used_by: u32,
    /// 旧到新。
    pub versions: Vec<LibraryVersion>,
    pub resident: Option<TokenEstimate>,
    pub has_scripts: bool,
    /// 正本的位置。还没收编时为空。
    pub store: Option<String>,
    /// 检查器名称下面的一句说明。
    pub summary: String,
    pub recent: Vec<ItemActivity>,
    /// 还没收编，可以「收编」（交给 `plan_adopt` 的名字就是 `name`）。
    pub adoptable: bool,
}

/// 来源下拉框的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceCount {
    /// 来源写法，例如 `github:obra/superpowers`、`npm`、「来源未知」。
    pub source: String,
    pub group: SourceGroup,
    pub count: u32,
}

/// 资产库页（6.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibraryOverview {
    /// 已登记的项目数，「谁在用」比例条的分母。
    pub project_total: u32,
    pub kinds: KindCounts,
    pub sources: Vec<SourceCount>,
    /// 有两个以上版本被项目固定的资产数。
    pub multi_version: u32,
    pub drift: u32,
    pub updatable: u32,
    pub unknown: u32,
    pub rows: Vec<LibraryRow>,
    /// 已登记、但没有一个列在这里的项目名，「没用到：…」用。
    pub projects: Vec<String>,
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub(super) fn group_of(source: &str) -> SourceGroup {
    if source.starts_with("local:") {
        SourceGroup::Local
    } else if source.starts_with("github:")
        || (source.split('/').count() == 2 && !source.contains(':') && !source.contains(' '))
    {
        SourceGroup::Github
    } else if source.starts_with("npx ") || source == "npm" {
        SourceGroup::Npm
    } else {
        SourceGroup::Other
    }
}

/// 存进来的日期：`MM-DD`。
fn day(at: &Timestamp) -> String {
    at.0.get(5..10).unwrap_or_default().to_owned()
}

/// 一个已登记项目里某项资产的使用情况。
struct Pin {
    project: String,
    path: PathBuf,
    hash: String,
    commit: Option<String>,
    tag: Option<String>,
    mode: String,
    drift: bool,
}

impl Kernel {
    /// 资产库页（6.4）。只读。
    pub fn library_overview(&self) -> Result<LibraryOverview> {
        let scan = self.scan();
        let library = Library::open(&self.home)?;
        let entries = library.entries()?;
        let dates = library.version_dates().unwrap_or_default();
        let registered = crate::registry::load(&self.home)?;
        let records = self.journal.read(&RecordFilter::default())?;
        let upstream = super::upstream::UpstreamState::load(&self.home).unwrap_or_default();
        let home = scan.home.clone();

        // 各已登记项目的锁：资产（种类、名字）→ 固定的那一版。
        let mut pins: BTreeMap<(AssetKind, String), Vec<Pin>> = BTreeMap::new();
        let mut project_names = Vec::new();
        for project in &registered {
            let root = &project.path;
            let name = crate::scanner::project_name(root);
            project_names.push(name.clone());
            let Ok(Some(lock)) = Lock::load(root) else {
                continue;
            };
            let drifted: BTreeSet<String> = self
                .drifts(root)
                .unwrap_or_default()
                .into_iter()
                .map(|d| d.asset)
                .collect();
            let wanted = crate::scanner::fsutil::canonical(root);
            let items = scan
                .project_items
                .iter()
                .find(|(r, _)| crate::scanner::fsutil::canonical(r) == wanted)
                .map(|(_, items)| items);
            let linked = |skill: &str| {
                items.is_some_and(|by_tool| {
                    by_tool.values().any(|c| {
                        c.skills
                            .iter()
                            .any(|s| s.name == skill && s.link_target.is_some())
                    })
                })
            };
            for (skill, locked) in &lock.skills {
                pins.entry((AssetKind::Skill, skill.clone()))
                    .or_default()
                    .push(Pin {
                        project: name.clone(),
                        path: root.clone(),
                        hash: locked.hash.0.clone(),
                        commit: locked.commit.clone(),
                        tag: locked.tag.clone(),
                        mode: if locked.bundle.is_some() {
                            "套装".to_owned()
                        } else if linked(skill) {
                            "本机软链".to_owned()
                        } else {
                            "拷入仓库".to_owned()
                        },
                        drift: drifted.contains(skill),
                    });
            }
            for (server, locked) in &lock.mcp {
                pins.entry((AssetKind::Mcp, server.clone()))
                    .or_default()
                    .push(Pin {
                        project: name.clone(),
                        path: root.clone(),
                        hash: locked.hash.0.clone(),
                        commit: None,
                        tag: None,
                        mode: "配置".to_owned(),
                        drift: false,
                    });
            }
            for (file, locked) in &lock.instructions {
                pins.entry((AssetKind::Instruction, file.clone()))
                    .or_default()
                    .push(Pin {
                        project: name.clone(),
                        path: root.clone(),
                        hash: locked.hash.0.clone(),
                        commit: None,
                        tag: None,
                        mode: "正本".to_owned(),
                        drift: drifted.contains(file),
                    });
            }
        }
        let total = u32_of(registered.len());

        // 全局层里链到正本的 skill：哈希 → 哪些工具。
        let mut global_skills: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (tool, collected) in &scan.global {
            for s in collected.skills.iter().filter(|s| s.scope == Scope::Global) {
                global_skills
                    .entry(s.hash.0.clone())
                    .or_default()
                    .insert(scan.tool_name(tool).to_owned());
            }
        }

        let mut rows = Vec::new();
        let mut library_hashes = BTreeSet::new();
        for entry in &entries {
            for v in &entry.versions {
                library_hashes.insert(v.hash.0.clone());
            }
            let key = (entry.kind, entry.name.clone());
            let empty = Vec::new();
            let pinned = pins.get(&key).unwrap_or(&empty);
            let mut versions: Vec<LibraryVersion> = Vec::new();
            for v in &entry.versions {
                let mut users: Vec<VersionUser> = pinned
                    .iter()
                    .filter(|p| p.hash == v.hash.0)
                    .map(|p| VersionUser {
                        name: p.project.clone(),
                        path: Some(display_path(&p.path, &home)),
                        mode: p.mode.clone(),
                        drift: p.drift,
                    })
                    .collect();
                if let Some(tools) = global_skills.get(&v.hash.0) {
                    users.push(VersionUser {
                        name: "全局层".to_owned(),
                        path: None,
                        mode: tools.iter().cloned().collect::<Vec<_>>().join("、"),
                        drift: false,
                    });
                }
                let label = v
                    .tag
                    .clone()
                    .or_else(|| v.commit.as_deref().map(short_hash))
                    .unwrap_or_else(|| short_hash(&v.hash.0));
                versions.push(LibraryVersion {
                    label,
                    hash: v.hash.0.clone(),
                    note: dates.get(&v.hash.0).map(day).unwrap_or_default(),
                    tag: if users.is_empty() {
                        VersionTag::Old
                    } else {
                        VersionTag::Locked
                    },
                    users,
                });
            }
            // 锁里固定的那一版不在库里（例如别的机器同步来的锁）：也画出来。
            for p in pinned {
                if versions.iter().any(|v| v.hash == p.hash) {
                    continue;
                }
                let label = p
                    .tag
                    .clone()
                    .or_else(|| p.commit.as_deref().map(short_hash))
                    .unwrap_or_else(|| short_hash(&p.hash));
                versions.push(LibraryVersion {
                    label,
                    hash: p.hash.clone(),
                    note: "不在本机资产库".to_owned(),
                    tag: VersionTag::Locked,
                    users: pinned
                        .iter()
                        .filter(|q| q.hash == p.hash)
                        .map(|q| VersionUser {
                            name: q.project.clone(),
                            path: Some(display_path(&q.path, &home)),
                            mode: q.mode.clone(),
                            drift: q.drift,
                        })
                        .collect(),
                });
            }
            // 上游有新提交：最新的一版排在最后。
            let mut updatable = false;
            if entry.kind == AssetKind::Skill {
                let newer = pinned
                    .iter()
                    .find_map(|p| upstream.for_project(&p.path, &entry.name));
                if let Some(e) = newer {
                    updatable = true;
                    if let Some(v) = versions.iter_mut().find(|v| v.hash == e.to_hash) {
                        if v.users.is_empty() {
                            v.tag = VersionTag::Latest;
                        }
                    } else {
                        versions.push(LibraryVersion {
                            label: short_hash(&e.to_commit),
                            hash: e.to_hash.clone(),
                            note: "上游新提交".to_owned(),
                            tag: VersionTag::Latest,
                            users: Vec::new(),
                        });
                    }
                }
            }

            let users: BTreeSet<&str> = versions
                .iter()
                .flat_map(|v| v.users.iter())
                .map(|u| u.name.as_str())
                .collect();
            let used_by = if users.contains("全局层") {
                total
            } else {
                u32_of(users.len())
            };
            let drift = u32_of(pinned.iter().filter(|p| p.drift).count());
            let latest = entry.versions.last();
            let object = latest.and_then(|v| library.locate(&v.hash).ok().flatten());
            let (source, group) = match (&entry.source, entry.kind) {
                (Some(s), _) => (s.0.clone(), group_of(&s.0)),
                (None, AssetKind::Mcp) => {
                    let command = latest
                        .and_then(|v| library.read_mcp(&v.hash).ok().flatten())
                        .map(|spec| {
                            std::iter::once(spec.command)
                                .chain(spec.args)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_default();
                    let group = if command.starts_with("npx ") {
                        SourceGroup::Npm
                    } else {
                        SourceGroup::Other
                    };
                    (command, group)
                }
                (None, _) => ("未登记来源 · 已收录".to_owned(), SourceGroup::Local),
            };
            let (description, resident) = match (entry.kind, &object) {
                (AssetKind::Skill, Some(dir)) => {
                    let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
                    let description = frontmatter_value(&text, "description");
                    let resident = estimator::estimate_text(&estimator::skill_listing(
                        &entry.name,
                        description.as_deref(),
                    ));
                    (description, Some(resident))
                }
                (AssetKind::Instruction, Some(file)) => {
                    let text = std::fs::read_to_string(file).unwrap_or_default();
                    (None, Some(estimator::estimate_text(&text)))
                }
                _ => (None, None),
            };
            let status = if drift > 0 {
                LibraryStatus::Drift
            } else if updatable {
                LibraryStatus::Updatable
            } else if used_by == 0 {
                LibraryStatus::Unused
            } else if entry.source.is_none() && entry.kind == AssetKind::Skill {
                LibraryStatus::Adopted
            } else {
                LibraryStatus::Synced
            };
            let summary = description.unwrap_or_else(|| match status {
                LibraryStatus::Adopted => {
                    "已收录，原位置自动换为受管软链。补上来源后即可写入项目清单。".to_owned()
                }
                LibraryStatus::Unused => "在资产库里，没有已登记的项目固定它。".to_owned(),
                _ => format!(
                    "{} 个版本存在资产库里，各项目固定自己要的那版。",
                    entry.versions.len()
                ),
            });
            rows.push(LibraryRow {
                id: format!("{}:{}:{source}", kind_key(entry.kind), entry.name),
                name: entry.name.clone(),
                kind: entry.kind,
                source,
                group,
                status,
                drift,
                used_by,
                versions,
                resident,
                has_scripts: entry.has_scripts,
                store: object.map(|p| display_path(&p, &home)),
                summary,
                recent: recent_named(std::slice::from_ref(&entry.name), &records),
                adoptable: false,
            });
        }

        // 来源未知、还没收编的 skill：全局层和已登记项目里扫描到的，按名字合成一行。
        let wanted_roots: BTreeSet<PathBuf> = registered
            .iter()
            .map(|p| crate::scanner::fsutil::canonical(&p.path))
            .collect();
        let mut unknown: BTreeMap<String, Vec<&SkillCopy>> = BTreeMap::new();
        let in_library_name: BTreeSet<&str> = entries
            .iter()
            .filter(|e| e.kind == AssetKind::Skill)
            .map(|e| e.name.as_str())
            .collect();
        let project_copies = scan
            .project_items
            .iter()
            .filter(|(root, _)| wanted_roots.contains(&crate::scanner::fsutil::canonical(root)))
            .flat_map(|(_, by_tool)| by_tool.values())
            .flat_map(|c| c.skills.iter());
        let global_copies = scan
            .global
            .values()
            .flat_map(|c| c.skills.iter())
            .filter(|s| s.scope == Scope::Global);
        for copy in global_copies.chain(project_copies) {
            if copy.source.is_some()
                || copy.disabled_by.is_some()
                || library_hashes.contains(&copy.hash.0)
                || in_library_name.contains(copy.name.as_str())
                || pins.contains_key(&(AssetKind::Skill, copy.name.clone()))
            {
                continue;
            }
            unknown.entry(copy.name.clone()).or_default().push(copy);
        }
        for (name, copies) in unknown {
            let mut seen = BTreeSet::new();
            let copies: Vec<&SkillCopy> = copies
                .into_iter()
                .filter(|c| seen.insert(c.real_dir.clone()))
                .collect();
            let first = copies[0];
            let in_global = copies.iter().any(|c| c.scope == Scope::Global);
            let mut by_hash: BTreeMap<String, Vec<&SkillCopy>> = BTreeMap::new();
            for c in &copies {
                by_hash.entry(c.hash.0.clone()).or_default().push(c);
            }
            let versions = by_hash
                .into_iter()
                .map(|(hash, cs)| LibraryVersion {
                    label: short_hash(&hash),
                    hash,
                    note: "本地内容哈希".to_owned(),
                    tag: VersionTag::Unadopted,
                    users: cs
                        .iter()
                        .map(|c| match &c.project {
                            Some(root) => VersionUser {
                                name: crate::scanner::project_name(root),
                                path: Some(display_path(root, &home)),
                                mode: scan.tool_name(&c.tool).to_owned(),
                                drift: false,
                            },
                            None => VersionUser {
                                name: format!("全部 {total} 个项目"),
                                path: None,
                                mode: format!("{} 全局层", scan.tool_name(&c.tool)),
                                drift: false,
                            },
                        })
                        .collect(),
                })
                .collect();
            let projects: BTreeSet<&Path> =
                copies.iter().filter_map(|c| c.project.as_deref()).collect();
            rows.push(LibraryRow {
                id: format!("unknown:{name}"),
                name: name.clone(),
                kind: AssetKind::Skill,
                source: display_path(&first.dir, &home),
                group: SourceGroup::Unknown,
                status: LibraryStatus::UnknownSource,
                drift: 0,
                used_by: if in_global {
                    total
                } else {
                    u32_of(projects.len())
                },
                versions,
                resident: Some(first.resident),
                has_scripts: copies.iter().any(|c| c.has_scripts),
                store: None,
                summary: if in_global {
                    "全局层发现，尚未收录进资产库。收录后即可统一追踪版本与更新，原位置自动换为软链。".to_owned()
                } else {
                    "项目里发现，尚未收录进资产库。收录后即可统一追踪版本与更新，原位置自动换为软链。".to_owned()
                },
                recent: recent_named(std::slice::from_ref(&name), &records),
                adoptable: true,
            });
        }

        let mut kinds = KindCounts::default();
        let mut sources: BTreeMap<(SourceGroup, String), u32> = BTreeMap::new();
        for row in &rows {
            match row.kind {
                AssetKind::Skill => kinds.skill += 1,
                AssetKind::Mcp => kinds.mcp += 1,
                AssetKind::Instruction => kinds.instruction += 1,
            }
            let label = match row.group {
                SourceGroup::Unknown => "未收录".to_owned(),
                SourceGroup::Npm => "npm".to_owned(),
                _ => repo_of(&row.source),
            };
            *sources.entry((row.group, label)).or_default() += 1;
        }
        let multi_version = u32_of(
            rows.iter()
                .filter(|r| r.versions.iter().filter(|v| !v.users.is_empty()).count() > 1)
                .count(),
        );
        Ok(LibraryOverview {
            project_total: total,
            kinds,
            sources: sources
                .into_iter()
                .map(|((group, source), count)| SourceCount {
                    source,
                    group,
                    count,
                })
                .collect(),
            multi_version,
            drift: u32_of(
                rows.iter()
                    .filter(|r| r.status == LibraryStatus::Drift)
                    .count(),
            ),
            updatable: u32_of(
                rows.iter()
                    .filter(|r| r.status == LibraryStatus::Updatable)
                    .count(),
            ),
            unknown: u32_of(
                rows.iter()
                    .filter(|r| r.status == LibraryStatus::UnknownSource)
                    .count(),
            ),
            rows,
            projects: project_names,
        })
    }
}

fn kind_key(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Skill => "skill",
        AssetKind::Mcp => "mcp",
        AssetKind::Instruction => "instruction",
    }
}

/// 来源下拉框里的写法：同一个仓库里的不同目录算一个来源，例如 `local:luyao-skills/instructions`
/// 归到 `local:luyao-skills`。
fn repo_of(source: &str) -> String {
    if let Some(rest) = source.strip_prefix("local:") {
        return format!("local:{}", rest.split('/').next().unwrap_or(rest));
    }
    source.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_sources() {
        assert_eq!(group_of("local:luyao-skills"), SourceGroup::Local);
        assert_eq!(group_of("github:obra/superpowers"), SourceGroup::Github);
        assert_eq!(group_of("obra/superpowers"), SourceGroup::Github);
        assert_eq!(group_of("npx @upstash/context7-mcp"), SourceGroup::Npm);
        assert_eq!(
            repo_of("local:luyao-skills/instructions"),
            "local:luyao-skills"
        );
    }
}
