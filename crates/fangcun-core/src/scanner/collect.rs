//! 按适配器找出一个工具在某个范围内会读到的全部东西。
//!
//! 体检、生效上下文、全局页都从这里取原料。这里只描述「磁盘上有什么、工具会不会读」，
//! 同名时谁生效、状态是什么，由上层判断。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::fsutil::{
    canonical, frontmatter_value, has_scripts, hash_dir, hash_file, link_target, lookup,
    read_config,
};
use crate::adapter::{
    Adapter, FileFormat, InstructionLocation, LockFormat, McpLocation, ObservedLocation, Pick,
    Requirement, SwitchShape, expand, walk_dirs,
};
use crate::estimator::{estimate_text, skill_listing};
use crate::model::{ContentHash, ObservedKind, Scope, TokenEstimate, ToolId};

/// 磁盘上的一个 skill 目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCopy {
    pub tool: ToolId,
    pub scope: Scope,
    pub name: String,
    /// 工具读到它的位置，可能是软链。
    pub dir: PathBuf,
    /// 跟随软链后的实际位置。多处指向同一个实际位置时只算一份。
    pub real_dir: PathBuf,
    pub link_target: Option<PathBuf>,
    pub hash: ContentHash,
    pub description: Option<String>,
    pub resident: TokenEstimate,
    pub has_scripts: bool,
    /// 锁文件里记的来源，例如 `obra/superpowers`。
    pub source: Option<String>,
    /// 被配置里的开关停用时，写明是哪份配置。
    pub disabled_by: Option<String>,
    /// 项目层时是项目根目录。
    pub project: Option<PathBuf>,
}

/// 一份会被加载的指令文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionFile {
    pub tool: ToolId,
    pub scope: Scope,
    /// 列表里显示的名字：相对项目根目录的路径，或文件名。
    pub name: String,
    pub path: PathBuf,
    pub real_path: PathBuf,
    pub link_target: Option<PathBuf>,
    pub hash: Option<ContentHash>,
    pub bytes: u64,
    pub resident: TokenEstimate,
    /// 因为没有首选文件，退而读取的。例如 Claude Code 没有 CLAUDE.md 时读 AGENTS.md。
    pub fallback: bool,
    /// 只在碰到匹配的文件时才加载，不常驻。例如带 `paths` 的规则。
    pub on_demand: bool,
    /// 超出工具的大小上限，不会加载。
    pub truncated: bool,
    /// 在项目根目录上面的目录里。
    pub above_root: bool,
    /// 整个文件只是一行对另一个文件的引用（`@AGENTS.md`）时，引用的目标。
    pub mirror_of: Option<String>,
    /// 通过 `@路径` 一起带进来的文件。常驻里已经算上它们。
    pub imports: Vec<PathBuf>,
    pub project: Option<PathBuf>,
}

/// 一个 MCP 服务的配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEntry {
    pub tool: ToolId,
    pub scope: Scope,
    pub name: String,
    pub file: PathBuf,
    /// 在配置文件里的键，例如 `mcpServers.github`。
    pub key: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    /// 环境变量的名字与值。值只在内核里用来检查明文密钥，不出内核。
    pub env: Vec<(String, String)>,
    pub url: Option<String>,
    pub enabled: bool,
    /// 被别处的开关停用时，写明是哪份配置，例如 `~/.claude.json`。
    pub disabled_by: Option<String>,
    /// 这一条自己写了哪些字段：command、args、env、url、enabled。按字段合并时用。
    pub defines: Vec<&'static str>,
    /// 工具不读这份配置的原因，例如项目没有标为受信任。为空表示会读。
    pub inactive: Option<String>,
    pub hash: ContentHash,
    pub project: Option<PathBuf>,
}

/// 「其他加载项」里的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedEntry {
    pub tool: ToolId,
    pub kind: ObservedKind,
    pub scope: Scope,
    pub name: String,
    pub note: Option<String>,
    pub location: PathBuf,
}

