use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// MVP 管理的三类资产。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum AssetKind {
    Skill,
    Mcp,
    Instruction,
}

/// 「其他加载项」：MVP 只看不管，逐行展示，不提供操作，不计入预算（F5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ObservedKind {
    Plugin,
    Hook,
    SlashCommand,
    Subagent,
}

/// Agent 工具的标识，与清单里 `targets` 的写法一致，例如 `claude-code`、`codex`。
///
/// 是字符串而不是枚举：适配器是数据，新增工具不需要改内核（7.3）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ToolId(pub String);

impl ToolId {
    pub const CLAUDE_CODE: &'static str = "claude-code";
    pub const CODEX: &'static str = "codex";

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ToolId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// 项目的标识：项目根目录的绝对路径。M0 没有登记，项目从会话记录里发现。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProjectId(pub String);

/// 快照编号，界面上写作「快照 #127」。按范围（每个项目、全局层）各自编号，
/// 所以要和记录里的项目一起才能唯一确定一个快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SnapshotId(pub u32);

/// 资产的出处，清单里的原样写法，例如 `github:obra/superpowers`、`local:luyao-skills`。
///
/// 拆成结构化的来源类型放在 `library::source` 里做。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SourceRef(pub String);
