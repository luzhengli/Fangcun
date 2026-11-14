//! 解析器（7.2）：合并四层作用域，解出确定结果。
//!
//! 两个出口：
//! - [`effective_context`]：某个项目在某个工具下实际加载了什么（F5、6.3）。M0 就要，只读。
//! - [`resolve_lock`]：按清单解出确切版本，写锁（F3）。M1。
//!
//! M0 没有清单，生效上下文直接按磁盘现状和适配器的优先级规则解。
//! 同名时谁生效按工具的真实规则，不按方寸「下层覆盖上层」的设想：
//! 例如 Claude Code 的 skill 是全局层盖过项目层。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::{Adapter, MergeMode};
use crate::manifest::{Lock, Manifest};
use crate::model::{AssetKind, AssetState, ObservedKind, Scope, TokenEstimate, ToolId};
use crate::scanner::fsutil::display_path;
use crate::scanner::{
    Collected, InstructionFile, McpEntry, Scan, SkillCopy, group_global_skills, short_hash,
};
use crate::{Error, Result};

/// 项目预算的默认值。清单里怎么写预算还没定，先用这个数。
pub const DEFAULT_BUDGET: TokenEstimate = TokenEstimate(20_000);

/// 上下文预算条：常驻成本分三段，对照项目预算。全部是估算值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Budget {
    pub limit: TokenEstimate,
    pub instructions: TokenEstimate,
    pub skills: TokenEstimate,
    pub mcp: TokenEstimate,
    /// 没有估算的 MCP 服务数。工具定义要启动服务才读得到，M0 不启动进程。
    pub mcp_unestimated: u32,
}

impl Budget {
    pub fn total(&self) -> TokenEstimate {
        TokenEstimate(self.instructions.0 + self.skills.0 + self.mcp.0)
    }
}

/// 「在哪一层声明 · 谁覆盖谁」的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Declaration {
    pub scope: Scope,
    /// 这一层里的位置。没有声明时为空。
    pub location: Option<String>,
    /// 生效的那层。
    pub effective: bool,
}

/// 生效上下文列表里的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EffectiveItem {
    /// 在这份生效上下文里唯一。
    pub id: String,
    pub name: String,
    pub kind: AssetKind,
    /// 短哈希。
    pub version: Option<String>,
    /// 生效的那一层。
    pub scope: Scope,
    /// 来自套装时是套装名。
    pub bundle: Option<String>,
    /// 六种语义里的一种。为空时圆点用灰色。
    pub state: Option<AssetState>,
    /// 状态列的短标签。
    pub label: String,
    /// 常驻估算。没有估算时为空。
    pub resident: Option<TokenEstimate>,
    /// 计不计入预算：被覆盖、按需加载、超出上限的不计。
    pub counted: bool,
    /// 位置，用 `~` 写。
    pub location: String,
    pub link_target: Option<String>,
    pub source: Option<String>,
    pub has_scripts: bool,
    /// 合成一行的成员，例如「lark-* · 16 项」。
    pub members: Vec<String>,
    /// MCP 的启动命令或地址。
    pub command: Option<String>,
    pub declared: Vec<Declaration>,
    /// 为什么是这个状态，两句白话。
    pub reason: String,
    /// 近 30 天在本项目被触发的次数。指令文件不统计，为空。
    pub triggers: Option<u32>,
    /// 写入了哪些文件：这里是工具实际读的文件。
    pub files: Vec<String>,
    /// 被谁覆盖：生效那一项的 `id`。
    pub overridden_by: Option<String>,
}

/// 「其他加载项」里的一行，只看不管。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ObservedItem {
    pub name: String,
    pub kind: ObservedKind,
    pub scope: Scope,
    pub note: Option<String>,
    pub location: String,
}

/// 按「项目 × 工具」解出的生效上下文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EffectiveContext {
    pub tool: ToolId,
    pub project_name: String,
    /// 项目路径，用 `~` 写。
    pub project_path: String,
    /// 项目有没有 `fangcun.toml`。
    pub has_manifest: bool,
    pub budget: Budget,
    pub items: Vec<EffectiveItem>,
    pub observed: Vec<ObservedItem>,
    /// 适配器能读、但这个项目里没发现的「其他加载项」类型。
    pub observed_missing: Vec<ObservedKind>,
}

