//! 体检报告（6.2）：从一次扫描里归并出发现。
//!
//! 每条发现的字段对应原型 Checkup 画板的一条：类别、标题、一句说明、涉及的位置、
//! 动作按钮、按钮下的小字。M0 只读，动作按钮的事在 M1 做。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::fsutil::display_path;
use super::{McpEntry, Scan, SkillCopy};
use crate::model::{AssetKind, Scope, Timestamp, TokenEstimate};

/// 发现的类别。取自 6.2 线框图的「类别」列。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FindingCategory {
    /// 重复：同一份内容有多份副本。
    Duplicate,
    /// 全局税：全局层的资产进了每个项目。
    GlobalTax,
    /// 不一致：全局资产在各工具间分发不齐。
    Inconsistent,
    /// 脱节：锁文件与目录不一致。
    Detached,
    /// 密钥：已提交文件里有明文密钥。
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum FindingStatus {
    /// 待处理。
    Pending,
    /// 已处理，已留快照。
    Resolved,
    /// 尚未扫描。
    NotScanned,
}

/// 涉及的位置里一行的语气，决定第三列的颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum LocationTone {
    Warn,
    Info,
    Muted,
    Ok,
}

/// 检查器「涉及的位置」里的一行：位置、细节（哈希、数量）、说明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FindingLocation {
    pub path: String,
    pub detail: String,
    pub note: String,
    pub tone: Option<LocationTone>,
}

/// 一条体检发现。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    pub id: String,
    pub category: FindingCategory,
    pub title: String,
    pub status: FindingStatus,
    /// 检查器里的一句说明。
    pub intro: String,
    pub locations: Vec<FindingLocation>,
    /// 动作按钮的文字。
    pub action: String,
    /// 按钮下的小字。
    pub foot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckupReport {
    pub scanned_at: Timestamp,
    pub tools_scanned: u32,
    pub project_dirs_scanned: u32,
    /// 读了多少份会话记录（近 30 天）。
    pub sessions_read: u32,
    /// 发现多少项资产：skill 按实际目录去重，加上 MCP 服务和指令文件。
    pub assets_found: u32,
    /// 多出来的副本数：每个有副本的 skill，副本数减一，加起来。
    pub duplicate_copies: u32,
    /// 全局层里找不到来源的 skill 数。
    pub unknown_source: u32,
    pub findings: Vec<Finding>,
}

/// 一个 skill 在全局层，近 30 天被几个项目用到时算「很少用到」。与全局页一致。
const RARELY_USED_MAX: usize = 2;
/// 同一来源至少有这么多项时合并成一组。
pub const GROUP_MIN: usize = 3;

/// 从一次扫描归并出体检发现。
pub fn diagnose(scan: &Scan) -> CheckupReport {
    let mut findings = Vec::new();
    let mut duplicate_copies = 0;

    duplicates(scan, &mut findings, &mut duplicate_copies);
    repos(scan, &mut findings);
    global_tax(scan, &mut findings);
    inconsistent(scan, &mut findings);
    detached(scan, &mut findings);
    secrets(scan, &mut findings);

    let mut skill_dirs: BTreeSet<&PathBuf> = BTreeSet::new();
    let mut others = 0u32;
    let mut unknown: BTreeSet<&PathBuf> = BTreeSet::new();
    for collected in scan.global.values() {
        for skill in &collected.skills {
            skill_dirs.insert(&skill.real_dir);
            if skill.source.is_none() {
                unknown.insert(&skill.real_dir);
            }
        }
        others += count_u32(collected.mcp.len() + collected.instructions.len());
    }
    for per_tool in scan.project_items.values() {
        let mut files: BTreeSet<&PathBuf> = BTreeSet::new();
        for collected in per_tool.values() {
            for skill in &collected.skills {
                skill_dirs.insert(&skill.real_dir);
            }
            for file in &collected.instructions {
                files.insert(&file.real_path);
            }
            others += count_u32(collected.mcp.len());
        }
        others += count_u32(files.len());
    }

    CheckupReport {
        scanned_at: scan.scanned_at.clone(),
        tools_scanned: count_u32(scan.scanned_tools.len()),
        project_dirs_scanned: count_u32(scan.projects.len()),
        sessions_read: scan.usage.sessions_read,
        assets_found: count_u32(skill_dirs.len()) + others,
        duplicate_copies,
        unknown_source: count_u32(unknown.len()),
        findings,
    }
}

