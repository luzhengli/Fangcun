//! 适配器 TOML 的结构。格式说明在 `adapters/README.md`，两处一起改。
//!
//! 路径写法：
//! - 以 `~/` 开头的是主目录下的绝对位置，全局层用。
//! - 以 `/` 开头的是绝对路径。
//! - 其余是相对项目根目录的位置，项目层和个人层用。

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{ObservedKind, Scope, ToolId};

/// 当前内核能读的描述格式版本。
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Adapter {
    pub schema: u32,
    pub tool: ToolInfo,
    /// 没写表示还没核实。
    pub capabilities: Option<Capabilities>,
    /// 读 skill 的位置。每个位置是一个目录，下面每个子目录是一个 skill。
    #[serde(default)]
    pub skills: Vec<SkillLocation>,
    /// 读指令文件的位置。
    #[serde(default)]
    pub instructions: Vec<InstructionLocation>,
    /// 规则目录：目录下的 `.md` 全部加载，带 `paths` 的按需加载。
    #[serde(default)]
    pub rules: Vec<RulesLocation>,
    /// 读 MCP 配置的位置。
    #[serde(default)]
    pub mcp: Vec<McpLocation>,
    /// MCP 配置里的字段名。
    pub mcp_fields: Option<McpFields>,
    /// 同名时的优先级规则。
    #[serde(default)]
    pub precedence: Precedence,
    /// 「其他加载项」：插件、Hook、斜杠命令、子代理。只看不管。
    #[serde(default)]
    pub observed: Vec<ObservedLocation>,
    /// 会话记录，用来统计触发次数和发现项目。
    pub sessions: Option<SessionSource>,
    /// 路径里的变量：`$名称/…` 先取同名环境变量，没有就用这里的默认值。例如 `CODEX_HOME`。
    #[serde(default)]
    pub path_vars: std::collections::BTreeMap<String, String>,
    /// 配置里按名称或路径停用 skill 的开关，例如 Codex 的 `[[skills.config]]`。
    #[serde(default)]
    pub skill_switches: Vec<SkillSwitches>,
}

/// 一份配置里的 skill 开关表：数组里每一项按 `path`（SKILL.md 的绝对路径）或 `name` 匹配，
/// 带一个布尔开关。多项匹配时后面的赢。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SkillSwitches {
    pub scope: Scope,
    pub file: String,
    pub format: FileFormat,
    pub key: Vec<String>,
    pub path_field: String,
    pub name_field: String,
    pub enabled_field: String,
    pub requires: Option<Requirement>,
}

/// 另一份配置里的一个字符串数组，例如 Codex 的 `project_doc_fallback_filenames`。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ConfigList {
    pub file: String,
    pub format: FileFormat,
    pub key: Vec<String>,
}

/// 插件的安装清单：键路径下的对象，每个插件一项，项里（或项的数组首个元素里）有安装目录。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct InstallIndex {
    pub file: String,
    pub format: FileFormat,
    pub key: Vec<String>,
    pub path_field: String,
    /// 安装目录下放 skill 的子目录。
    pub skills_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct ToolInfo {
    pub id: ToolId,
    pub name: String,
    pub support: SupportLevel,
}

/// 首批完整支持 Claude Code 与 Codex，其余工具在 M0 只读扫描（9.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum SupportLevel {
    Full,
    ScanOnly,
}

/// 能力标记。没写的一项表示还没核实，界面按「未知」处理，不静默失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Capabilities {
    /// 是否跟随软链。不跟随时物化退回「复制加哈希跟踪」（9.1）。
    pub follows_symlinks: Option<bool>,
    /// 是否支持项目级 MCP。
    pub project_mcp: Option<bool>,
    /// 是否支持按项目停用。
    pub per_project_disable: Option<bool>,
}