/// `fangcun why <资产>` 的回答：为什么生效、来自哪一层、谁覆盖谁。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Explanation {
    pub tool: ToolId,
    pub tool_name: String,
    pub name: String,
    pub kind: AssetKind,
    pub state: Option<AssetState>,
    pub label: String,
    /// 生效的那一层。
    pub effective_scope: Scope,
    pub declared: Vec<Declaration>,
    /// 两句白话。
    pub reason: String,
}

pub fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Global => "全局",
        Scope::Bundle => "套装",
        Scope::Project => "项目",
        Scope::Personal => "个人",
    }
}

fn kind_key(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Skill => "skill",
        AssetKind::Mcp => "mcp",
        AssetKind::Instruction => "instruction",
    }
}

fn declarations(present: &BTreeMap<Scope, String>, winner: Option<Scope>) -> Vec<Declaration> {
    Scope::ALL
        .iter()
        .map(|scope| Declaration {
            scope: *scope,
            location: present.get(scope).cloned(),
            effective: Some(*scope) == winner,
        })
        .collect()
}

/// 谁赢：优先级表里排在前面的层。表为空表示都加载，返回 `None`。
fn winner(priority: &[Scope], scopes: &BTreeSet<Scope>) -> Option<Scope> {
    priority.iter().copied().find(|s| scopes.contains(s))
}

fn priority_text(priority: &[Scope]) -> String {
    priority
        .iter()
        .map(|s| format!("{}层", scope_name(*s)))
        .collect::<Vec<_>>()
        .join("优先于")
}

/// 解出某个项目在某个工具下实际加载的全部内容。
pub fn effective_context(
    scan: &Scan,
    adapter: &Adapter,
    project: &Collected,
    project_root: &Path,
) -> EffectiveContext {
    let tool = &adapter.tool.id;
    let tool_name = adapter.tool.name.as_str();
    let empty = Collected::default();
    let global = scan.global.get(tool).unwrap_or(&empty);
    let home = scan.home.as_path();
    let mut items = Vec::new();

    instructions(global, project, home, tool_name, &mut items);
    skills(scan, adapter, global, project, project_root, &mut items);
    mcp(scan, adapter, global, project, project_root, &mut items);

    let mut budget = Budget {
        limit: DEFAULT_BUDGET,
        ..Budget::default()
    };
    for item in items.iter().filter(|i| i.counted) {
        let Some(tokens) = item.resident else {
            if item.kind == AssetKind::Mcp {
                budget.mcp_unestimated += 1;
            }
            continue;
        };
        let slot = match item.kind {
            AssetKind::Instruction => &mut budget.instructions,
            AssetKind::Skill => &mut budget.skills,
            AssetKind::Mcp => &mut budget.mcp,
        };
        slot.0 += tokens.0;
    }

    let observed: Vec<ObservedItem> = global
        .observed
        .iter()
        .chain(project.observed.iter())
        .map(|o| ObservedItem {
            name: o.name.clone(),
            kind: o.kind,
            scope: o.scope,
            note: o.note.clone(),
            location: display_path(&o.location, home),
        })
        .collect();
    let readable: BTreeSet<ObservedKind> = adapter.observed.iter().map(|o| o.kind).collect();
    let seen: BTreeSet<ObservedKind> = observed.iter().map(|o| o.kind).collect();
    let all_kinds = [
        ObservedKind::Plugin,
        ObservedKind::Hook,
        ObservedKind::SlashCommand,
        ObservedKind::Subagent,
    ];
    let observed_missing = all_kinds
        .into_iter()
        .filter(|k| !seen.contains(k) && (readable.contains(k) || readable.is_empty()))
        .collect();

    EffectiveContext {
        tool: tool.clone(),
        project_name: crate::scanner::project_name(project_root),
        project_path: display_path(project_root, home),
        has_manifest: project_root.join(crate::paths::MANIFEST_FILE).is_file(),
        budget,
        items,
        observed,
        observed_missing,
    }
}

fn instructions(
    global: &Collected,
    project: &Collected,
    home: &Path,
    tool_name: &str,
    items: &mut Vec<EffectiveItem>,
) {
    for file in global
        .instructions
        .iter()
        .chain(project.instructions.iter())
    {
        items.push(instruction_item(file, home, tool_name));
    }
}

