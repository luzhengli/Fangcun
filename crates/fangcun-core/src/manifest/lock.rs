//! `fangcun.lock`（7.4）：方寸生成，和清单一起提交，不手改。TOML，一项资产一节。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::model::ContentHash;
use crate::{Error, Result};

/// 锁的格式版本。
pub const LOCK_VERSION: u32 = 1;

const HEADER: &str = "# fangcun.lock —— 方寸生成，提交到 git，不要手改\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skills: BTreeMap<String, LockedSkill>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mcp: BTreeMap<String, LockedMcp>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub instructions: BTreeMap<String, LockedInstruction>,
}

impl Default for Lock {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            skills: BTreeMap::new(),
            mcp: BTreeMap::new(),
            instructions: BTreeMap::new(),
        }
    }
}

/// 字段顺序就是写出来的顺序，照 7.4 的示例。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedSkill {
    pub source: String,
    /// 照抄清单里的约束，换了约束就重新解析。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// 本地来源没写版本时为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// skill 在仓库里的目录；在仓库根目录时为空字符串（F2）。
    #[serde(default)]
    pub path: String,
    pub hash: ContentHash,
    /// 来自套装时写套装名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedMcp {
    /// 启动命令、参数、环境变量（密钥只记引用）的哈希。
    pub hash: ContentHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedInstruction {
    pub path: String,
    pub hash: ContentHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
}

impl Lock {
    /// 读项目的锁。没有锁时为 `None`。
    pub fn load(project_root: &Path) -> Result<Option<Self>> {
        let path = project_root.join(crate::paths::LOCK_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Io { path, source }),
        };
        Self::parse(&path.display().to_string(), &text).map(Some)
    }

    pub fn parse(origin: &str, text: &str) -> Result<Self> {
        let lock: Self = toml::from_str(text).map_err(|e| Error::Parse {
            origin: origin.to_owned(),
            message: e.to_string(),
        })?;
        if lock.version != LOCK_VERSION {
            return Err(Error::Parse {
                origin: origin.to_owned(),
                message: format!(
                    "锁的格式版本是 {}，这个方寸只认 {LOCK_VERSION}",
                    lock.version
                ),
            });
        }
        Ok(lock)
    }

    /// 写出来的文本：开头一行说明，键按字母序，同样的锁总是同样的字节。
    pub fn to_toml(&self) -> Result<String> {
        let body = toml::to_string(self).map_err(|e| Error::Parse {
            origin: "fangcun.lock".to_owned(),
            message: e.to_string(),
        })?;
        Ok(format!("{HEADER}{body}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 设计文档 7.4 的锁示例，哈希的省略号换成了完整的值。示例改了，这里跟着改。
    const DESIGN_DOC_EXAMPLE: &str = r#"
# fangcun.lock —— 方寸生成，提交到 git，不要手改
version = 1

[skills.tdd]
source = "github:obra/superpowers"
track  = "main"
commit = "b7e9f0c2"
path   = "skills/tdd"
hash   = "9f2c"
bundle = "工程基线"

[skills.skill-b]
source = "github:acme/skills"
pin    = "v2.1.0"
tag    = "v2.1.0"
commit = "41ad0e27"
hash   = "0d81"

[mcp.github]
hash = "77c3"

[instructions."AGENTS.md"]
path = "AGENTS.md"
hash = "a9e4"
"#;

    #[test]
    fn reads_the_design_doc_example_and_writes_it_back_the_same() {
        let lock = Lock::parse("7.4", DESIGN_DOC_EXAMPLE).unwrap();
        assert_eq!(lock.skills["tdd"].bundle.as_deref(), Some("工程基线"));
        assert_eq!(lock.skills["skill-b"].path, "");
        assert_eq!(lock.instructions["AGENTS.md"].path, "AGENTS.md");
        let text = lock.to_toml().unwrap();
        assert!(text.starts_with(HEADER));
        assert_eq!(Lock::parse("回读", &text).unwrap(), lock);
        assert_eq!(
            Lock::parse("再写", &text).unwrap().to_toml().unwrap(),
            text,
            "同样的锁同样的字节"
        );
    }

    #[test]
    fn refuses_other_versions() {
        assert!(Lock::parse("x", "version = 2\n").is_err());
    }
}
