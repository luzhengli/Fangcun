//! 全局页（6.6）的只读部分：全局层每一项的常驻、近 30 天用到它的项目、状态与建议。
//!
//! 转成套装、从全局移除属于 M1，这里只给出建议。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::{Adapter, AdapterRegistry};
use crate::model::{AssetKind, Scope, TokenEstimate, ToolId};
use crate::resolver::{Budget, DEFAULT_BUDGET, mcp_command};
use crate::scanner::fsutil::display_path;
use crate::scanner::{Scan, SkillCopy, format_k, group_global_skills, short_hash};

/// 全局页的状态（6.6「状态」表），按近 30 天用到的项目数给出。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum GlobalStatus {
    /// 建议转套装：只有一两个项目用到。蓝。
    SuggestBundle,
    /// 从未使用：近 30 天没有项目触发。橙。
    NeverUsed,
    /// 被覆盖 N 处：有项目在项目层放了同名项，把它遮蔽了。灰。
    Overridden,
    /// 来源未知：内容哈希与任何来源都不匹配。紫。
    UnknownSource,
    /// 常用。绿。
    Common,
    /// 多数项目在用：一半以上。绿。
    MostProjects,
    /// 每个项目都用。绿。
    EveryProject,
    /// 全文加载：指令文件每次会话都加载，触发次数无从统计。灰。
    AlwaysLoaded,
    /// 已停用：配置里的开关停用了它，工具不加载。灰。
    Disabled,
}

/// 「用到它的项目」里一个项目的标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum UseMark {
    Used,
    Unused,
    Overridden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectUse {
    pub name: String,
    pub path: String,
    pub mark: UseMark,
    pub triggers: u32,
}

/// 「分发到哪些工具」的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Distribution {
    pub tool: ToolId,
    pub tool_name: String,
    pub path: Option<String>,
    /// 已链接、实体文件、未分发、按键合并、全文加载、未配置。
    pub label: String,
    /// 这个工具有没有读到它。
    pub present: bool,
}

/// 全局页列表里的一行。来源相同的一组合成一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GlobalRow {
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    /// 合成一行的项数。单项为 1。
    pub count: u32,
    pub members: Vec<String>,
    pub location: String,
    pub source: Option<String>,
    pub version: Option<String>,
    /// 给每个项目带来的常驻。MCP 没有估算时为空。
    pub resident: Option<TokenEstimate>,
    /// 近 30 天用到它的项目数。指令文件不统计，为空。
    pub used_by: Option<u32>,
    pub status: GlobalStatus,
    /// 被项目层覆盖的项目数。
    pub overridden: u32,
    pub has_scripts: bool,
    pub command: Option<String>,
    pub usage: Vec<ProjectUse>,
    pub distribution: Vec<Distribution>,
    /// 检查器名称下面的一句说明。
    pub summary: String,
    /// 检查器「建议」一段话。
    pub advice: String,
    /// 建议下面的按钮：转成套装…、从全局移除…、去资产库收编。M1 才能执行。
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KindCounts {
    pub skill: u32,
    pub mcp: u32,
    pub instruction: u32,
}

/// 别的工具全局层里有、当前工具没有的项数。列表末尾那句「另有 N 项只被 Codex 加载」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OtherToolCount {
    pub tool: ToolId,
    pub tool_name: String,
    pub count: u32,
}

/// 全局页（6.6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GlobalOverview {
    pub tool: ToolId,
    /// 全局层给每个项目带来的常驻。
    pub budget: Budget,
    pub project_total: u32,
    pub kinds: KindCounts,
    /// 「很少用到 · N」：状态为建议转套装的项数。
    pub rarely_used: u32,
    /// 「从未使用 · N」：状态为从未使用或被覆盖的项数。
    pub never_used: u32,
    pub rows: Vec<GlobalRow>,
    pub other_tools: Vec<OtherToolCount>,
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// 按近 30 天用到的项目数给出状态。
pub fn status_for(used: usize, total: usize) -> GlobalStatus {
    if used == 0 {
        GlobalStatus::NeverUsed
    } else if total > 0 && used == total {
        GlobalStatus::EveryProject
    } else if used * 2 > total {
        GlobalStatus::MostProjects
    } else if used <= 2 {
        GlobalStatus::SuggestBundle
    } else {
        GlobalStatus::Common
    }
}