fn instruction_item(file: &InstructionFile, home: &Path, tool_name: &str) -> EffectiveItem {
    let resident = file.resident;
    let counted = !file.on_demand && !file.truncated;
    let (label, reason) = if file.truncated {
        (
            "超出大小上限".to_owned(),
            format!("{tool_name} 合并指令文件有大小上限，到这一份已经超出。超出的部分不会加载。"),
        )
    } else if file.on_demand {
        (
            "按需加载".to_owned(),
            "这条规则声明了 paths，只有碰到匹配的文件时才加载。不计入常驻。".to_owned(),
        )
    } else if let Some(target) = &file.mirror_of {
        (
            format!("镜像自 {target}"),
            format!(
                "这份文件指向 {target}，{tool_name} 读它时读到的是 {target} 的全文。每次会话都加载。"
            ),
        )
    } else if file.fallback {
        (
            "代替首选文件".to_owned(),
            format!("项目里没有 {tool_name} 首选的指令文件，所以改读这一份。每次会话都加载全文。"),
        )
    } else if file.above_root {
        (
            "上层目录".to_owned(),
            format!(
                "它在项目根目录上面的目录里，{tool_name} 从工作目录往上逐层读取。每次会话都加载全文。"
            ),
        )
    } else {
        (
            "全文加载".to_owned(),
            format!(
                "{tool_name} 在{}层读到这份指令文件。每次会话都加载全文。",
                scope_name(file.scope)
            ),
        )
    };
    let location = display_path(&file.path, home);
    let mut files = vec![location.clone()];
    files.extend(
        file.imports
            .iter()
            .map(|p| format!("{} · 经 @ 引用带入", display_path(p, home))),
    );
    let mut present = BTreeMap::new();
    present.insert(file.scope, location.clone());
    EffectiveItem {
        id: format!("instruction:{}", file.path.display()),
        name: file.name.clone(),
        kind: AssetKind::Instruction,
        version: file.hash.as_ref().map(|h| short_hash(&h.0)),
        scope: file.scope,
        bundle: None,
        state: None,
        label,
        resident: Some(resident),
        counted,
        location: location.clone(),
        link_target: file.link_target.as_ref().map(|t| t.display().to_string()),
        source: None,
        has_scripts: false,
        members: Vec::new(),
        command: None,
        declared: declarations(&present, Some(file.scope)),
        reason,
        triggers: None,
        files,
        overridden_by: None,
    }
}

/// 项目层 skill 指向某个来源仓库时，来源写成 `local:<仓库名>`。
fn local_source(scan: &Scan, skill: &SkillCopy) -> Option<String> {
    scan.repo_skills
        .iter()
        .find(|r| r.real_dir == skill.real_dir)
        .and_then(|r| r.project.as_deref())
        .map(|root| format!("local:{}", crate::scanner::project_name(root)))
}

