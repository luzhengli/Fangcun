//! 方寸自己的目录与文件名。所有路径拼接集中在这里，其他模块不手写。

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// 项目清单，提交到 git。
pub const MANIFEST_FILE: &str = "fangcun.toml";
/// 版本锁，自动生成，提交到 git。
pub const LOCK_FILE: &str = "fangcun.lock";
/// 个人覆盖，不提交。
pub const LOCAL_MANIFEST_FILE: &str = "fangcun.local.toml";

/// 覆盖 `~/.fangcun` 的环境变量。测试和回归样例用它指向临时目录。
pub const HOME_ENV: &str = "FANGCUN_HOME";

/// `~/.fangcun`：正本、索引、套装、活动记录、快照。
///
/// 设计文档只写定了 `bundles/<名称>.toml` 与 `journal`。其余子目录的名字是
/// Agent 定的（见 DECISIONS 2026-11-14「工程骨架」），实现对应模块时可以改。
#[derive(Debug, Clone)]
pub struct FangcunHome {
    root: PathBuf,
}

impl FangcunHome {
    /// 先看 `FANGCUN_HOME`，没有就用 `~/.fangcun`。只算路径，不创建目录。
    pub fn locate() -> Result<Self> {
        if let Some(root) = std::env::var_os(HOME_ENV) {
            return Ok(Self::at(PathBuf::from(root)));
        }
        let home = std::env::home_dir().ok_or(Error::NoHomeDir)?;
        Ok(Self::at(home.join(".fangcun")))
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 内容寻址的正本。
    pub fn library_dir(&self) -> PathBuf {
        self.root.join("library")
    }

    /// git 来源的本地裸仓库缓存。
    pub fn git_cache_dir(&self) -> PathBuf {
        self.root.join("git")
    }

    /// 资产库的 SQLite 索引。
    pub fn index_db(&self) -> PathBuf {
        self.root.join("index.sqlite")
    }

    pub fn bundles_dir(&self) -> PathBuf {
        self.root.join("bundles")
    }

    pub fn bundle_file(&self, name: &str) -> PathBuf {
        self.bundles_dir().join(format!("{name}.toml"))
    }

    /// 每次写入记一条，活动页读的就是它。
    pub fn journal_dir(&self) -> PathBuf {
        self.root.join("journal")
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    /// 热更新下来的适配器，同名时盖过内置的那份。
    pub fn adapters_dir(&self) -> PathBuf {
        self.root.join("adapters")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_file_follows_the_documented_layout() {
        let home = FangcunHome::at("/tmp/fc");
        assert_eq!(
            home.bundle_file("工程基线"),
            PathBuf::from("/tmp/fc/bundles/工程基线.toml")
        );
    }
}