fn count_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

pub fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

/// 位置的写法：项目里的写成「项目名/相对路径」，其余用 `~`。
fn location_label(path: &Path, project: Option<&Path>, home: &Path) -> String {
    if let Some(root) = project
        && let Ok(rest) = path.strip_prefix(root)
    {
        let name = super::project_name(root);
        return format!("{name}/{}", rest.display());
    }
    display_path(path, home)
}

/// 全部 skill 副本：全局层、项目层、来源仓库。同一个实际目录只算一份。
fn all_skill_copies(scan: &Scan) -> Vec<&SkillCopy> {
    let mut seen: BTreeSet<&PathBuf> = BTreeSet::new();
    let mut out = Vec::new();
    let global = scan.global.values().flat_map(|c| c.skills.iter());
    let project = scan
        .project_items
        .values()
        .flat_map(|per_tool| per_tool.values())
        .flat_map(|c| c.skills.iter());
    for copy in global.chain(project).chain(scan.repo_skills.iter()) {
        if seen.insert(&copy.real_dir) {
            out.push(copy);
        }
    }
    out
}

fn duplicates(scan: &Scan, findings: &mut Vec<Finding>, extra: &mut u32) {
    let mut by_name: BTreeMap<&str, Vec<&SkillCopy>> = BTreeMap::new();
    for copy in all_skill_copies(scan) {
        by_name.entry(copy.name.as_str()).or_default().push(copy);
    }
    let mut groups: Vec<(&str, Vec<&SkillCopy>)> = by_name
        .into_iter()
        .filter(|(_, copies)| copies.len() > 1)
        .collect();
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));

    for (name, copies) in groups {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for copy in &copies {
            *counts.entry(copy.hash.0.as_str()).or_default() += 1;
        }
        let majority = counts
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            .map(|(hash, _)| *hash)
            .unwrap_or_default();
        let same = counts.get(majority).copied().unwrap_or(0);
        let forked = copies.len() - same;
        *extra += count_u32(copies.len() - 1);

        let title = if forked > 0 {
            format!("{name} 有 {} 份副本，{forked} 份已分叉", copies.len())
        } else {
            format!("{name} 有 {} 份副本", copies.len())
        };
        let intro = if forked > 0 {
            format!(
                "按内容哈希归并。{same} 份相同的副本收为同一版本，分叉的那份另存为独立版本，等你决定去留。"
            )
        } else {
            format!(
                "按内容哈希归并。{} 份副本内容相同，收为同一版本。",
                copies.len()
            )
        };
        let locations = copies
            .iter()
            .map(|copy| {
                let is_same = copy.hash.0 == majority;
                FindingLocation {
                    path: location_label(&copy.dir, copy.project.as_deref(), &scan.home),
                    detail: short_hash(&copy.hash.0),
                    note: if is_same { "相同" } else { "已分叉" }.to_owned(),
                    tone: (!is_same).then_some(LocationTone::Warn),
                }
            })
            .collect();
        findings.push(Finding {
            id: format!("duplicate:{name}"),
            category: FindingCategory::Duplicate,
            title,
            status: FindingStatus::Pending,
            intro,
            locations,
            action: "收编到资产库".to_owned(),
            foot: "原位置替换为受管链接，事前自动备份。".to_owned(),
        });
    }
}

