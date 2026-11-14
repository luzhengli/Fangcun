//! 扫描器（F1、7.2）：遍历、按内容哈希归并、产出体检发现，并读取使用数据。
//!
//! 全程只读。读哪些位置一律从适配器查，这里不写死任何工具的路径。
//! 唯一的例外是「来源仓库」：项目根目录下的 `skills/<名称>/SKILL.md`。
//! 它不是哪个工具的读取位置，而是 skill 仓库的通行写法，体检用它找出存放相同 skill 的仓库。

mod checkup;
mod collect;
pub mod fsutil;
mod usage;

pub use checkup::{
    CheckupReport, Finding, FindingCategory, FindingLocation, FindingStatus, GROUP_MIN,
    GlobalGroup, LocationTone, common_prefix_label, diagnose, format_k, group_global_skills,
    looks_like_secret, short_hash,
};
pub use collect::{
    Collected, InstructionFile, LockInfo, McpEntry, ObservedEntry, Reach, SkillCopy, collect,
    read_locks,
};
pub use usage::{
    ProjectActivity, USAGE_WINDOW_DAYS, UsageKey, UsageStats, collect_usage, project_root,
};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::adapter::{AdapterRegistry, SupportLevel};
use crate::estimator::{estimate_text, skill_listing};
use crate::model::{Scope, Timestamp, ToolId};

/// 来源仓库里放 skill 的目录。
const SOURCE_REPO_SKILL_DIR: &str = "skills";

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// 额外要扫的项目目录，例如命令行的当前目录。会话记录里出现过的项目总是会扫。
    pub project_roots: Vec<PathBuf>,
}

/// 一个被发现的项目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInfo {
    pub root: PathBuf,
    pub name: String,
    /// 近 30 天最后一次用 Agent 的时间。只是额外指定的目录时为空。
    pub last_seen: Option<Timestamp>,
    pub sessions: u32,
    pub tools: Vec<ToolId>,
}

/// 一次全盘扫描的结果。体检、项目页、全局页都读它。
#[derive(Debug, Clone)]
pub struct Scan {
    pub home: PathBuf,
    pub scanned_at: Timestamp,
    /// 完整支持的工具。项目页、全局页、使用数据只看它们。
    pub tools: Vec<ToolId>,
    /// 扫过的全部工具，含只读扫描的。`global`、`project_items` 按它收集。
    pub scanned_tools: Vec<ToolId>,
    /// 有全局 skill 目录的工具。「分发不齐」只比较它们。
    pub skill_tools: std::collections::BTreeSet<ToolId>,
    /// 工具的显示名，取自适配器。
    pub tool_names: BTreeMap<ToolId, String>,
    /// 每个工具的全局层。
    pub global: BTreeMap<ToolId, Collected>,
    /// 发现的项目，最近用过的在前。
    pub projects: Vec<ProjectInfo>,
    /// 项目根目录 → 工具 → 项目层与个人层。
    pub project_items: BTreeMap<PathBuf, BTreeMap<ToolId, Collected>>,
    /// 来源仓库里的 skill。
    pub repo_skills: Vec<SkillCopy>,
    pub locks: Vec<LockInfo>,
    pub usage: UsageStats,
}

impl Scan {
    pub fn tool_name<'a>(&'a self, tool: &'a ToolId) -> &'a str {
        self.tool_names
            .get(tool)
            .map_or(tool.as_str(), String::as_str)
    }

    pub fn project(&self, root: &Path) -> Option<&ProjectInfo> {
        self.projects.iter().find(|p| p.root == root)
    }
}

/// 遍历主目录与发现的项目目录，识别所有工具的 skill、MCP、指令文件。
pub fn scan(adapters: &AdapterRegistry, home: &Path, options: &ScanOptions) -> Scan {
    let scanned_at = Timestamp::now();
    let locks = read_locks(adapters.iter(), home);
    let usage = collect_usage(adapters, home, USAGE_WINDOW_DAYS);

    let mut projects: Vec<ProjectInfo> = usage
        .projects
        .iter()
        .map(|(root, activity)| ProjectInfo {
            root: root.clone(),
            name: project_name(root),
            last_seen: Some(activity.last_seen.clone()),
            sessions: activity.sessions,
            tools: activity.tools.iter().cloned().collect(),
        })
        .collect();
    projects.sort_by(|a, b| b.last_seen.cmp(&a.last_seen).then(a.name.cmp(&b.name)));
    for extra in &options.project_roots {
        let root = project_root(extra, home).unwrap_or_else(|| extra.clone());
        if !projects.iter().any(|p| p.root == root) {
            projects.push(ProjectInfo {
                name: project_name(&root),
                root,
                last_seen: None,
                sessions: 0,
                tools: Vec::new(),
            });
        }
    }

    let full: Vec<_> = adapters
        .iter()
        .filter(|a| a.tool.support == SupportLevel::Full)
        .collect();
    // 只读扫描的工具也收集，体检用；项目页、全局页只看完整支持的工具（`tools`）。
    let mut global = BTreeMap::new();
    for adapter in adapters.iter() {
        global.insert(
            adapter.tool.id.clone(),
            collect(adapter, home, Reach::Global, &locks),
        );
    }

    let mut project_items = BTreeMap::new();
    let mut repo_skills = Vec::new();
    for project in &projects {
        let mut per_tool = BTreeMap::new();
        for adapter in adapters.iter() {
            per_tool.insert(
                adapter.tool.id.clone(),
                collect(adapter, home, Reach::Project(&project.root), &locks),
            );
        }
        project_items.insert(project.root.clone(), per_tool);
        read_repo_skills(&project.root, &mut repo_skills);
    }

    Scan {
        home: home.to_path_buf(),
        scanned_at,
        tools: full.iter().map(|a| a.tool.id.clone()).collect(),
        scanned_tools: adapters.iter().map(|a| a.tool.id.clone()).collect(),
        skill_tools: adapters
            .iter()
            .filter(|a| a.skills.iter().any(|l| l.scope == Scope::Global))
            .map(|a| a.tool.id.clone())
            .collect(),
        tool_names: adapters
            .iter()
            .map(|a| (a.tool.id.clone(), a.tool.name.clone()))
            .collect(),
        global,
        projects,
        project_items,
        repo_skills,
        locks,
        usage,
    }
}

pub fn project_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string())
}

fn read_repo_skills(root: &Path, out: &mut Vec<SkillCopy>) {
    let dir = root.join(SOURCE_REPO_SKILL_DIR);
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(text) = fs::read_to_string(path.join("SKILL.md")) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let description = fsutil::frontmatter_value(&text, "description");
        out.push(SkillCopy {
            tool: ToolId(String::new()),
            scope: Scope::Project,
            resident: estimate_text(&skill_listing(&name, description.as_deref())),
            description,
            hash: fsutil::hash_dir(&path),
            has_scripts: fsutil::has_scripts(&path),
            link_target: fsutil::link_target(&path),
            real_dir: fsutil::canonical(&path),
            dir: path,
            name,
            source: None,
            disabled_by: None,
            project: Some(root.to_path_buf()),
        });
    }
}