struct Ctx<'a> {
    scan: &'a Scan,
    adapter: &'a Adapter,
    adapters: &'a AdapterRegistry,
}

impl Ctx<'_> {
    fn tool(&self) -> &ToolId {
        &self.adapter.tool.id
    }

    fn total(&self) -> usize {
        self.scan.projects.len()
    }

    fn users(&self, kind: AssetKind, names: &[&str]) -> BTreeMap<PathBuf, u32> {
        let mut out: BTreeMap<PathBuf, u32> = BTreeMap::new();
        for name in names {
            for (project, n) in self.scan.usage.projects_using(self.tool(), kind, name) {
                *out.entry(project).or_default() += n;
            }
        }
        out
    }

    /// 在哪些项目里，项目层的同名项把全局这一份遮蔽了。
    fn overridden_in(&self, kind: AssetKind, name: &str) -> BTreeSet<PathBuf> {
        let priority = match kind {
            AssetKind::Skill => &self.adapter.precedence.skills,
            AssetKind::Mcp => &self.adapter.precedence.mcp,
            AssetKind::Instruction => return BTreeSet::new(),
        };
        let mut out = BTreeSet::new();
        for (root, per_tool) in &self.scan.project_items {
            let Some(collected) = per_tool.get(self.tool()) else {
                continue;
            };
            let scopes: BTreeSet<Scope> = match kind {
                AssetKind::Skill => collected
                    .skills
                    .iter()
                    .filter(|s| s.name == name)
                    .map(|s| s.scope)
                    .collect(),
                _ => collected
                    .mcp
                    .iter()
                    .filter(|m| m.name == name && m.inactive.is_none())
                    .map(|m| m.scope)
                    .collect(),
            };
            if scopes.is_empty() {
                continue;
            }
            let win = priority
                .iter()
                .copied()
                .find(|s| *s == Scope::Global || scopes.contains(s));
            if win.is_some_and(|w| w != Scope::Global) {
                out.insert(root.clone());
            }
        }
        out
    }

    fn usage(
        &self,
        users: &BTreeMap<PathBuf, u32>,
        overridden: &BTreeSet<PathBuf>,
    ) -> Vec<ProjectUse> {
        self.scan
            .projects
            .iter()
            .map(|p| {
                let triggers = users.get(&p.root).copied().unwrap_or(0);
                let mark = if overridden.contains(&p.root) {
                    UseMark::Overridden
                } else if triggers > 0 {
                    UseMark::Used
                } else {
                    UseMark::Unused
                };
                ProjectUse {
                    name: p.name.clone(),
                    path: display_path(&p.root, &self.scan.home),
                    mark,
                    triggers,
                }
            })
            .collect()
    }
}

fn advice(
    status: GlobalStatus,
    used_names: &[String],
    total: usize,
    resident: Option<TokenEstimate>,
    overridden: usize,
) -> (String, Vec<String>) {
    let saving = resident.map_or_else(
        || "启动这个服务的开销".to_owned(),
        |r| format!("{} 常驻", format_k(r)),
    );
    match status {
        GlobalStatus::SuggestBundle => (
            format!(
                "只有 {} 用到。转成套装后，其余 {} 个项目每个省下 {saving}。",
                used_names.join("、"),
                total.saturating_sub(used_names.len())
            ),
            vec!["转成套装…".to_owned(), "从全局移除…".to_owned()],
        ),
        GlobalStatus::NeverUsed => (
            format!("近 30 天没有项目触发。从全局移除后，每个项目省下 {saving}。"),
            vec!["从全局移除…".to_owned()],
        ),
        GlobalStatus::Overridden => (
            format!("{overridden} 个项目在项目层放了同名项，把它遮蔽了。留在全局只是占位。"),
            vec!["从全局移除…".to_owned()],
        ),
        GlobalStatus::UnknownSource => (
            "内容哈希与任何来源都不匹配。先去资产库收编，再决定留还是走。".to_owned(),
            vec!["去资产库收编".to_owned()],
        ),
        GlobalStatus::Disabled => (
            "配置里已经停用，不占常驻。要彻底清掉，在 M1 从全局移除。".to_owned(),
            Vec::new(),
        ),
        GlobalStatus::AlwaysLoaded => (
            "全局指令每次会话全文加载，没有「触发」可统计。只放每个项目都需要的约定。".to_owned(),
            Vec::new(),
        ),
        GlobalStatus::Common | GlobalStatus::MostProjects | GlobalStatus::EveryProject => (
            format!("{} / {total} 个项目用到，留在全局。", used_names.len()),
            Vec::new(),
        ),
    }
}