fn repos(scan: &Scan, findings: &mut Vec<Finding>) {
    let mut by_repo: BTreeMap<&Path, BTreeSet<&str>> = BTreeMap::new();
    for copy in &scan.repo_skills {
        if let Some(root) = copy.project.as_deref() {
            by_repo
                .entry(root)
                .or_default()
                .insert(copy.hash.0.as_str());
        }
    }
    let shared: Vec<(&Path, usize)> = by_repo
        .iter()
        .filter_map(|(repo, hashes)| {
            let common = hashes
                .iter()
                .filter(|h| {
                    by_repo
                        .iter()
                        .any(|(other, hs)| other != repo && hs.contains(*h))
                })
                .count();
            (common > 0).then_some((*repo, common))
        })
        .collect();
    if shared.len() < 2 {
        return;
    }
    let locations = shared
        .iter()
        .map(|(repo, common)| FindingLocation {
            path: display_path(repo, &scan.home),
            detail: format!("{common} 个 skill"),
            note: "内容相同".to_owned(),
            tone: None,
        })
        .collect();
    findings.push(Finding {
        id: "duplicate:repos".to_owned(),
        category: FindingCategory::Duplicate,
        title: format!("{} 个仓库存放着相同的 skill", shared.len()),
        status: FindingStatus::Pending,
        intro: format!(
            "同一批 skill 出现在 {} 个仓库里。指定一个为正本，其余登记为镜像，不删除任何仓库。",
            shared.len()
        ),
        locations,
        action: "指定正本".to_owned(),
        foot: "只写入方寸的资产库索引。".to_owned(),
    });
}

/// 全局层的一组 skill：同一来源至少 [`GROUP_MIN`] 项合成一组，其余各自一组。
pub struct GlobalGroup<'a> {
    pub label: String,
    pub source: Option<String>,
    pub members: Vec<&'a SkillCopy>,
}

/// 把一组名字写成「lark-*」这样的共同前缀，没有像样的前缀时返回 `None`。
pub fn common_prefix_label(names: &[&str]) -> Option<String> {
    let first = names.first()?;
    let mut prefix_len = first.len();
    for name in &names[1..] {
        prefix_len = first
            .chars()
            .zip(name.chars())
            .take_while(|(a, b)| a == b)
            .map(|(a, _)| a.len_utf8())
            .sum::<usize>()
            .min(prefix_len);
    }
    let prefix = &first[..prefix_len];
    let cut = prefix.rfind(['-', '_', ':'])?;
    (cut >= 2).then(|| format!("{}*", &prefix[..=cut]))
}

/// 全局层 skill 按来源分组。输入按实际目录去重过的副本。
pub fn group_global_skills<'a>(skills: &[&'a SkillCopy]) -> Vec<GlobalGroup<'a>> {
    let mut by_source: BTreeMap<&str, Vec<&'a SkillCopy>> = BTreeMap::new();
    let mut groups = Vec::new();
    for skill in skills {
        match skill.source.as_deref() {
            Some(source) if !source.is_empty() => by_source.entry(source).or_default().push(skill),
            _ => groups.push(GlobalGroup {
                label: skill.name.clone(),
                source: None,
                members: vec![skill],
            }),
        }
    }
    for (source, members) in by_source {
        if members.len() >= GROUP_MIN {
            let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
            let label = common_prefix_label(&names).unwrap_or_else(|| source.to_owned());
            groups.push(GlobalGroup {
                label,
                source: Some(source.to_owned()),
                members,
            });
        } else {
            for member in members {
                groups.push(GlobalGroup {
                    label: member.name.clone(),
                    source: Some(source.to_owned()),
                    members: vec![member],
                });
            }
        }
    }
    groups
}

