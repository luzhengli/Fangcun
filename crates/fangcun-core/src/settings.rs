//! 本机设置 `~/.fangcun/settings.toml`。不提交，只影响这台机器。
//!
//! 现在只有只读模式（7.5）。设置页的界面文档还没写，先能手改这个文件。

use std::fs;

use serde::{Deserialize, Serialize};

use crate::paths::FangcunHome;
use crate::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Settings {
    /// 只读模式：打开后一切写入用例直接拒绝，和 M0 一样只写 `~/.fangcun/journal`。默认关闭。
    #[serde(default)]
    pub read_only: bool,
}

impl Settings {
    /// 文件不存在时用默认值。
    pub fn load(home: &FangcunHome) -> Result<Self> {
        let file = home.settings_file();
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(Error::Io { path: file, source }),
        };
        toml::from_str(&text).map_err(|e| Error::Parse {
            origin: file.display().to_string(),
            message: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_writable_and_the_switch_is_read() {
        let root = std::env::temp_dir().join(format!("fangcun-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = FangcunHome::at(&root);
        assert!(!Settings::load(&home).unwrap().read_only);
        fs::create_dir_all(&root).unwrap();
        fs::write(home.settings_file(), "read-only = true\n").unwrap();
        assert!(Settings::load(&home).unwrap().read_only);
        let _ = fs::remove_dir_all(&root);
    }
}
