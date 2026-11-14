//! 使用数据（7.2、9.3）：只读本机各工具的会话记录，统计每项资产近 30 天
//! 在哪些项目被触发。数据不上传，也不离开内核进程。
//!
//! 同一份会话记录还回答「最近在哪些项目里用过 Agent」，M0 用它发现项目，
//! 不需要先登记（见 DECISIONS 2026-11-14「M0 实现」）。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::adapter::{AdapterRegistry, SessionFormat, expand};
use crate::model::{AssetKind, Timestamp, ToolId};

/// 统计窗口，天。全局页的状态、建议、快速选择都按它算。
pub const USAGE_WINDOW_DAYS: u32 = 30;

/// 被触发的一项资产。Skill 按目录名，MCP 按服务名。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UsageKey {
    pub tool: ToolId,
    pub kind: AssetKind,
    pub name: String,
}

/// 一个项目在窗口内的 Agent 使用情况。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectActivity {
    pub last_seen: Timestamp,
    pub sessions: u32,
    pub tools: BTreeSet<ToolId>,
}

#[derive(Debug, Clone, Default)]
pub struct UsageStats {
    /// 资产 → 项目根目录 → 触发次数。
    pub by_asset: BTreeMap<UsageKey, BTreeMap<PathBuf, u32>>,
    /// 窗口内用过 Agent 的项目，按项目根目录。
    pub projects: BTreeMap<PathBuf, ProjectActivity>,
    pub sessions_read: u32,
}

impl UsageStats {
    /// 某项资产在窗口内被哪些项目触发过。
    pub fn projects_using(
        &self,
        tool: &ToolId,
        kind: AssetKind,
        name: &str,
    ) -> BTreeMap<PathBuf, u32> {
        self.by_asset
            .get(&UsageKey {
                tool: tool.clone(),
                kind,
                name: name.to_owned(),
            })
            .cloned()
            .unwrap_or_default()
    }
}

/// 读全部适配器声明的会话记录。
pub fn collect_usage(adapters: &AdapterRegistry, home: &Path, window_days: u32) -> UsageStats {
    let cutoff = Timestamp::days_ago(window_days);
    let mut stats = UsageStats::default();
    let mut roots = RootCache::new(home);
    for adapter in adapters.iter() {
        let Some(source) = &adapter.sessions else {
            continue;
        };
        let Some(dir) = expand(&source.dir, home, None) else {
            continue;
        };
        let mut files = Vec::new();
        recent_jsonl(&dir, &cutoff, &mut files, 0);
        files.sort();
        for file in files {
            let tool = &adapter.tool.id;
            let read = match source.format {
                SessionFormat::ClaudeCodeJsonl => {
                    read_claude(&file, tool, &cutoff, &mut roots, &mut stats)
                }
                SessionFormat::CodexRolloutJsonl => {
                    read_codex(&file, tool, &cutoff, &mut roots, &mut stats)
                }
            };
            if read {
                stats.sessions_read += 1;
            }
        }
    }
    stats
}

/// 窗口内改过的 `.jsonl` 文件。
fn recent_jsonl(dir: &Path, cutoff: &Timestamp, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            recent_jsonl(&path, cutoff, out, depth + 1);
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            let recent = meta
                .modified()
                .map(|m| Timestamp::from_system_time(m) >= *cutoff)
                .unwrap_or(false);
            if recent {
                out.push(path);
            }
        }
    }
}

/// 工作目录 → 项目根目录。往上找 `.git`，找不到就用工作目录本身。
/// 主目录本身、临时目录、已经不存在的目录不算项目。
struct RootCache<'a> {
    home: &'a Path,
    cache: HashMap<String, Option<PathBuf>>,
}

impl<'a> RootCache<'a> {
    fn new(home: &'a Path) -> Self {
        Self {
            home,
            cache: HashMap::new(),
        }
    }

    fn root(&mut self, cwd: &str) -> Option<PathBuf> {
        if let Some(hit) = self.cache.get(cwd) {
            return hit.clone();
        }
        let root = project_root(Path::new(cwd), self.home);
        self.cache.insert(cwd.to_owned(), root.clone());
        root
    }
}

const TEMP_PREFIXES: &[&str] = &["/tmp", "/private/", "/var/folders"];