fn global_tax(scan: &Scan, findings: &mut Vec<Finding>) {
    // 各工具的全局 skill，按实际目录去重。
    let mut seen: BTreeSet<&PathBuf> = BTreeSet::new();
    let mut skills: Vec<&SkillCopy> = Vec::new();
    // 使用数据只来自完整支持的工具，所以只看它们的全局层。
    let full = scan
        .global
        .iter()
        .filter(|(tool, _)| scan.tools.contains(tool));
    for (_, collected) in full {
        for skill in &collected.skills {
            if seen.insert(&skill.real_dir) {
                skills.push(skill);
            }
        }
    }
    let used_by = |skill: &SkillCopy| -> BTreeSet<PathBuf> {
        scan.tools
            .iter()
            .flat_map(|tool| {
                scan.usage
                    .projects_using(tool, AssetKind::Skill, &skill.name)
                    .into_keys()
            })
            .collect()
    };

    let mut never_used: Vec<&SkillCopy> = Vec::new();
    for group in group_global_skills(&skills) {
        let projects: BTreeSet<PathBuf> = group.members.iter().flat_map(|m| used_by(m)).collect();
        if group.members.len() == 1 {
            if projects.is_empty() {
                never_used.push(group.members[0]);
            }
            continue;
        }
        if projects.len() > RARELY_USED_MAX {
            continue;
        }
        let resident: u32 = group.members.iter().map(|m| m.resident.0).sum();
        let names: Vec<&str> = group
            .members
            .iter()
            .take(3)
            .map(|m| m.name.as_str())
            .collect();
        let users: Vec<String> = projects.iter().map(|p| super::project_name(p)).collect();
        findings.push(Finding {
            id: format!("global-tax:{}", group.label),
            category: FindingCategory::GlobalTax,
            title: format!("{} 个 {} skill 加载进了每一个项目", group.members.len(), group.label),
            status: FindingStatus::Pending,
            intro: format!(
                "来源 {}。名称与描述常驻每个项目的上下文，近 30 天{}。把它们从全局层移到套装，只给需要的项目。",
                group.source.as_deref().unwrap_or("未知"),
                if users.is_empty() {
                    "没有项目触发过".to_owned()
                } else {
                    format!("只有 {} 触发过", users.join("、"))
                }
            ),
            locations: vec![
                FindingLocation {
                    path: format!("{} …", names.join(" · ")),
                    detail: format!("{} 项", group.members.len()),
                    note: format!("常驻约 {}", format_k(TokenEstimate(resident))),
                    tone: Some(LocationTone::Muted),
                },
                FindingLocation {
                    path: "用到它的项目".to_owned(),
                    detail: format!("{} / {}", users.len(), scan.projects.len()),
                    note: if users.is_empty() {
                        "无".to_owned()
                    } else {
                        users.join("、")
                    },
                    tone: None,
                },
            ],
            action: "创建套装".to_owned(),
            foot: "估算值。全局链接移除前自动快照。".to_owned(),
        });
    }

    if !never_used.is_empty() {
        let resident: u32 = never_used.iter().map(|m| m.resident.0).sum();
        let locations = never_used
            .iter()
            .map(|skill| FindingLocation {
                path: display_path(&skill.dir, &scan.home),
                detail: format_k(skill.resident),
                note: "从未使用".to_owned(),
                tone: Some(LocationTone::Warn),
            })
            .collect();
        findings.push(Finding {
            id: "global-tax:never-used".to_owned(),
            category: FindingCategory::GlobalTax,
            title: format!("{} 个全局 skill 近 30 天没有项目用到", never_used.len()),
            status: FindingStatus::Pending,
            intro: format!(
                "它们的名称与描述常驻每个项目，合计约 {}（估算）。依据本机会话记录近 30 天的触发情况，不上传。",
                format_k(TokenEstimate(resident))
            ),
            locations,
            action: "从全局移除".to_owned(),
            foot: "资产库保留正本，可在「活动」回滚。".to_owned(),
        });
    }
}

fn inconsistent(scan: &Scan, findings: &mut Vec<Finding>) {
    let names_by_tool: BTreeMap<_, BTreeSet<&str>> = scan
        .global
        .iter()
        .filter(|(tool, _)| scan.skill_tools.contains(*tool))
        .map(|(tool, c)| (tool, c.skills.iter().map(|s| s.name.as_str()).collect()))
        .collect();
    let all: BTreeSet<&str> = names_by_tool.values().flatten().copied().collect();
    if all.is_empty() || names_by_tool.values().all(|names| names.len() == all.len()) {
        return;
    }
    let locations = scan
        .global
        .iter()
        .filter(|(tool, _)| scan.skill_tools.contains(*tool))
        .map(|(tool, collected)| {
            let have = names_by_tool[tool].len();
            let dirs: BTreeSet<String> = collected
                .skills
                .iter()
                .filter_map(|s| s.dir.parent().map(|p| display_path(p, &scan.home)))
                .collect();
            FindingLocation {
                path: format!(
                    "{}  {}",
                    scan.tool_name(tool),
                    dirs.into_iter().collect::<Vec<_>>().join(" · ")
                ),
                detail: format!("{have} / {}", all.len()),
                note: if have == all.len() {
                    "齐全".to_owned()
                } else {
                    format!("缺 {}", all.len() - have)
                },
                tone: (have < all.len()).then_some(LocationTone::Warn),
            }
        })
        .collect();
    findings.push(Finding {
        id: "inconsistent:global-skills".to_owned(),
        category: FindingCategory::Inconsistent,
        title: format!("{} 个全局 skill 在各工具间分发不齐", all.len()),
        status: FindingStatus::Pending,
        intro: format!(
            "各工具的全局层合起来有 {} 个 skill，每个工具实际读到的数量不同。",
            all.len()
        ),
        locations,
        action: "标记为已知".to_owned(),
        foot: "推荐做法：把全局层瘦身后再统一分发。".to_owned(),
    });
}