fn skill_distribution(ctx: &Ctx<'_>, names: &[&str]) -> Vec<Distribution> {
    ctx.adapters
        .iter()
        .filter(|a| ctx.scan.skill_tools.contains(&a.tool.id))
        .map(|a| {
            let found: Vec<&SkillCopy> = ctx
                .scan
                .global
                .get(&a.tool.id)
                .map(|c| {
                    c.skills
                        .iter()
                        .filter(|s| names.contains(&s.name.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            let path = found.first().and_then(|s| s.dir.parent()).map(|p| {
                let dir = display_path(p, &ctx.scan.home);
                if names.len() == 1 {
                    format!("{dir}/{}", names[0])
                } else {
                    format!("{dir}/ ({} / {})", found.len(), names.len())
                }
            });
            let label = if found.is_empty() {
                "未分发"
            } else if found.iter().all(|s| s.link_target.is_some()) {
                "已链接"
            } else {
                "实体文件"
            };
            Distribution {
                tool: a.tool.id.clone(),
                tool_name: a.tool.name.clone(),
                path,
                label: label.to_owned(),
                present: !found.is_empty(),
            }
        })
        .collect()
}

/// 全局页的数据。
pub fn overview(scan: &Scan, adapters: &AdapterRegistry, adapter: &Adapter) -> GlobalOverview {
    let ctx = Ctx {
        scan,
        adapter,
        adapters,
    };
    let home = scan.home.as_path();
    let total = ctx.total();
    let empty = crate::scanner::Collected::default();
    let global = scan.global.get(ctx.tool()).unwrap_or(&empty);
    let project_names = |users: &BTreeMap<PathBuf, u32>| -> Vec<String> {
        scan.projects
            .iter()
            .filter(|p| users.contains_key(&p.root))
            .map(|p| p.name.clone())
            .collect()
    };
    let mut rows = Vec::new();

    // Skills
    let mut seen = BTreeSet::new();
    let skills: Vec<&SkillCopy> = global
        .skills
        .iter()
        .filter(|s| s.disabled_by.is_none() && seen.insert(s.real_dir.clone()))
        .collect();
    for group in group_global_skills(&skills) {
        let names: Vec<&str> = group.members.iter().map(|m| m.name.as_str()).collect();
        let users = ctx.users(AssetKind::Skill, &names);
        let overridden: BTreeSet<PathBuf> = names
            .iter()
            .flat_map(|n| ctx.overridden_in(AssetKind::Skill, n))
            .collect();
        let resident = TokenEstimate(group.members.iter().map(|m| m.resident.0).sum());
        let unknown = group.members.iter().all(|m| m.source.is_none());
        let status = if !overridden.is_empty() {
            GlobalStatus::Overridden
        } else if unknown {
            GlobalStatus::UnknownSource
        } else {
            status_for(users.len(), total)
        };
        let used_names = project_names(&users);
        let (advice, actions) =
            advice(status, &used_names, total, Some(resident), overridden.len());
        let first = group.members[0];
        let single = group.members.len() == 1;
        let location = if single {
            display_path(&first.dir, home)
        } else {
            let dir = first
                .dir
                .parent()
                .map(|p| display_path(p, home))
                .unwrap_or_default();
            format!("{dir}/{}", group.label)
        };
        let used_text = if used_names.is_empty() {
            "近 30 天没有项目触发过".to_owned()
        } else if used_names.len() <= 2 {
            format!("近 30 天只有 {} 触发过", used_names.join("、"))
        } else {
            format!("近 30 天 {} 个项目触发过", used_names.len())
        };
        let summary = if single {
            format!("名称与描述常驻每个项目的上下文，{used_text}。")
        } else {
            format!(
                "{} 个 skill，来自 {}。名称与描述常驻每个项目的上下文，{used_text}。",
                group.members.len(),
                group.source.as_deref().unwrap_or("同一来源")
            )
        };
        rows.push(GlobalRow {
            summary,
            id: format!("skill:{}", group.label),
            name: if single {
                first.name.clone()
            } else {
                group.label.clone()
            },
            kind: AssetKind::Skill,
            count: u32_of(group.members.len()),
            members: names.iter().map(|n| (*n).to_owned()).collect(),
            location,
            source: group.source.clone(),
            version: single.then(|| short_hash(&first.hash.0)),
            resident: Some(resident),
            used_by: Some(u32_of(users.len())),
            status,
            overridden: u32_of(overridden.len()),
            has_scripts: group.members.iter().any(|m| m.has_scripts),
            command: None,
            usage: ctx.usage(&users, &overridden),
            distribution: skill_distribution(&ctx, &names),
            advice,
            actions,
        });
    }

    for skill in global.skills.iter().filter(|s| s.disabled_by.is_some()) {
        let (advice, actions) = advice(GlobalStatus::Disabled, &[], total, None, 0);
        rows.push(GlobalRow {
            id: format!("skill-disabled:{}", skill.dir.display()),
            name: skill.name.clone(),
            kind: AssetKind::Skill,
            count: 1,
            members: vec![skill.name.clone()],
            location: display_path(&skill.dir, home),
            source: skill.source.clone(),
            version: Some(short_hash(&skill.hash.0)),
            resident: None,
            used_by: None,
            status: GlobalStatus::Disabled,
            overridden: 0,
            has_scripts: skill.has_scripts,
            command: None,
            usage: Vec::new(),
            distribution: skill_distribution(&ctx, &[skill.name.as_str()]),
            summary: format!(
                "{} 里的开关停用了它。",
                skill.disabled_by.clone().unwrap_or_default()
            ),
            advice,
            actions,
        });
    }

    // MCP
    for entry in &global.mcp {
        let users = ctx.users(AssetKind::Mcp, &[entry.name.as_str()]);
        let overridden = ctx.overridden_in(AssetKind::Mcp, &entry.name);
        let status = if !overridden.is_empty() {
            GlobalStatus::Overridden
        } else {
            status_for(users.len(), total)
        };
        let used_names = project_names(&users);
        let (advice, actions) = advice(status, &used_names, total, None, overridden.len());
        let distribution = adapters
            .iter()
            .filter(|a| scan.scanned_tools.contains(&a.tool.id))
            .map(|a| {
                let found = scan
                    .global
                    .get(&a.tool.id)
                    .and_then(|c| c.mcp.iter().find(|m| m.name == entry.name));
                Distribution {
                    tool: a.tool.id.clone(),
                    tool_name: a.tool.name.clone(),
                    path: found.map(|m| format!("{} · {}", display_path(&m.file, home), m.key)),
                    label: if found.is_some() {
                        "按键合并"
                    } else {
                        "未配置"
                    }
                    .to_owned(),
                    present: found.is_some(),
                }
            })
            .collect();
        rows.push(GlobalRow {
            summary: format!(
                "全局 MCP 服务，每次会话都会启动。近 30 天 {} 个项目调用过。",
                users.len()
            ),
            id: format!("mcp:{}", entry.name),
            name: entry.name.clone(),
            kind: AssetKind::Mcp,
            count: 1,
            members: vec![entry.name.clone()],
            location: format!("{} · {}", display_path(&entry.file, home), entry.key),
            source: None,
            version: Some(short_hash(&entry.hash.0)),
            resident: None,
            used_by: Some(u32_of(users.len())),
            status,
            overridden: u32_of(overridden.len()),
            has_scripts: entry.command.is_some(),
            command: mcp_command(entry),
            usage: ctx.usage(&users, &overridden),
            distribution,
            advice,
            actions,
        });
    }

    // 指令文件
    for file in &global.instructions {
        let (advice, actions) = advice(
            GlobalStatus::AlwaysLoaded,
            &[],
            total,
            Some(file.resident),
            0,
        );
        let counted = !file.on_demand && !file.truncated;
        let distribution = adapters
            .iter()
            .filter(|a| scan.scanned_tools.contains(&a.tool.id))
            .map(|a| {
                let found = scan.global.get(&a.tool.id).and_then(|c| {
                    c.instructions
                        .iter()
                        .find(|i| i.real_path == file.real_path)
                });
                Distribution {
                    tool: a.tool.id.clone(),
                    tool_name: a.tool.name.clone(),
                    path: found.map(|i| display_path(&i.path, home)),
                    label: if found.is_some() {
                        "全文加载"
                    } else {
                        "未配置"
                    }
                    .to_owned(),
                    present: found.is_some(),
                }
            })
            .collect();
        rows.push(GlobalRow {
            summary: "用户级指令文件，每个项目的每次会话都全文加载。".to_owned(),
            id: format!("instruction:{}", file.path.display()),
            name: file
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.name.clone()),
            kind: AssetKind::Instruction,
            count: 1,
            members: Vec::new(),
            location: display_path(&file.path, home),
            source: None,
            version: file.hash.as_ref().map(|h| short_hash(&h.0)),
            resident: counted.then_some(file.resident),
            used_by: None,
            status: GlobalStatus::AlwaysLoaded,
            overridden: 0,
            has_scripts: false,
            command: None,
            usage: Vec::new(),
            distribution,
            advice,
            actions,
        });
    }

    rows.sort_by(|a, b| {
        b.resident
            .is_some()
            .cmp(&a.resident.is_some())
            .then(b.resident.cmp(&a.resident))
            .then(a.name.cmp(&b.name))
    });

    let mut budget = Budget {
        limit: DEFAULT_BUDGET,
        ..Budget::default()
    };
    let mut kinds = KindCounts::default();
    let mut rarely_used = 0;
    let mut never_used = 0;
    for row in &rows {
        match row.kind {
            AssetKind::Skill => {
                kinds.skill += row.count;
                budget.skills.0 += row.resident.map_or(0, |r| r.0);
            }
            AssetKind::Mcp => {
                kinds.mcp += row.count;
                match row.resident {
                    Some(r) => budget.mcp.0 += r.0,
                    None => budget.mcp_unestimated += 1,
                }
            }
            AssetKind::Instruction => {
                kinds.instruction += row.count;
                budget.instructions.0 += row.resident.map_or(0, |r| r.0);
            }
        }
        match row.status {
            GlobalStatus::SuggestBundle => rarely_used += row.count,
            GlobalStatus::NeverUsed | GlobalStatus::Overridden => never_used += row.count,
            _ => {}
        }
    }

    let mine: BTreeSet<String> = rows
        .iter()
        .flat_map(|r| r.members.iter().cloned())
        .chain(
            global
                .instructions
                .iter()
                .map(|i| i.real_path.display().to_string()),
        )
        .collect();
    let other_tools = adapters
        .iter()
        .filter(|a| a.tool.id != *ctx.tool() && scan.tools.contains(&a.tool.id))
        .filter_map(|a| {
            let other = scan.global.get(&a.tool.id)?;
            let names: BTreeSet<String> = other
                .skills
                .iter()
                .map(|s| s.name.clone())
                .chain(other.mcp.iter().map(|m| m.name.clone()))
                .chain(
                    other
                        .instructions
                        .iter()
                        .map(|i| i.real_path.display().to_string()),
                )
                .collect();
            let count = u32_of(names.iter().filter(|n| !mine.contains(*n)).count());
            (count > 0).then(|| OtherToolCount {
                tool: a.tool.id.clone(),
                tool_name: a.tool.name.clone(),
                count,
            })
        })
        .collect();

    GlobalOverview {
        tool: ctx.tool().clone(),
        budget,
        project_total: u32_of(total),
        kinds,
        rarely_used,
        never_used,
        rows,
        other_tools,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_follows_the_design_table() {
        assert_eq!(status_for(0, 6), GlobalStatus::NeverUsed);
        assert_eq!(status_for(1, 6), GlobalStatus::SuggestBundle);
        assert_eq!(status_for(2, 6), GlobalStatus::SuggestBundle);
        assert_eq!(status_for(3, 6), GlobalStatus::Common);
        assert_eq!(status_for(4, 6), GlobalStatus::MostProjects);
        assert_eq!(status_for(6, 6), GlobalStatus::EveryProject);
    }
}
