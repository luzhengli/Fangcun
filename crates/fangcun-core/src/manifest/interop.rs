//! 与 `skills-lock.json` 互通（F3、9.2）。M1 实现。

use std::path::Path;

use super::{Lock, Manifest};
use crate::{Error, Result};

/// 读一份 `skills-lock.json`，转成清单与锁。
pub fn import_skills_lock(_path: &Path) -> Result<(Manifest, Lock)> {
    Err(Error::NotImplemented("manifest::import_skills_lock"))
}

/// 把清单与锁里的 Skill 部分导出成 `skills-lock.json` 的文本。
pub fn export_skills_lock(_manifest: &Manifest, _lock: &Lock) -> Result<String> {
    Err(Error::NotImplemented("manifest::export_skills_lock"))
}