fn detached(scan: &Scan, findings: &mut Vec<Finding>) {
    for lock in &scan.locks {
        let Ok(entries) = fs::read_dir(&lock.dir) else {
            continue;
        };
        let mut on_disk: BTreeSet<String> = BTreeSet::new();
        let mut newest: Option<Timestamp> = None;
        for entry in entries.flatten() {
            if !entry.path().join("SKILL.md").is_file() {
                continue;
            }
            on_disk.insert(entry.file_name().to_string_lossy().into_owned());
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                let at = Timestamp::from_system_time(modified);
                if newest.as_ref().is_none_or(|n| at > *n) {
                    newest = Some(at);
                }
            }
        }
        let in_lock: BTreeSet<String> = lock.sources.keys().cloned().collect();
        let only_lock: Vec<&String> = in_lock.difference(&on_disk).collect();
        let only_disk: Vec<&String> = on_disk.difference(&in_lock).collect();
        let stale = match (&lock.modified, &newest) {
            (Some(lock_at), Some(dir_at)) => dir_at > lock_at,
            _ => false,
        };
        if only_lock.is_empty() && only_disk.is_empty() {
            continue;
        }
        let day = |t: &Option<Timestamp>| -> String {
            t.as_ref()
                .map(|t| t.0.get(5..10).unwrap_or("").to_owned())
                .unwrap_or_default()
        };
        let mut intro = format!(
            "锁里 {} 项，目录里 {} 项：{} 项只在锁里，{} 项只在目录里。",
            in_lock.len(),
            on_disk.len(),
            only_lock.len(),
            only_disk.len()
        );
        if stale {
            intro.push_str(&format!(
                "锁文件最后修改于 {}，目录 {} 还在变动。",
                day(&lock.modified),
                day(&newest)
            ));
        }
        let mut locations = vec![
            FindingLocation {
                path: display_path(&lock.path, &scan.home),
                detail: day(&lock.modified),
                note: if stale { "过期" } else { "锁" }.to_owned(),
                tone: stale.then_some(LocationTone::Warn),
            },
            FindingLocation {
                path: format!("{}/", display_path(&lock.dir, &scan.home)),
                detail: day(&newest),
                note: "实际状态".to_owned(),
                tone: None,
            },
        ];
        for name in &only_lock {
            locations.push(FindingLocation {
                path: (*name).clone(),
                detail: "只在锁里".to_owned(),
                note: "目录里没有".to_owned(),
                tone: Some(LocationTone::Warn),
            });
        }
        for name in &only_disk {
            locations.push(FindingLocation {
                path: (*name).clone(),
                detail: "只在目录里".to_owned(),
                note: "来源未知".to_owned(),
                tone: Some(LocationTone::Muted),
            });
        }
        findings.push(Finding {
            id: format!("detached:{}", lock.path.display()),
            category: FindingCategory::Detached,
            title: "全局锁文件与实际目录不一致".to_owned(),
            status: FindingStatus::Pending,
            intro,
            locations,
            action: "按目录重建锁".to_owned(),
            foot: "旧锁备份到活动，可回滚。".to_owned(),
        });
    }
}