/// 一个工具在一个范围内读到的全部东西。
#[derive(Debug, Clone, Default)]
pub struct Collected {
    pub skills: Vec<SkillCopy>,
    pub instructions: Vec<InstructionFile>,
    pub mcp: Vec<McpEntry>,
    pub observed: Vec<ObservedEntry>,
    /// 项目层、个人层配置里的 skill 开关，按读到的顺序。全局层的 skill 也要过一遍（按项目停用，7.4）。
    pub project_switches: Vec<SwitchRule>,
    /// 项目层、个人层配置里按名字停用的 MCP 服务，作用于全局层这几份（7.4）。
    pub mcp_off: Vec<McpOff>,
}

/// 一条 skill 开关：按 SKILL.md 的路径或按名字匹配。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchRule {
    pub path: Option<PathBuf>,
    pub name: Option<String>,
    pub enabled: bool,
    /// 哪份配置，例如 `.claude/settings.json`。
    pub label: String,
}

/// 一条按项目停用 MCP 的记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpOff {
    pub name: String,
    pub applies_to: Vec<Scope>,
    pub label: String,
}

impl Collected {
    /// 全局层这份按项目的开关改过之后的样子：项目里停用的全局 skill、全局 MCP 标成停用。
    pub fn with_project_switches(&self, project: &Collected) -> Collected {
        let mut out = self.clone();
        apply_rules(&project.project_switches, &mut out.skills);
        for entry in &mut out.mcp {
            if let Some(off) = project
                .mcp_off
                .iter()
                .find(|o| o.name == entry.name && o.applies_to.contains(&entry.scope))
            {
                entry.enabled = false;
                entry.disabled_by = Some(off.label.clone());
            }
        }
        out
    }
}

/// 一份 skill 锁文件读出来的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockInfo {
    pub path: PathBuf,
    /// 锁管的 skill 目录。
    pub dir: PathBuf,
    /// 名称 → 来源。
    pub sources: BTreeMap<String, String>,
    pub modified: Option<crate::model::Timestamp>,
}

/// 范围：全局层，或某个项目的项目层与个人层。
#[derive(Debug, Clone, Copy)]
pub enum Reach<'a> {
    Global,
    Project(&'a Path),
}

impl Reach<'_> {
    fn includes(self, scope: Scope) -> bool {
        match self {
            Reach::Global => scope == Scope::Global,
            Reach::Project(_) => matches!(scope, Scope::Project | Scope::Personal),
        }
    }
}