fn skills(
    scan: &Scan,
    adapter: &Adapter,
    global: &Collected,
    project: &Collected,
    project_root: &Path,
    items: &mut Vec<EffectiveItem>,
) {
    let tool = &adapter.tool.id;
    let tool_name = adapter.tool.name.as_str();
    let home = scan.home.as_path();
    let priority = &adapter.precedence.skills;
    let usage_for = |name: &str| -> u32 {
        scan.usage
            .projects_using(tool, AssetKind::Skill, name)
            .get(project_root)
            .copied()
            .unwrap_or(0)
    };

    // 同名的各份，按实际目录去重：多处软链指向同一目标只算一份。
    let mut by_name: BTreeMap<&str, Vec<&SkillCopy>> = BTreeMap::new();
    let mut rows: Vec<EffectiveItem> = Vec::new();
    for skill in global.skills.iter().chain(project.skills.iter()) {
        if let Some(switch) = &skill.disabled_by {
            let mut item = single_global(skill, usage_for(&skill.name), tool_name, home);
            let mut present = BTreeMap::new();
            present.insert(skill.scope, display_path(&skill.dir, home));
            item.scope = skill.scope;
            item.state = None;
            item.label = "已停用".to_owned();
            item.counted = false;
            item.declared = declarations(&present, None);
            item.reason = format!("{switch} 里的开关把它停用了，{tool_name} 不加载它。");
            rows.push(item);
            continue;
        }
        let list = by_name.entry(skill.name.as_str()).or_default();
        if !list.iter().any(|s| s.real_dir == skill.real_dir) {
            list.push(skill);
        }
    }

    // 只有全局层、没有同名冲突的 skill，按来源合成一组再出行。
    let mut plain_global: Vec<&SkillCopy> = Vec::new();
    for (name, copies) in &by_name {
        let scopes: BTreeSet<Scope> = copies.iter().map(|c| c.scope).collect();
        if copies.len() == 1 && copies[0].scope == Scope::Global {
            plain_global.push(copies[0]);
            continue;
        }
        let mut present = BTreeMap::new();
        for copy in copies {
            present
                .entry(copy.scope)
                .or_insert_with(|| display_path(&copy.dir, home));
        }
        let win = if copies.len() > 1 {
            winner(priority, &scopes)
        } else {
            copies.first().map(|c| c.scope)
        };
        let winner_id = win.and_then(|w| copies.iter().find(|c| c.scope == w).map(|c| skill_id(c)));
        for copy in copies {
            let triggers = usage_for(name);
            let source = copy.source.clone().or_else(|| local_source(scan, copy));
            let overridden = copies.len() > 1 && win.is_some() && Some(copy.scope) != win;
            let both = copies.len() > 1 && win.is_none();
            let (state, label, reason) = if overridden {
                let w = win.unwrap_or(Scope::Global);
                (
                    Some(AssetState::Overridden),
                    format!("被{}层同名项覆盖", scope_name(w)),
                    format!(
                        "{tool_name} 遇到同名 skill 时{}。{}层的那一份生效，这一份不加载。",
                        priority_text(priority),
                        scope_name(w)
                    ),
                )
            } else if both {
                (
                    None,
                    format!("同名 {} 份，都会加载", copies.len()),
                    format!(
                        "{tool_name} 遇到同名 skill 不合并，{} 份都出现在可选列表里。名称与描述各占一份常驻。",
                        copies.len()
                    ),
                )
            } else if copies.len() > 1 {
                let others: Vec<&str> = scopes
                    .iter()
                    .filter(|s| Some(**s) != win)
                    .map(|s| scope_name(*s))
                    .collect();
                (
                    None,
                    format!("覆盖{}层同名项", others.join("、")),
                    format!(
                        "{tool_name} 遇到同名 skill 时{}，所以这一份生效。{}",
                        priority_text(priority),
                        trigger_sentence(triggers)
                    ),
                )
            } else if source.is_none() {
                (
                    Some(AssetState::UnknownSource),
                    "来源未知".to_owned(),
                    format!(
                        "{tool_name} 在{}层读到它，但内容哈希与任何已知来源都不匹配。{}",
                        scope_name(copy.scope),
                        trigger_sentence(triggers)
                    ),
                )
            } else {
                (
                    None,
                    trigger_label(triggers, copy.scope),
                    format!(
                        "{tool_name} 在{}层读到它，名称与描述常驻上下文。{}",
                        scope_name(copy.scope),
                        trigger_sentence(triggers)
                    ),
                )
            };
            let id = skill_id(copy);
            rows.push(EffectiveItem {
                overridden_by: if overridden { winner_id.clone() } else { None },
                id,
                name: copy.name.clone(),
                kind: AssetKind::Skill,
                version: Some(short_hash(&copy.hash.0)),
                scope: copy.scope,
                bundle: None,
                state,
                label,
                resident: Some(copy.resident),
                counted: !overridden,
                location: display_path(&copy.dir, home),
                link_target: copy.link_target.as_ref().map(|t| display_path(t, home)),
                source,
                has_scripts: copy.has_scripts,
                members: Vec::new(),
                command: None,
                declared: declarations(&present, if both { Some(copy.scope) } else { win }),
                reason,
                triggers: Some(triggers),
                files: vec![display_path(&copy.dir.join("SKILL.md"), home)],
            });
        }
    }

    for group in group_global_skills(&plain_global) {
        let triggers: u32 = group.members.iter().map(|m| usage_for(&m.name)).sum();
        let first = group.members[0];
        let mut present = BTreeMap::new();
        if group.members.len() == 1 {
            let mut item = single_global(first, triggers, tool_name, home);
            item.source = first.source.clone();
            present.insert(Scope::Global, item.location.clone());
            item.declared = declarations(&present, Some(Scope::Global));
            rows.push(item);
            continue;
        }
        let resident: u32 = group.members.iter().map(|m| m.resident.0).sum();
        let dir = first
            .dir
            .parent()
            .map(|p| display_path(p, home))
            .unwrap_or_default();
        let location = format!("{dir}/{}", group.label);
        present.insert(Scope::Global, location.clone());
        let unused = triggers == 0;
        rows.push(EffectiveItem {
            id: format!("skill-group:{}", group.label),
            name: format!("{} …等 {} 项", first.name, group.members.len()),
            kind: AssetKind::Skill,
            version: None,
            scope: Scope::Global,
            bundle: None,
            state: None,
            label: if unused {
                "本项目从未使用".to_owned()
            } else {
                format!("近 30 天触发 {triggers} 次")
            },
            resident: Some(TokenEstimate(resident)),
            counted: true,
            location,
            link_target: None,
            source: group.source.clone(),
            has_scripts: group.members.iter().any(|m| m.has_scripts),
            members: group.members.iter().map(|m| m.name.clone()).collect(),
            command: None,
            declared: declarations(&present, Some(Scope::Global)),
            reason: format!(
                "这 {} 个 skill 来自 {}，都在全局层，名称与描述常驻每个项目。{}",
                group.members.len(),
                group.source.as_deref().unwrap_or("同一来源"),
                trigger_sentence(triggers)
            ),
            triggers: Some(triggers),
            files: group
                .members
                .iter()
                .map(|m| display_path(&m.dir, home))
                .collect(),
            overridden_by: None,
        });
    }

    rows.sort_by(|a, b| {
        scope_rank(b.scope)
            .cmp(&scope_rank(a.scope))
            .then(b.resident.cmp(&a.resident))
            .then(a.name.cmp(&b.name))
    });
    items.extend(rows);
}