/// 常见密钥的前缀。
const SECRET_PREFIXES: &[&str] = &[
    "ghp_",
    "gho_",
    "ghs_",
    "github_pat_",
    "sk-",
    "sk_live_",
    "xoxb-",
    "xoxp-",
    "AKIA",
    "glpat-",
    "AIza",
];

/// 看起来像明文密钥：已知前缀，或变量名像密钥而值不是引用。
pub fn looks_like_secret(name: &str, value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('$') || value.starts_with("secret:") {
        return false;
    }
    if SECRET_PREFIXES.iter().any(|p| value.starts_with(p)) && value.len() >= 12 {
        return true;
    }
    let upper = name.to_ascii_uppercase();
    let secret_name = [
        "TOKEN",
        "SECRET",
        "PASSWORD",
        "API_KEY",
        "APIKEY",
        "ACCESS_KEY",
    ]
    .iter()
    .any(|k| upper.contains(k));
    secret_name && value.len() >= 12 && !value.contains(' ')
}

fn secrets(scan: &Scan, findings: &mut Vec<Finding>) {
    let mut hits: Vec<(&McpEntry, &str)> = Vec::new();
    for (root, per_tool) in &scan.project_items {
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
        for collected in per_tool.values() {
            for entry in collected.mcp.iter().filter(|m| m.scope == Scope::Project) {
                let file_name = entry
                    .file
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned());
                let ignored = file_name.as_deref().is_some_and(|n| {
                    ignore
                        .lines()
                        .any(|l| l.trim().trim_start_matches('/') == n)
                });
                if ignored {
                    continue;
                }
                for (name, value) in &entry.env {
                    if looks_like_secret(name, value) {
                        hits.push((entry, name.as_str()));
                    }
                }
            }
        }
    }
    if hits.is_empty() {
        return;
    }
    let locations = hits
        .iter()
        .map(|(entry, name)| FindingLocation {
            path: location_label(&entry.file, entry.project.as_deref(), &scan.home),
            detail: format!("{}.env.{name}", entry.key),
            note: "明文".to_owned(),
            tone: Some(LocationTone::Warn),
        })
        .collect();
    findings.push(Finding {
        id: "secret:mcp".to_owned(),
        category: FindingCategory::Secret,
        title: format!("{} 处已提交的 MCP 配置里有明文密钥", hits.len()),
        status: FindingStatus::Pending,
        intro: "在项目目录里会提交的 MCP 配置中找到了像密钥的值。只记位置，不读出、不上传值本身。"
            .to_owned(),
        locations,
        action: "迁入钥匙串".to_owned(),
        foot: "配置里改为环境变量引用，值存进 macOS 钥匙串。".to_owned(),
    });
}

/// 「6.4k」这样的写法。
pub fn format_k(tokens: TokenEstimate) -> String {
    if tokens.0 > 0 && tokens.0 < 50 {
        return "<0.1k".to_owned();
    }
    let tenths = (u64::from(tokens.0) + 50) / 100;
    format!("{}.{}k", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_label_needs_a_separator() {
        assert_eq!(
            common_prefix_label(&["lark-doc", "lark-base", "lark-im"]).as_deref(),
            Some("lark-*")
        );
        assert_eq!(common_prefix_label(&["pdf", "pptx"]), None);
    }

    #[test]
    fn secret_detection_ignores_references() {
        assert!(looks_like_secret("GITHUB_TOKEN", "ghp_abcdefghijklmnop"));
        assert!(looks_like_secret("API_KEY", "abcdefghijklmnop"));
        assert!(!looks_like_secret("GITHUB_TOKEN", "${GITHUB_TOKEN}"));
        assert!(!looks_like_secret("GITHUB_TOKEN", "secret:github-token"));
        assert!(!looks_like_secret("LOG_LEVEL", "debug-verbose-mode"));
    }

    #[test]
    fn format_k_uses_one_decimal() {
        assert_eq!(format_k(TokenEstimate(6400)), "6.4k");
        assert_eq!(format_k(TokenEstimate(250)), "0.3k");
        assert_eq!(format_k(TokenEstimate(30)), "<0.1k");
        assert_eq!(format_k(TokenEstimate(0)), "0.0k");
    }
}