pub fn project_root(cwd: &Path, home: &Path) -> Option<PathBuf> {
    if cwd == home || !cwd.is_dir() {
        return None;
    }
    // 主目录里的路径不算临时目录，哪怕主目录本身在临时目录下（测试就是这样）。
    let text = cwd.to_string_lossy();
    if !cwd.starts_with(home) && TEMP_PREFIXES.iter().any(|p| text.starts_with(p)) {
        return None;
    }
    // 方寸自己和各工具的配置目录不算项目。
    if let Ok(rest) = cwd.strip_prefix(home)
        && rest
            .components()
            .next()
            .is_some_and(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return None;
    }
    let git_root = cwd
        .ancestors()
        .take_while(|dir| *dir != home && dir.parent().is_some())
        .find(|dir| dir.join(".git").exists());
    Some(git_root.unwrap_or(cwd).to_path_buf())
}

/// 从一行 JSON 里取某个字符串字段的值，不做完整解析。只用在不含转义的字段上。
fn quick_field<'l>(line: &'l str, field: &str) -> Option<&'l str> {
    let pattern = format!("\"{field}\":\"");
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    Some(&line[start..end])
}

fn note_session(stats: &mut UsageStats, root: &Path, tool: &ToolId, at: &str) {
    let entry = stats
        .projects
        .entry(root.to_path_buf())
        .or_insert_with(|| ProjectActivity {
            last_seen: Timestamp(at.to_owned()),
            sessions: 0,
            tools: BTreeSet::new(),
        });
    entry.sessions += 1;
    entry.tools.insert(tool.clone());
    if at > entry.last_seen.as_str() {
        entry.last_seen = Timestamp(at.to_owned());
    }
}

fn bump(stats: &mut UsageStats, tool: &ToolId, kind: AssetKind, name: &str, root: &Path) {
    let key = UsageKey {
        tool: tool.clone(),
        kind,
        name: name.to_owned(),
    };
    *stats
        .by_asset
        .entry(key)
        .or_default()
        .entry(root.to_path_buf())
        .or_insert(0) += 1;
}

/// `mcp__<服务>__<工具>` → 服务名。
fn mcp_server(tool_name: &str) -> Option<&str> {
    let rest = tool_name.strip_prefix("mcp__")?;
    rest.rsplit_once("__").map(|(server, _)| server)
}

/// Claude Code：每行带 `cwd` 与 `timestamp`。
fn read_claude(
    file: &Path,
    tool: &ToolId,
    cutoff: &Timestamp,
    roots: &mut RootCache<'_>,
    stats: &mut UsageStats,
) -> bool {
    let Ok(handle) = File::open(file) else {
        return false;
    };
    let mut session_root: Option<PathBuf> = None;
    let mut last_at = String::new();
    let mut seen_projects: BTreeSet<PathBuf> = BTreeSet::new();
    for line in BufReader::new(handle).lines().map_while(Result::ok) {
        let Some(at) = quick_field(&line, "timestamp") else {
            continue;
        };
        if at < cutoff.as_str() {
            continue;
        }
        let Some(root) = quick_field(&line, "cwd").and_then(|cwd| roots.root(cwd)) else {
            continue;
        };
        last_at = at.to_owned();
        seen_projects.insert(root.clone());
        session_root = Some(root.clone());

        let interesting = line.contains("\"name\":\"Skill\"")
            || line.contains("\"name\":\"mcp__")
            || line.contains("<command-name>/");
        if !interesting {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(content) = value.pointer("/message/content") else {
            continue;
        };
        match content {
            serde_json::Value::Array(blocks) => {
                for block in blocks {
                    if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                        continue;
                    }
                    let name = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    if name == "Skill" {
                        if let Some(skill) = block.pointer("/input/skill").and_then(|s| s.as_str())
                        {
                            bump(
                                stats,
                                tool,
                                AssetKind::Skill,
                                skill.trim_start_matches('/'),
                                &root,
                            );
                        }
                    } else if let Some(server) = mcp_server(name) {
                        bump(stats, tool, AssetKind::Mcp, server, &root);
                    }
                }
            }
            serde_json::Value::String(text) => {
                // 用户输入 `/<skill>` 直接调用。
                if let Some(start) = text.find("<command-name>/") {
                    let rest = &text[start + "<command-name>/".len()..];
                    if let Some(end) = rest.find('<') {
                        bump(stats, tool, AssetKind::Skill, &rest[..end], &root);
                    }
                }
            }
            _ => {}
        }
    }
    for root in &seen_projects {
        note_session(stats, root, tool, &last_at);
    }
    session_root.is_some()
}

