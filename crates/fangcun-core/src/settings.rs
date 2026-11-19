//! 本机设置 `~/.fangcun/settings.toml`。不提交，只影响这台机器。
//!
//! 只读模式（7.5）和快照保留期（6.7）。界面是「设置…」小窗（6.1，M1-24），现在先能手改这个文件。

use std::fs;

use serde::{Deserialize, Serialize};

use crate::paths::FangcunHome;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Settings {
    /// 只读模式：打开后一切写入用例直接拒绝，和 M0 一样只写 `~/.fangcun/journal`。默认关闭。
    #[serde(default)]
    pub read_only: bool,
    /// 快照保留几天（6.7），默认 30。
    #[serde(default = "default_snapshot_days")]
    pub snapshot_days: u32,
    /// 每个项目（和全局层）至少保留最近几个快照，过期了也留（6.7），默认 20。
    #[serde(default = "default_snapshot_keep")]
    pub snapshot_keep: u32,
}

fn default_snapshot_days() -> u32 {
    30
}

fn default_snapshot_keep() -> u32 {
    20
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            read_only: false,
            snapshot_days: default_snapshot_days(),
            snapshot_keep: default_snapshot_keep(),
        }
    }
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
        assert_eq!(Settings::load(&home).unwrap().snapshot_days, 30);
        fs::write(
            home.settings_file(),
            "snapshot-days = 7\nsnapshot-keep = 3\n",
        )
        .unwrap();
        let settings = Settings::load(&home).unwrap();
        assert_eq!((settings.snapshot_days, settings.snapshot_keep), (7, 3));
        let _ = fs::remove_dir_all(&root);
    }
}