/// 读全部适配器声明的锁文件。
pub fn read_locks<'a>(adapters: impl Iterator<Item = &'a Adapter>, home: &Path) -> Vec<LockInfo> {
    let mut locks: Vec<LockInfo> = Vec::new();
    for adapter in adapters {
        for location in &adapter.skills {
            let Some(lock) = &location.lock else { continue };
            let (Some(path), Some(dir)) = (
                expand(&lock.path, home, None),
                expand(&location.path, home, None),
            ) else {
                continue;
            };
            if locks.iter().any(|l| l.path == path) {
                continue;
            }
            let Some(value) = read_config(&path, false) else {
                continue;
            };
            let sources = match lock.format {
                LockFormat::SkillsCli => value
                    .get("skills")
                    .and_then(|s| s.as_object())
                    .map(|skills| {
                        skills
                            .iter()
                            .map(|(name, entry)| {
                                let source = entry
                                    .get("source")
                                    .and_then(|s| s.as_str())
                                    .unwrap_or_default()
                                    .to_owned();
                                (name.clone(), source)
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            let modified = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .map(crate::model::Timestamp::from_system_time);
            locks.push(LockInfo {
                path,
                dir: canonical(&dir),
                sources,
                modified,
            });
        }
    }
    locks
}

/// 找出一个工具在某个范围内读到的全部东西。
pub fn collect(adapter: &Adapter, home: &Path, reach: Reach<'_>, locks: &[LockInfo]) -> Collected {
    let project = match reach {
        Reach::Global => None,
        Reach::Project(root) => Some(root),
    };
    let tool = adapter.tool.id.clone();
    let mut out = Collected::default();

    for location in adapter.skills.iter().filter(|l| reach.includes(l.scope)) {
        let dirs: Vec<PathBuf> = match project {
            None => expand(&location.path, home, None).into_iter().collect(),
            Some(root) => walk_dirs(location.walk, root)
                .into_iter()
                .map(|dir| dir.join(&location.path))
                .collect(),
        };
        for dir in dirs {
            read_skill_dir(
                &dir,
                &tool,
                location.scope,
                location.source.as_deref(),
                project,
                locks,
                &mut out.skills,
            );
        }
    }

    // 全局层的指令文件。项目层往上找时可能走到主目录，碰到同一个文件（例如 ~/.claude/CLAUDE.md），
    // 那份只算在全局层。
    let global_files: Vec<PathBuf> = adapter
        .instructions
        .iter()
        .filter(|l| l.scope == Scope::Global)
        .flat_map(|l| l.names.iter())
        .filter_map(|n| expand(n, home, None))
        .map(|p| canonical(&p))
        .collect();
    for location in adapter
        .instructions
        .iter()
        .filter(|l| reach.includes(l.scope))
    {
        read_instructions(
            location,
            &tool,
            home,
            project,
            &global_files,
            &mut out.instructions,
        );
    }
    for location in adapter.rules.iter().filter(|l| reach.includes(l.scope)) {
        let Some(dir) = expand(&location.dir, home, project) else {
            continue;
        };
        let mut files = Vec::new();
        super::fsutil::list_files(&dir, Path::new(""), &mut files, 0);
        files.sort();
        for rel in files.into_iter().filter(|f| {
            f.extension()
                .is_some_and(|e| e == location.extension.as_str())
        }) {
            let path = dir.join(&rel);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let on_demand = frontmatter_value(&text, "paths").is_some();
            out.instructions.push(InstructionFile {
                tool: tool.clone(),
                scope: location.scope,
                name: format!(
                    "{}/{}",
                    location.dir.trim_start_matches("~/"),
                    rel.display()
                ),
                real_path: canonical(&path),
                link_target: link_target(&path),
                hash: hash_file(&path),
                bytes: text.len() as u64,
                resident: estimate_text(&text),
                fallback: false,
                on_demand,
                truncated: false,
                above_root: false,
                mirror_of: None,
                imports: Vec::new(),
                project: project.map(Path::to_path_buf),
                path,
            });
        }
    }

    for location in adapter.mcp.iter().filter(|l| reach.includes(l.scope)) {
        read_mcp(adapter, location, &tool, home, project, &mut out.mcp);
    }

    for location in adapter.observed.iter().filter(|l| reach.includes(l.scope)) {
        read_observed(location, &tool, home, project, &mut out.observed);
    }

    // 全局层的开关对所有项目生效；项目层、个人层的排在后面，后面的赢。
    let global_rules = switch_rules(adapter, home, project, |scope| scope == Scope::Global);
    apply_rules(&global_rules, &mut out.skills);
    if project.is_some() {
        out.project_switches = switch_rules(adapter, home, project, |scope| scope != Scope::Global);
        apply_rules(&out.project_switches, &mut out.skills);
        out.mcp_off = mcp_off(adapter, home, project);
    }
    out
}

/// 读出一组开关，按适配器里写的顺序。
fn switch_rules(
    adapter: &Adapter,
    home: &Path,
    project: Option<&Path>,
    wanted: impl Fn(Scope) -> bool,
) -> Vec<SwitchRule> {
    let mut rules = Vec::new();
    for switches in adapter.skill_switches.iter().filter(|s| wanted(s.scope)) {
        if switches
            .requires
            .as_ref()
            .is_some_and(|req| !requirement_met(req, home, project))
        {
            continue;
        }
        let Some(file) = expand(&switches.file, home, project) else {
            continue;
        };
        let Some(config) = read_config(&file, switches.format == FileFormat::Toml) else {
            continue;
        };
        let Some(node) = lookup(&config, &switches.key) else {
            continue;
        };
        let label = super::fsutil::display_path(&file, home);
        match switches.shape {
            SwitchShape::List => {
                for entry in node.as_array().into_iter().flatten() {
                    let field = |f: &Option<String>| {
                        f.as_ref()
                            .and_then(|f| entry.get(f))
                            .and_then(|v| v.as_str())
                    };
                    rules.push(SwitchRule {
                        path: field(&switches.path_field).and_then(|p| expand(p, home, project)),
                        name: field(&switches.name_field).map(str::to_owned),
                        enabled: switches
                            .enabled_field
                            .as_ref()
                            .and_then(|f| entry.get(f))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        label: label.clone(),
                    });
                }
            }
            SwitchShape::Map => {
                for (name, value) in node.as_object().into_iter().flatten() {
                    let off = value
                        .as_str()
                        .is_some_and(|v| switches.off_values.iter().any(|o| o == v));
                    rules.push(SwitchRule {
                        path: None,
                        name: Some(name.clone()),
                        enabled: !off,
                        label: label.clone(),
                    });
                }
            }
        }
    }
    rules
}

/// 按顺序把开关用到 skill 上，后面的赢。
fn apply_rules(rules: &[SwitchRule], skills: &mut [SkillCopy]) {
    for skill in skills.iter_mut() {
        let skill_md = skill.dir.join("SKILL.md");
        let real_md = skill.real_dir.join("SKILL.md");
        for rule in rules {
            let by_path = rule
                .path
                .as_ref()
                .is_some_and(|p| *p == skill_md || *p == real_md);
            let by_name = rule.name.as_deref() == Some(skill.name.as_str());
            if by_path || by_name {
                skill.disabled_by = (!rule.enabled).then(|| rule.label.clone());
            }
        }
    }
}

/// 项目里按名字停用的 MCP 服务，例如 Claude Code `~/.claude.json` 里这个项目的 `disabledMcpServers`。
fn mcp_off(adapter: &Adapter, home: &Path, project: Option<&Path>) -> Vec<McpOff> {
    let Some(root) = project else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for switches in &adapter.mcp_switches {
        let Some(file) = expand(&switches.file, home, project) else {
            continue;
        };
        let Some(config) = read_config(&file, switches.format == FileFormat::Toml) else {
            continue;
        };
        let key: Vec<String> = switches
            .key
            .iter()
            .map(|segment| segment.replace("{project}", &root.to_string_lossy()))
            .collect();
        let label = super::fsutil::display_path(&file, home);
        for name in lookup(&config, &key)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str())
        {
            out.push(McpOff {
                name: name.to_owned(),
                applies_to: switches.applies_to.clone(),
                label: label.clone(),
            });
        }
    }
    out
}

/// 找出文本里 `@路径` 引用的文件，递归到 `depth` 层。代码块和反引号里的不算。
fn collect_imports(file: &Path, text: &str, home: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let dir = file.parent().unwrap_or(Path::new("/"));
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut in_code = false;
        let mut prev = ' ';
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '`' {
                in_code = !in_code;
            } else if c == '@' && !in_code && prev.is_whitespace() {
                let token: String = chars[i + 1..]
                    .iter()
                    .take_while(|c| !c.is_whitespace() && **c != '`' && **c != ')' && **c != ',')
                    .collect();
                if token.contains('/') || token.contains('.') {
                    let target = if let Some(rest) = token.strip_prefix("~/") {
                        home.join(rest)
                    } else if token.starts_with('/') {
                        PathBuf::from(&token)
                    } else {
                        dir.join(&token)
                    };
                    if target.is_file() && !out.contains(&target) && target != file {
                        out.push(target.clone());
                        if let Ok(more) = fs::read_to_string(&target) {
                            collect_imports(&target, &more, home, depth - 1, out);
                        }
                    }
                }
                i += token.chars().count();
            }
            prev = c;
            i += 1;
        }
    }
}

fn read_skill_dir(
    dir: &Path,
    tool: &ToolId,
    scope: Scope,
    fallback_source: Option<&str>,
    project: Option<&Path>,
    locks: &[LockInfo],
    out: &mut Vec<SkillCopy>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    let dir_real = canonical(dir);
    for entry in entries {
        let path = entry.path();
        let skill_md = path.join("SKILL.md");
        let Ok(text) = fs::read_to_string(&skill_md) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let description = frontmatter_value(&text, "description");
        let real_dir = canonical(&path);
        let source = locks
            .iter()
            .find_map(|lock| {
                let in_lock_dir =
                    real_dir.parent() == Some(lock.dir.as_path()) || dir_real == lock.dir;
                if in_lock_dir {
                    lock.sources.get(&name).cloned()
                } else {
                    None
                }
            })
            .or_else(|| fallback_source.map(str::to_owned));
        out.push(SkillCopy {
            tool: tool.clone(),
            scope,
            resident: estimate_text(&skill_listing(&name, description.as_deref())),
            description,
            hash: hash_dir(&path),
            has_scripts: has_scripts(&path),
            link_target: link_target(&path),
            real_dir,
            dir: path,
            name,
            source,
            disabled_by: None,
            project: project.map(Path::to_path_buf),
        });
    }
}

fn read_instructions(
    location: &InstructionLocation,
    tool: &ToolId,
    home: &Path,
    project: Option<&Path>,
    global_files: &[PathBuf],
    out: &mut Vec<InstructionFile>,
) {
    let found = |names: &[String]| -> Vec<(PathBuf, String, bool)> {
        let mut found = Vec::new();
        match project {
            None => {
                let candidates: Vec<(PathBuf, String)> = names
                    .iter()
                    .filter_map(|n| expand(n, home, None).map(|p| (p, n.clone())))
                    .collect();
                pick(&candidates, location.pick, &mut found, false);
            }
            Some(root) => {
                for dir in walk_dirs(location.walk, root) {
                    let above = dir != root && root.starts_with(&dir);
                    let candidates: Vec<(PathBuf, String)> = names
                        .iter()
                        .map(|n| {
                            let path = dir.join(n);
                            let label = path
                                .strip_prefix(root)
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|_| super::fsutil::display_path(&path, home));
                            (path, label)
                        })
                        .collect();
                    pick(&candidates, location.pick, &mut found, above);
                }
            }
        }
        found
    };

    let mut names = location.names.clone();
    if let Some(extra) = &location.extra_names
        && let Some(file) = expand(&extra.file, home, project)
        && let Some(config) = read_config(&file, extra.format == FileFormat::Toml)
        && let Some(list) = lookup(&config, &extra.key).and_then(|v| v.as_array())
    {
        names.extend(list.iter().filter_map(|v| v.as_str().map(str::to_owned)));
    }
    let mut files = found(&names);
    let mut fallback = false;
    if files.is_empty() && !location.fallback.is_empty() {
        files = found(&location.fallback);
        fallback = true;
    }

    let mut total: u64 = 0;
    for (path, name, above_root) in files {
        if location.scope != Scope::Global && global_files.contains(&canonical(&path)) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let mut imports = Vec::new();
        let mut imported_text = String::new();
        if location.import_depth > 0 {
            collect_imports(&path, &text, home, location.import_depth, &mut imports);
            for import in &imports {
                if let Ok(more) = fs::read_to_string(import) {
                    imported_text.push_str(&more);
                }
            }
        }
        let bytes = text.len() as u64;
        total += bytes;
        let truncated = location.max_bytes.is_some_and(|max| total > max);
        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let scope = if file_name
            .as_ref()
            .is_some_and(|n| location.personal.iter().any(|p| p.ends_with(n.as_str())))
        {
            Scope::Personal
        } else {
            location.scope
        };
        let trimmed = text.trim();
        let mirror_of = trimmed
            .strip_prefix('@')
            .filter(|rest| !rest.contains(char::is_whitespace))
            .map(str::to_owned);
        let link = link_target(&path);
        out.push(InstructionFile {
            tool: tool.clone(),
            scope,
            name,
            real_path: canonical(&path),
            mirror_of: mirror_of.or_else(|| {
                link.as_ref()
                    .and_then(|l| l.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
            }),
            link_target: link,
            hash: hash_file(&path),
            bytes,
            resident: estimate_text(&format!("{text}{imported_text}")),
            imports,
            fallback,
            on_demand: false,
            truncated,
            above_root,
            project: project.map(Path::to_path_buf),
            path,
        });
    }
}

fn pick(
    candidates: &[(PathBuf, String)],
    mode: Pick,
    found: &mut Vec<(PathBuf, String, bool)>,
    above: bool,
) {
    for (path, label) in candidates {
        let non_empty = fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() > 0);
        if !non_empty {
            continue;
        }
        if found
            .iter()
            .any(|(p, _, _)| canonical(p) == canonical(path))
        {
            continue;
        }
        found.push((path.clone(), label.clone(), above));
        if mode == Pick::First {
            return;
        }
    }
}

fn read_mcp(
    adapter: &Adapter,
    location: &McpLocation,
    tool: &ToolId,
    home: &Path,
    project: Option<&Path>,
    out: &mut Vec<McpEntry>,
) {
    let Some(file) = expand(&location.file, home, project) else {
        return;
    };
    let Some(config) = read_config(&file, location.format == FileFormat::Toml) else {
        return;
    };
    let key: Vec<String> = location
        .key
        .iter()
        .map(|segment| match project {
            Some(root) => segment.replace("{project}", &root.to_string_lossy()),
            None => segment.clone(),
        })
        .collect();
    if key.iter().any(|segment| segment.contains("{project}")) {
        return;
    }
    let Some(servers) = lookup(&config, &key).and_then(|v| v.as_object()) else {
        return;
    };
    let fields = adapter.mcp_fields.as_ref();
    let field = |name: Option<&String>, fallback: &str| -> String {
        name.cloned().unwrap_or_else(|| fallback.to_owned())
    };
    let command_key = field(fields.map(|f| &f.command), "command");
    let args_key = field(fields.map(|f| &f.args), "args");
    let env_key = field(fields.map(|f| &f.env), "env");
    let url_key = field(fields.map(|f| &f.url), "url");
    let enabled_key = fields.and_then(|f| f.enabled.clone());

    let inactive = location
        .requires
        .as_ref()
        .filter(|req| !requirement_met(req, home, project))
        .map(|req| req.unmet.clone());
    let key_label = location.key.last().cloned().unwrap_or_default();
    for (name, server) in servers {
        let mut defines = Vec::new();
        for (role, key) in [
            ("command", &command_key),
            ("args", &args_key),
            ("env", &env_key),
            ("url", &url_key),
        ] {
            if server.get(key).is_some() {
                defines.push(role);
            }
        }
        if enabled_key
            .as_ref()
            .is_some_and(|k| server.get(k).is_some())
        {
            defines.push("enabled");
        }
        let command = server
            .get(&command_key)
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let args = server
            .get(&args_key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let env = server
            .get(&env_key)
            .and_then(|v| v.as_object())
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let url = server
            .get(&url_key)
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let enabled = enabled_key
            .as_ref()
            .and_then(|k| server.get(k))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let canonical_text = serde_json::to_string(server).unwrap_or_default();
        out.push(McpEntry {
            tool: tool.clone(),
            scope: location.scope,
            name: name.clone(),
            file: file.clone(),
            key: format!("{key_label}.{name}"),
            command,
            args,
            env,
            url,
            enabled,
            disabled_by: None,
            defines,
            inactive: inactive.clone(),
            hash: super::fsutil::hash_bytes(canonical_text.as_bytes()),
            project: project.map(Path::to_path_buf),
        });
    }
}

/// 插件带了什么：在安装清单里找到它的安装目录，数 `skills/` 下的 SKILL.md 和它声明的 MCP 服务。
fn plugin_note(
    index: &crate::adapter::InstallIndex,
    plugin: &str,
    home: &Path,
    project: Option<&Path>,
) -> Option<String> {
    let file = expand(&index.file, home, project)?;
    let config = read_config(&file, index.format == FileFormat::Toml)?;
    let entry = lookup(&config, &index.key)?.get(plugin)?;
    let entry = entry.as_array().and_then(|a| a.first()).unwrap_or(entry);
    let install = PathBuf::from(entry.get(&index.path_field)?.as_str()?);
    let skills = fs::read_dir(install.join(&index.skills_dir))
        .map(|dir| {
            dir.flatten()
                .filter(|e| e.path().join("SKILL.md").is_file())
                .count()
        })
        .unwrap_or(0);
    let mcp = plugin_mcp_servers(index, &install).len();
    let mut parts = Vec::new();
    if skills > 0 {
        parts.push(format!("{skills} 个 skill"));
    }
    if mcp > 0 {
        parts.push(format!("{mcp} 个 MCP"));
    }
    (!parts.is_empty()).then(|| format!("含 {}", parts.join("、")))
}

/// 插件声明的 MCP 服务名。先读 `.mcp.json`，再读清单的 `mcpServers`，同名的后者替换前者，所以按名字去重。
/// `.mcpb` 包和网址读不到服务名，按路径各算一个。
fn plugin_mcp_servers(
    index: &crate::adapter::InstallIndex,
    install: &Path,
) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    let add_file = |path: &Path, names: &mut std::collections::BTreeSet<String>| {
        if let Some(value) = read_config(path, false) {
            add_server_map(&value, names);
        }
    };
    if let Some(mcp_file) = &index.mcp_file {
        add_file(&install.join(mcp_file), &mut names);
    }
    let Some(manifest) = index
        .manifest
        .as_ref()
        .and_then(|m| read_config(&install.join(m), false))
    else {
        return names;
    };
    let Some(declared) = manifest.get("mcpServers") else {
        return names;
    };
    let shapes: Vec<&serde_json::Value> = match declared {
        serde_json::Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    for shape in shapes {
        match shape {
            serde_json::Value::Object(map) => {
                names.extend(map.keys().cloned());
            }
            serde_json::Value::String(path) if path.ends_with(".json") => {
                add_file(&install.join(path.trim_start_matches("./")), &mut names);
            }
            serde_json::Value::String(path) => {
                names.insert(path.clone());
            }
            _ => {}
        }
    }
    names
}

/// MCP 配置里的服务名：包在 `mcpServers` 里，或直接放在顶层。
fn add_server_map(value: &serde_json::Value, names: &mut std::collections::BTreeSet<String>) {
    let map = value
        .get("mcpServers")
        .and_then(serde_json::Value::as_object)
        .or_else(|| value.as_object());
    if let Some(map) = map {
        names.extend(
            map.iter()
                .filter(|(_, v)| v.is_object())
                .map(|(k, _)| k.clone()),
        );
    }
}

/// 另一份配置里某个键是否等于要求的值。
pub(crate) fn requirement_met(req: &Requirement, home: &Path, project: Option<&Path>) -> bool {
    let Some(file) = expand(&req.file, home, project) else {
        return false;
    };
    let Some(config) = read_config(&file, req.format == FileFormat::Toml) else {
        return false;
    };
    let key: Vec<String> = req
        .key
        .iter()
        .map(|segment| match project {
            Some(root) => segment.replace("{project}", &root.to_string_lossy()),
            None => segment.clone(),
        })
        .collect();
    lookup(&config, &key).and_then(|v| v.as_str()) == Some(req.equals.as_str())
}

fn read_observed(
    location: &ObservedLocation,
    tool: &ToolId,
    home: &Path,
    project: Option<&Path>,
    out: &mut Vec<ObservedEntry>,
) {
    if let Some(dir) = location
        .dir
        .as_deref()
        .and_then(|d| expand(d, home, project))
    {
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "md") {
                continue;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = match location.kind {
                ObservedKind::SlashCommand => format!("/{stem}"),
                _ => stem,
            };
            out.push(ObservedEntry {
                tool: tool.clone(),
                kind: location.kind,
                scope: location.scope,
                name,
                note: None,
                location: path,
            });
        }
        return;
    }

    let Some(file) = location
        .file
        .as_deref()
        .and_then(|f| expand(f, home, project))
    else {
        return;
    };
    let toml_format = location.format == Some(FileFormat::Toml);
    let Some(config) = read_config(&file, toml_format) else {
        return;
    };
    let Some(object) = lookup(&config, &location.key).and_then(|v| v.as_object()) else {
        return;
    };
    for (key, value) in object {
        match value {
            serde_json::Value::Bool(false) => {}
            serde_json::Value::Array(groups) => {
                // Hook：事件名 → [{ matcher, hooks: [{ command }] }]
                for group in groups {
                    let matcher = group.get("matcher").and_then(|m| m.as_str()).unwrap_or("");
                    let name = if matcher.is_empty() {
                        key.clone()
                    } else {
                        format!("{key} · {matcher}")
                    };
                    let runs_command = group
                        .get("hooks")
                        .and_then(|h| h.as_array())
                        .is_some_and(|h| h.iter().any(|x| x.get("command").is_some()));
                    out.push(ObservedEntry {
                        tool: tool.clone(),
                        kind: location.kind,
                        scope: location.scope,
                        name,
                        note: runs_command.then(|| "会执行脚本".to_owned()),
                        location: file.clone(),
                    });
                }
            }
            _ => out.push(ObservedEntry {
                tool: tool.clone(),
                kind: location.kind,
                scope: location.scope,
                name: key.clone(),
                note: location
                    .install_index
                    .as_ref()
                    .and_then(|index| plugin_note(index, key, home, project)),
                location: file.clone(),
            }),
        }
    }
}
