//! 本机设置 `~/.fangcun/settings.toml`。不提交，只影响这台机器。
//!
//! 只读模式（7.5）和快照保留期（6.7）。界面在主窗口的「设置」页（6.1，M1-24），也可以手改这个文件。

use std::fs;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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

/// 设置页的表单：和 [`Settings`] 一样的三项，字段名按界面的写法（IPC 用 camelCase，文件用 kebab-case）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SettingsForm {
    pub read_only: bool,
    pub snapshot_days: u32,
    pub snapshot_keep: u32,
}

impl From<&Settings> for SettingsForm {
    fn from(s: &Settings) -> Self {
        Self {
            read_only: s.read_only,
            snapshot_days: s.snapshot_days,
            snapshot_keep: s.snapshot_keep,
        }
    }
}

impl Settings {
    /// 写回 `~/.fangcun/settings.toml`。这是方寸自己的文件，不经变更计划；只读模式下也能写，
    /// 不然关不掉只读模式。保留期至少 1 天、每个项目至少留 1 个。
    pub fn save(home: &FangcunHome, form: &SettingsForm) -> Result<Self> {
        if form.snapshot_days == 0 || form.snapshot_keep == 0 {
            return Err(Error::Parse {
                origin: "设置".into(),
                message: "快照保留天数和每个项目至少保留的个数都至少是 1".into(),
            });
        }
        let settings = Self {
            read_only: form.read_only,
            snapshot_days: form.snapshot_days,
            snapshot_keep: form.snapshot_keep,
        };
        let text = format!(
            "# 方寸的本机设置。界面「设置」页会改写这个文件，也可以手改。\n{}",
            toml::to_string(&settings).map_err(|e| Error::Parse {
                origin: "设置".into(),
                message: e.to_string(),
            })?
        );
        let file = home.settings_file();
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        crate::materializer::write_atomic(&file, text.as_bytes(), None)?;
        Ok(settings)
    }

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

    #[test]
    fn saving_writes_the_file_that_load_reads_back() {
        let root =
            std::env::temp_dir().join(format!("fangcun-settings-save-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = FangcunHome::at(&root);
        let form = SettingsForm {
            read_only: true,
            snapshot_days: 14,
            snapshot_keep: 5,
        };
        Settings::save(&home, &form).unwrap();
        assert_eq!(SettingsForm::from(&Settings::load(&home).unwrap()), form);
        let zero = SettingsForm {
            snapshot_days: 0,
            ..form
        };
        assert!(Settings::save(&home, &zero).is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