/// Codex：`session_meta`、`turn_context` 带 `cwd`，工具调用在 `response_item` 里。
fn read_codex(
    file: &Path,
    tool: &ToolId,
    cutoff: &Timestamp,
    roots: &mut RootCache<'_>,
    stats: &mut UsageStats,
) -> bool {
    let Ok(handle) = File::open(file) else {
        return false;
    };
    let mut root: Option<PathBuf> = None;
    let mut last_at = String::new();
    let mut seen_projects: BTreeSet<PathBuf> = BTreeSet::new();
    for line in BufReader::new(handle).lines().map_while(Result::ok) {
        let at = quick_field(&line, "timestamp").unwrap_or("").to_owned();
        if line.contains("\"type\":\"session_meta\"") || line.contains("\"type\":\"turn_context\"")
        {
            if let Some(cwd) = quick_field(&line, "cwd") {
                root = roots.root(cwd);
            }
            continue;
        }
        let Some(current) = root.clone() else {
            continue;
        };
        if at.as_str() < cutoff.as_str() {
            continue;
        }
        last_at.clone_from(&at);
        seen_projects.insert(current.clone());

        let has_skill = line.contains("SKILL.md");
        let has_mcp = line.contains("mcp__") || line.contains("\"mcp_tool_call_end\"");
        if !has_skill && !has_mcp {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let kind = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match kind {
            "function_call" | "custom_tool_call" | "local_shell_call" => {
                let name = payload.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if let Some(server) = mcp_server(name) {
                    bump(stats, tool, AssetKind::Mcp, server, &current);
                }
                if has_skill {
                    let text = payload
                        .get("input")
                        .or_else(|| payload.get("arguments"))
                        .or_else(|| payload.get("action"))
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default();
                    for skill in skills_read(&text) {
                        bump(stats, tool, AssetKind::Skill, &skill, &current);
                    }
                }
            }
            "mcp_tool_call_end" => {
                if let Some(server) = payload
                    .pointer("/invocation/server")
                    .and_then(|s| s.as_str())
                {
                    bump(stats, tool, AssetKind::Mcp, server, &current);
                }
            }
            _ => {}
        }
    }
    for project in &seen_projects {
        note_session(stats, project, tool, &last_at);
    }
    !seen_projects.is_empty()
}

/// 命令文本里读到的 `…/skills/<名称>/SKILL.md`。插件缓存里的 skill 不算，它们不在全局层。
fn skills_read(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(pos) = rest.find("/SKILL.md") {
        let before = &rest[..pos];
        let start = before
            .rfind(|c: char| c == '/' || c.is_whitespace() || c == '"' || c == '\'')
            .map_or(0, |i| i + 1);
        let name = &before[start..];
        let path_start = before
            .rfind(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map_or(0, |i| i + 1);
        let path = &before[path_start..];
        if !name.is_empty() && !path.contains("/plugins/") {
            out.insert(name.to_owned());
        }
        rest = &rest[pos + "/SKILL.md".len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_names_split_at_the_last_double_underscore() {
        assert_eq!(
            mcp_server("mcp__claude_ai_Claude_Docs__update"),
            Some("claude_ai_Claude_Docs")
        );
        assert_eq!(mcp_server("mcp__github__create_issue"), Some("github"));
        assert_eq!(mcp_server("Read"), None);
    }

    #[test]
    fn finds_skills_read_by_a_command() {
        let text = "cat /Users/me/.agents/skills/tdd/SKILL.md && cat ~/.codex/plugins/cache/x/skills/pdf/SKILL.md";
        let found: Vec<String> = skills_read(text).into_iter().collect();
        assert_eq!(found, ["tdd"]);
    }

    #[test]
    fn quick_field_reads_plain_string_values() {
        let line = r#"{"timestamp":"2026-11-14T06:26:27.740Z","cwd":"/Users/me/p"}"#;
        assert_eq!(quick_field(line, "cwd"), Some("/Users/me/p"));
        assert_eq!(quick_field(line, "missing"), None);
    }
}
