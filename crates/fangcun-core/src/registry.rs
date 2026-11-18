//! 登记表 `~/.fangcun/projects.toml`（F3、M1-pre-06）：已登记的项目，每个一节。
//!
//! M1-04 只读它，用来算写入范围。登记（写这个文件）在 M1-09。
//!
//! ```toml
//! [[project]]
//! path = "/Users/me/Project/evals_studio"
//! registered = "2026-11-19T08:00:00Z"
//! ```

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::Timestamp;
use crate::paths::FangcunHome;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RegisteredProject {
    /// 项目根目录的绝对路径。
    pub path: PathBuf,
    pub registered: Timestamp,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    project: Vec<RegisteredProject>,
}

/// 文件不存在时是空的。
pub fn load(home: &FangcunHome) -> Result<Vec<RegisteredProject>> {
    let file = home.projects_file();
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(Error::Io { path: file, source }),
    };
    let parsed: File = toml::from_str(&text).map_err(|e| Error::Parse {
        origin: file.display().to_string(),
        message: e.to_string(),
    })?;
    Ok(parsed.project)
}
