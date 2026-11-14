//! `fangcun.toml` 的结构，对应设计文档 7.4 的示例。
//!
//! 文档只给了示例，没有给完整规范。示例里出现的键都在这里；
//! 示例之外的键（例如按项目停用、个人覆盖特有的写法）等对应功能设计确认后再加。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{SyncMode, ToolId};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub project: ProjectSection,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skills: BTreeMap<String, SkillDecl>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mcp: BTreeMap<String, McpDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<InstructionsDecl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    /// 目标工具。
    #[serde(default)]
    pub targets: Vec<ToolId>,
    /// 物化方式。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SyncMode>,
    /// 引用的套装，按名称。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bundles: Vec<String>,
}

/// 一项 Skill 的声明。`pin` 与 `track` 二选一，校验放在解析器里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillDecl {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
}

/// 一个 MCP 服务的声明。`env` 的值可以是 `secret:<名称>`，真值在钥匙串里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpDecl {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionsDecl {
    /// 正本。
    pub canonical: String,
    /// 生成为对正本的引用。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirror: Vec<String>,
}

/// 套装文件 `~/.fangcun/bundles/<名称>.toml`。
///
/// 文档定了位置，没有定文件内容。暂时复用清单里的资产三节，
/// 实现简版套装（M1）前要先把格式写进设计文档。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skills: BTreeMap<String, SkillDecl>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mcp: BTreeMap<String, McpDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<InstructionsDecl>,
}