/// 列表里项目层排在前面，全局层在后。
fn scope_rank(scope: Scope) -> u8 {
    match scope {
        Scope::Personal => 3,
        Scope::Project => 2,
        Scope::Bundle => 1,
        Scope::Global => 0,
    }
}

fn skill_id(copy: &SkillCopy) -> String {
    format!("skill:{}:{}", copy.name, copy.dir.display())
}

fn trigger_label(triggers: u32, scope: Scope) -> String {
    if triggers > 0 {
        format!("近 30 天触发 {triggers} 次")
    } else if scope == Scope::Global {
        "本项目从未使用".to_owned()
    } else {
        "近 30 天未触发".to_owned()
    }
}

fn trigger_sentence(triggers: u32) -> String {
    if triggers > 0 {
        format!("近 30 天本项目触发过 {triggers} 次。")
    } else {
        "近 30 天本项目没有触发过。".to_owned()
    }
}

fn single_global(copy: &SkillCopy, triggers: u32, tool_name: &str, home: &Path) -> EffectiveItem {
    let (state, label, reason) = if copy.source.is_none() {
        (
            Some(AssetState::UnknownSource),
            "来源未知".to_owned(),
            format!(
                "{tool_name} 在全局层读到它，但内容哈希与任何已知来源都不匹配。{}",
                trigger_sentence(triggers)
            ),
        )
    } else {
        (
            None,
            trigger_label(triggers, Scope::Global),
            format!(
                "{tool_name} 在全局层读到它，每个项目都会加载它的名称与描述。{}",
                trigger_sentence(triggers)
            ),
        )
    };
    EffectiveItem {
        id: skill_id(copy),
        name: copy.name.clone(),
        kind: AssetKind::Skill,
        version: Some(short_hash(&copy.hash.0)),
        scope: Scope::Global,
        bundle: None,
        state,
        label,
        resident: Some(copy.resident),
        counted: true,
        location: display_path(&copy.dir, home),
        link_target: copy.link_target.as_ref().map(|t| display_path(t, home)),
        source: copy.source.clone(),
        has_scripts: copy.has_scripts,
        members: Vec::new(),
        command: None,
        declared: Vec::new(),
        reason,
        triggers: Some(triggers),
        files: vec![display_path(&copy.dir.join("SKILL.md"), home)],
        overridden_by: None,
    }
}

