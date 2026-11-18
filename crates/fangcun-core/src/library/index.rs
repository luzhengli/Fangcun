//! 资产库的索引 `~/.fangcun/library/index.toml`：每项资产一节，记来源、各个版本、含不含脚本。
//!
//! 纯文本，能读能审查（4.6「文件即真相」）。改写时拿 `index.lock`，先写临时文件再改名，
//! 桌面应用和命令行同时收编也不会互相覆盖。
//!
//! ```toml
//! format = 1
//!
//! [[asset]]
//! name = "tdd"
//! kind = "skill"
//! source = "github:obra/superpowers"
//! has-scripts = false
//!
//! [[asset.version]]
//! hash = "9f2c…"
//! commit = "b7e9f0c2…"
//! added = "2026-11-19T08:00:00Z"
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::model::{AssetKind, ContentHash, SourceRef, Timestamp};
use crate::{Error, Result};

pub(super) const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct Index {
    pub format: u32,
    #[serde(default, rename = "asset")]
    pub assets: Vec<IndexedAsset>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            format: FORMAT,
            assets: Vec::new(),
        }
    }
}

/// 同一个名字、同一类、同一个来源算同一项资产。来源未知的副本 `source` 为空。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct IndexedAsset {
    pub name: String,
    pub kind: AssetKind,
    pub source: Option<SourceRef>,
    #[serde(default)]
    pub has_scripts: bool,
    #[serde(default, rename = "version")]
    pub versions: Vec<IndexedVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct IndexedVersion {
    pub hash: ContentHash,
    pub commit: Option<String>,
    pub tag: Option<String>,
    pub added: Timestamp,
}

/// 读索引。文件不存在时是空的。
pub(super) fn read(file: &Path) -> Result<Index> {
    let text = match fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Index::default()),
        Err(source) => {
            return Err(Error::Io {
                path: file.to_path_buf(),
                source,
            });
        }
    };
    toml::from_str(&text).map_err(|e| Error::Parse {
        origin: file.display().to_string(),
        message: e.to_string(),
    })
}

/// 拿着锁读、改、写回。
pub(super) fn update<T>(file: &Path, change: impl FnOnce(&mut Index) -> T) -> Result<T> {
    let _lock = Lock::acquire(&file.with_extension("lock"))?;
    let mut index = read(file)?;
    let out = change(&mut index);
    let text = toml::to_string(&index).map_err(|e| Error::Parse {
        origin: "资产库索引".to_owned(),
        message: e.to_string(),
    })?;
    let tmp = file.with_extension(format!("toml.{}.tmp", std::process::id()));
    fs::write(&tmp, text).map_err(io(&tmp))?;
    fs::rename(&tmp, file).map_err(io(file))?;
    Ok(out)
}

/// 文件锁：`create_new` 建得出来就是拿到了。超过 30 秒的锁当作上次崩溃留下的，删掉重来。
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn acquire(path: &Path) -> Result<Self> {
        const STALE: Duration = Duration::from_secs(30);
        for _ in 0..200 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(_) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| SystemTime::now().duration_since(t).ok())
                        .is_some_and(|age| age > STALE);
                    if stale {
                        let _ = fs::remove_file(path);
                    } else {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                }
                Err(source) => {
                    return Err(Error::Io {
                        path: path.to_path_buf(),
                        source,
                    });
                }
            }
        }
        Err(Error::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::TimedOut, "等资产库索引的锁超时"),
        })
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}
