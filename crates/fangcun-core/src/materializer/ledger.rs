//! 物化台账（7.4「不进锁的」、M1-pre-01）：`~/.fangcun/state/`。
//!
//! 每个已登记项目一份 `state/projects/<键>.toml`，记方寸在这个项目里写过的路径和配置键。
//! 同步按它判断哪些是自己的（只动自己的东西，4.6 第 5 条）、清单里去掉的该删哪些；
//! 漂移检测（M1-15）按它找受管副本。收编换过的位置记在 `state/adopted.toml`。
//!
//! 台账是方寸自己的文件，随变更计划一起写，所以回滚时一起还原。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::SyncMode;
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 一个项目的台账。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Ledger {
    pub project: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<SyncMode>,
    #[serde(default, rename = "path", skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<OwnedPath>,
    #[serde(default, rename = "key", skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<OwnedKey>,
}

/// 方寸写过的一个路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OwnedPath {
    /// 相对项目根目录，用 `/` 连接。
    pub path: String,
    pub asset: String,
    pub kind: PathKind,
    /// 软链的目标。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 实体目录或镜像文件的内容哈希。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathKind {
    /// `.agents/skills/<名称>`：链接模式是指向资产库的软链，开发模式指向本地来源目录。
    SkillLink,
    /// `.agents/skills/<名称>`：入库模式的实体目录。
    SkillDir,
    /// 其他工具目录里指向 `.agents/skills/<名称>` 的相对软链。
    ToolLink,
    /// 指令镜像，例如内容是 `@AGENTS.md` 的 `CLAUDE.md`。
    Mirror,
}

/// 方寸写过的一个配置键，例如 `.mcp.json` 的 `mcpServers.github`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct OwnedKey {
    /// 相对项目根目录。
    pub file: String,
    /// 服务表的键路径加服务名，用 `.` 连接。
    pub key: String,
    pub asset: String,
}

impl Ledger {
    pub fn file(home: &FangcunHome, root: &Path) -> PathBuf {
        let key = super::snapshot::sha256_hex(root.to_string_lossy().as_bytes());
        home.state_dir()
            .join("projects")
            .join(format!("{}.toml", &key[..16]))
    }

    /// 没有台账时是空的。
    pub fn load(home: &FangcunHome, root: &Path) -> Result<Self> {
        let file = Self::file(home, root);
        match fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Parse {
                origin: file.display().to_string(),
                message: e.to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                project: root.to_path_buf(),
                ..Self::default()
            }),
            Err(source) => Err(Error::Io { path: file, source }),
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string(self).map_err(|e| Error::Parse {
            origin: "物化台账".to_owned(),
            message: e.to_string(),
        })
    }

    pub fn path(&self, rel: &str) -> Option<&OwnedPath> {
        self.paths.iter().find(|p| p.path == rel)
    }

    pub fn key(&self, file: &str, key: &str) -> Option<&OwnedKey> {
        self.keys.iter().find(|k| k.file == file && k.key == key)
    }
}

/// 收编换过的位置（M1-12）：原来是一份副本，现在是指向资产库的软链。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adopted {
    #[serde(default, rename = "link", skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<AdoptedLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AdoptedLink {
    /// 绝对路径。
    pub path: PathBuf,
    pub asset: String,
    pub hash: String,
}

impl Adopted {
    pub fn file(home: &FangcunHome) -> PathBuf {
        home.state_dir().join("adopted.toml")
    }

    pub fn load(home: &FangcunHome) -> Result<Self> {
        let file = Self::file(home);
        match fs::read_to_string(&file) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Parse {
                origin: file.display().to_string(),
                message: e.to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(Error::Io { path: file, source }),
        }
    }

    pub fn to_toml(&self) -> Result<String> {
        toml::to_string(self).map_err(|e| Error::Parse {
            origin: "收编台账".to_owned(),
            message: e.to_string(),
        })
    }
}