pub fn mcp_command(entry: &McpEntry) -> Option<String> {
    match (&entry.command, &entry.url) {
        (Some(command), _) => Some(
            std::iter::once(command.as_str())
                .chain(entry.args.iter().map(String::as_str))
                .collect::<Vec<_>>()
                .join(" "),
        ),
        (None, Some(url)) => Some(url.clone()),
        _ => None,
    }
}

fn mcp(
    scan: &Scan,
    adapter: &Adapter,
    global: &Collected,
    project: &Collected,
    project_root: &Path,
    items: &mut Vec<EffectiveItem>,
) {
    let tool = &adapter.tool.id;
    let tool_name = adapter.tool.name.as_str();
    let home = scan.home.as_path();
    let priority = &adapter.precedence.mcp;
    let mut by_name: BTreeMap<&str, Vec<&McpEntry>> = BTreeMap::new();
    for entry in project.mcp.iter().chain(global.mcp.iter()) {
        by_name.entry(entry.name.as_str()).or_default().push(entry);
    }
    let fields_merge = adapter.precedence.mcp_merge == MergeMode::Fields;
    for (name, all) in by_name {
        // 工具不读的那份（例如未受信任项目的配置）单独出一行，不参与谁覆盖谁。
        let (entries, inactive): (Vec<&McpEntry>, Vec<&McpEntry>) =
            all.into_iter().partition(|e| e.inactive.is_none());
        for entry in &inactive {
            let mut present = BTreeMap::new();
            present.insert(
                entry.scope,
                format!("{} · {}", display_path(&entry.file, home), entry.key),
            );
            items.push(EffectiveItem {
                id: mcp_id(entry),
                name: entry.name.clone(),
                kind: AssetKind::Mcp,
                version: Some(short_hash(&entry.hash.0)),
                scope: entry.scope,
                bundle: None,
                state: None,
                label: "不加载".to_owned(),
                resident: None,
                counted: false,
                location: format!("{} · {}", display_path(&entry.file, home), entry.key),
                link_target: None,
                source: None,
                has_scripts: entry.command.is_some(),
                members: Vec::new(),
                command: mcp_command(entry),
                declared: declarations(&present, None),
                reason: format!(
                    "{}。这一条不生效。",
                    entry.inactive.clone().unwrap_or_default()
                ),
                triggers: None,
                files: vec![display_path(&entry.file, home)],
                overridden_by: None,
            });
        }
        if entries.is_empty() {
            continue;
        }
        let scopes: BTreeSet<Scope> = entries.iter().map(|e| e.scope).collect();
        let win = if entries.len() > 1 {
            winner(priority, &scopes)
        } else {
            entries.first().map(|e| e.scope)
        };
        let mut present = BTreeMap::new();
        for entry in &entries {
            present
                .entry(entry.scope)
                .or_insert_with(|| format!("{} · {}", display_path(&entry.file, home), entry.key));
        }
        let calls = scan
            .usage
            .projects_using(tool, AssetKind::Mcp, name)
            .get(project_root)
            .copied()
            .unwrap_or(0);
        let winner_id = win.and_then(|w| entries.iter().find(|e| e.scope == w).map(|e| mcp_id(e)));
        // 按字段合并：从优先级最低的一层开始，逐层用写了的字段盖上去。
        let merged: Option<McpEntry> =
            (fields_merge && entries.len() > 1 && win.is_some()).then(|| {
                let mut ordered = entries.clone();
                ordered.sort_by_key(|e| {
                    std::cmp::Reverse(
                        priority
                            .iter()
                            .position(|s| *s == e.scope)
                            .unwrap_or(usize::MAX),
                    )
                });
                let mut merged = ordered[0].clone();
                for layer in &ordered[1..] {
                    for field in &layer.defines {
                        match *field {
                            "command" => merged.command.clone_from(&layer.command),
                            "args" => merged.args.clone_from(&layer.args),
                            "env" => merged.env.clone_from(&layer.env),
                            "url" => merged.url.clone_from(&layer.url),
                            "enabled" => merged.enabled = layer.enabled,
                            _ => {}
                        }
                    }
                }
                merged
            });
        for entry in &entries {
            let overridden = entries.len() > 1 && win.is_some() && Some(entry.scope) != win;
            let is_winner = merged.is_some() && Some(entry.scope) == win;
            let effective = match (&merged, is_winner) {
                (Some(m), true) => m,
                _ => *entry,
            };
            let secret = entry
                .env
                .iter()
                .any(|(k, v)| crate::scanner::looks_like_secret(k, v));
            let (state, label, reason) = if overridden && fields_merge {
                let w = win.unwrap_or(Scope::Global);
                (
                    Some(AssetState::Overridden),
                    format!("字段被{}层覆盖", scope_name(w)),
                    format!(
                        "{tool_name} 遇到同名 MCP 服务时{}，并按字段合并：{}层写了的字段盖过这一层，没写的沿用这一层。实际启动的是合并后的那一条。",
                        priority_text(priority),
                        scope_name(w)
                    ),
                )
            } else if overridden {
                let w = win.unwrap_or(Scope::Global);
                (
                    Some(AssetState::Overridden),
                    format!("被{}层同名项覆盖", scope_name(w)),
                    format!(
                        "{tool_name} 遇到同名 MCP 服务时{}，整条取用，字段不合并。这一条不生效。",
                        priority_text(priority)
                    ),
                )
            } else if !effective.enabled {
                (
                    None,
                    "已停用".to_owned(),
                    "配置里把它标成了停用，工具不会启动它。".to_owned(),
                )
            } else {
                let label = if secret {
                    "配置里有明文密钥".to_owned()
                } else if calls > 0 {
                    format!("近 30 天调用 {calls} 次")
                } else {
                    "本项目从未调用".to_owned()
                };
                let merge_note = if is_winner {
                    "它和同名的其他层按字段合并，下面的命令是合并后的结果。"
                } else {
                    ""
                };
                (
                    None,
                    label,
                    format!(
                        "{tool_name} 在{}层的 {} 里读到它，每次会话启动这个服务。{merge_note}工具定义要启动服务才读得到，M0 不估算它的常驻。",
                        scope_name(entry.scope),
                        display_path(&entry.file, home)
                    ),
                )
            };
            items.push(EffectiveItem {
                overridden_by: if overridden { winner_id.clone() } else { None },
                id: mcp_id(entry),
                name: entry.name.clone(),
                kind: AssetKind::Mcp,
                version: Some(short_hash(&entry.hash.0)),
                scope: entry.scope,
                bundle: None,
                state,
                label,
                resident: None,
                counted: !overridden && effective.enabled,
                location: format!("{} · {}", display_path(&entry.file, home), entry.key),
                link_target: None,
                source: None,
                has_scripts: entry.command.is_some(),
                members: Vec::new(),
                command: mcp_command(effective),
                declared: declarations(&present, win),
                reason,
                triggers: Some(calls),
                files: vec![display_path(&entry.file, home)],
            });
        }
    }
}

fn mcp_id(entry: &McpEntry) -> String {
    format!(
        "mcp:{}:{}:{}",
        entry.name,
        entry.file.display(),
        kind_key(AssetKind::Mcp)
    )
}

/// 解释一项资产。在生效上下文里按名称找，组里的成员也算。
pub fn explain(context: &EffectiveContext, tool_name: &str, asset: &str) -> Option<Explanation> {
    let item = context
        .items
        .iter()
        .filter(|i| i.name == asset || i.members.iter().any(|m| m == asset))
        .min_by_key(|i| i.state == Some(AssetState::Overridden))?;
    Some(Explanation {
        tool: context.tool.clone(),
        tool_name: tool_name.to_owned(),
        name: asset.to_owned(),
        kind: item.kind,
        state: item.state,
        label: item.label.clone(),
        effective_scope: item.scope,
        declared: item.declared.clone(),
        reason: item.reason.clone(),
    })
}

/// 合并四层声明，解出每项资产的确切版本。
pub fn resolve_lock(_manifest: &Manifest, _local: Option<&Manifest>) -> Result<Lock> {
    Err(Error::NotImplemented("resolver::resolve_lock"))
}
