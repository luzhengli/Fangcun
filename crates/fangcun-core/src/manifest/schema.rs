//! `fangcun.toml` 的结构，对应设计文档 7.4 的示例。
//!
//! 键都来自 7.4：清单示例、`[disable]`、个人覆盖的 `[dev]`、套装的 `[bundle]`。
//! 清单、个人覆盖、套装文件共用 [`Manifest`]；各自能写哪些节，由 `super::load_project` 校验。

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
    /// 按项目停用（7.4）。
    #[serde(default, skip_serializing_if = "DisableSection::is_empty")]
    pub disable: DisableSection,
    /// 开发模式：只许写在个人覆盖里（F2、7.4）。
    #[serde(default, skip_serializing_if = "DevSection::is_empty")]
    pub dev: DevSection,
    /// 套装文件的说明：只许写在套装文件里（7.4）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<BundleSection>,
}

/// `[disable]`：按类型列名字，`*` 通配。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisableSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp: Vec<String>,
}

impl DisableSection {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.mcp.is_empty()
    }

    /// 这一类里的这个名字被停用了没有（名字照写或 `*` 通配）。
    pub fn covers(&self, kind: crate::model::AssetKind, name: &str) -> bool {
        let list = match kind {
            crate::model::AssetKind::Skill => &self.skills,
            crate::model::AssetKind::Mcp => &self.mcp,
            crate::model::AssetKind::Instruction => return false,
        };
        list.iter().any(|p| glob(p, name))
    }
}

/// `[disable]` 的通配：只认 `*`，任意长度的任意字符。
pub fn glob(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if parts.len() == 1 {
        return pattern == name;
    }
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}

/// `[dev]`：在本机走开发模式的 skill。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
}

impl DevSection {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

/// 套装文件的 `[bundle]`。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSection {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
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
    /// 常驻预算，估算的 token 数。不写时 20000（7.4、D-22）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u32>,
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
    /// 仓库里的目录。不写时按 `SKILL.md` 的 `name` 找，找到多个才需要写（F2）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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

/// 套装文件 `~/.fangcun/bundles/<名称>.toml`：与清单同结构，多一节 `[bundle]`（7.4）。
pub type Bundle = Manifest;