/// 项目层的位置从哪些目录找。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Walk {
    /// 只看项目根目录。
    #[default]
    Root,
    /// 项目根目录和它上面的每一层，直到文件系统根。从上往下排。
    Ancestors,
    /// 从 git 根目录往下走到项目根目录。项目根就是 git 根时只有一层。
    GitRootDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SkillLocation {
    pub scope: Scope,
    pub path: String,
    #[serde(default)]
    pub walk: Walk,
    /// 这个目录配套的锁文件，用来查来源、发现「脱节」。
    pub lock: Option<LockRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LockRef {
    pub path: String,
    pub format: LockFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockFormat {
    /// `skills` 命令行的锁：`{ "skills": { 名称: { "source": … } } }`。
    SkillsCli,
}

/// 每个目录里取哪些文件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pick {
    /// 列出的文件都读。
    #[default]
    All,
    /// 按顺序取第一个非空的。
    First,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct InstructionLocation {
    pub scope: Scope,
    /// 文件名。全局层写完整位置。
    pub names: Vec<String>,
    #[serde(default)]
    pub walk: Walk,
    #[serde(default)]
    pub pick: Pick,
    /// 所有目录里都没找到 `names` 时，改读这些文件名。
    #[serde(default)]
    pub fallback: Vec<String>,
    /// 写在这里的文件名属于个人层，例如不提交的 `CLAUDE.local.md`。
    #[serde(default)]
    pub personal: Vec<String>,
    /// 合并后的上限，超出的部分工具不加载。
    pub max_bytes: Option<u64>,
    /// 追加到 `names` 后面的文件名，从另一份配置里读。
    pub extra_names: Option<ConfigList>,
    /// 文件里的 `@路径` 会把那个文件一起带进来，最多跟几层。0 表示不展开。
    #[serde(default)]
    pub import_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RulesLocation {
    pub scope: Scope,
    pub dir: String,
    /// 规则文件的扩展名，默认 `md`。Cursor 是 `mdc`。
    #[serde(default = "default_rule_extension")]
    pub extension: String,
}

fn default_rule_extension() -> String {
    "md".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileFormat {
    Json,
    Toml,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct McpLocation {
    pub scope: Scope,
    pub file: String,
    pub format: FileFormat,
    /// 服务表所在的键路径。`{project}` 换成项目根目录的绝对路径。
    pub key: Vec<String>,
    /// 工具只在满足这个条件时才读这份配置，例如 Codex 只读受信任项目的配置。
    pub requires: Option<Requirement>,
}

/// 另一份配置里某个键等于某个值。键路径里的 `{project}` 换成项目根目录。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Requirement {
    pub file: String,
    pub format: FileFormat,
    pub key: Vec<String>,
    pub equals: String,
    /// 不满足时界面上的说明。
    pub unmet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct McpFields {
    pub command: String,
    pub args: String,
    pub env: String,
    pub url: String,
    /// 停用开关的字段名，值为 `false` 时不加载。没有这个开关的工具不写。
    pub enabled: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Precedence {
    /// skill 同名时谁生效，排在前面的层赢。空表示都加载。
    #[serde(default)]
    pub skills: Vec<Scope>,
    /// MCP 服务同名时谁生效，排在前面的层赢。空表示都加载。
    #[serde(default)]
    pub mcp: Vec<Scope>,
    /// MCP 同名时整条取用，还是按字段合并（赢的那层有的字段盖过输的那层）。
    #[serde(default)]
    pub mcp_merge: MergeMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MergeMode {
    /// 整条取用，字段不跨层合并。
    #[default]
    Replace,
    /// 按字段合并。
    Fields,
}

/// 「其他加载项」的一处来源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ObservedLocation {
    pub kind: ObservedKind,
    pub scope: Scope,
    /// 目录：每个 `.md` 文件一项。
    pub dir: Option<String>,
    /// 配置文件：键路径下的对象，每个键一项。
    pub file: Option<String>,
    pub format: Option<FileFormat>,
    #[serde(default)]
    pub key: Vec<String>,
    /// 插件：从安装清单里找每个插件带的 skill，写进说明。
    pub install_index: Option<InstallIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SessionSource {
    pub dir: String,
    pub format: SessionFormat,
}

/// 会话记录的格式。解析写在 `scanner::usage` 里，每种格式一个函数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionFormat {
    /// `~/.claude/projects/<目录>/<会话>.jsonl`：每行带 `cwd`；
    /// skill 触发是 `tool_use` 的 `Skill`，MCP 调用是 `mcp__<服务>__<工具>`。
    ClaudeCodeJsonl,
    /// `~/.codex/sessions/<年>/<月>/<日>/rollout-*.jsonl`：`session_meta` 带 `cwd`；
    /// skill 触发是命令里读了 `<名称>/SKILL.md`，MCP 调用是 `mcp__<服务>__<工具>`。
    CodexRolloutJsonl,
}
