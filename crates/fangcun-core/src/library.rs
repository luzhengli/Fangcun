//! 资产库（F2、7.2）：本机唯一正本，内容寻址，多版本并存。M1 实现。
//!
//! - 正本在 `~/.fangcun/library/`，索引是 `~/.fangcun/index.sqlite`。
//! - git 来源走 `~/.fangcun/git/` 下的本地裸仓库缓存。
//! - 本地开发目录是活链接，不复制进库。
//!
//! 实现时拆成 `store`（内容寻址）、`index`（SQLite）、`source`（三种来源）三个子模块。

use std::path::Path;

use crate::model::{AssetKind, ContentHash, SourceRef, Version};
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 资产库里的一项资产。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub name: String,
    pub kind: AssetKind,
    /// 来源未知的资产没有来源，等待收编。
    pub source: Option<SourceRef>,
    pub versions: Vec<Version>,
    pub has_scripts: bool,
}

#[derive(Debug)]
pub struct Library {
    _home: FangcunHome,
}

impl Library {
    pub fn open(_home: &FangcunHome) -> Result<Self> {
        Err(Error::NotImplemented("library::Library::open"))
    }

    pub fn entries(&self) -> Result<Vec<LibraryEntry>> {
        Err(Error::NotImplemented("library::Library::entries"))
    }

    /// 按内容哈希取正本所在的目录。
    pub fn locate(&self, _hash: &ContentHash) -> Result<Option<std::path::PathBuf>> {
        Err(Error::NotImplemented("library::Library::locate"))
    }

    /// 收编：把一份副本存进库。原位置的替换由物化器做，不在这里。
    pub fn ingest(&self, _name: &str, _kind: AssetKind, _from: &Path) -> Result<Version> {
        Err(Error::NotImplemented("library::Library::ingest"))
    }

    /// 向来源查有没有新版本。只查不改。
    pub fn check_upstream(&self, _name: &str) -> Result<Option<Version>> {
        Err(Error::NotImplemented("library::Library::check_upstream"))
    }
}
