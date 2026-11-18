//! 快照（F6、M1-pre-07）：只存这次要动的每个路径的原样。
//!
//! - 文件内容整份存进 `~/.fangcun/snapshots/objects/<sha256>`，按内容去重。
//! - 每个快照一份 TOML：`snapshots/global/<编号>.toml` 或 `snapshots/projects/<键>/<编号>.toml`，
//!   记路径、状态（不存在、文件、软链、目录）、哈希、权限、软链目标。编号按范围各自递增（6.7）。
//! - 保留期是 6.7 的「默认 30 天，每个项目至少 20 个」，清理在 M1-11 做。

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{SnapshotId, Timestamp};
use crate::paths::FangcunHome;
use crate::{Error, Result};

use super::State;

/// 快照的范围：一个项目，或全局层。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnapshotScope {
    Project(PathBuf),
    Global,
}

impl SnapshotScope {
    /// 项目根目录落到真实路径，同一个项目经不同的软链写法也是同一个范围。
    pub fn normalized(&self) -> Self {
        match self {
            Self::Global => Self::Global,
            Self::Project(root) => Self::Project(super::scope::resolve(root)),
        }
    }

    fn dir(&self, home: &FangcunHome) -> PathBuf {
        match self {
            Self::Global => home.snapshots_dir().join("global"),
            // 路径做键会很长，也可能有不能做文件名的字符，用哈希的前 16 位。
            Self::Project(root) => home
                .snapshots_dir()
                .join("projects")
                .join(&sha256_hex(root.to_string_lossy().as_bytes())[..16]),
        }
    }

    fn label(&self) -> String {
        match self {
            Self::Global => "global".to_owned(),
            Self::Project(root) => root.to_string_lossy().into_owned(),
        }
    }
}

/// 一个快照文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: u32,
    /// 项目根目录的绝对路径，或 `global`。
    pub scope: String,
    pub at: Timestamp,
    pub title: String,
    #[serde(rename = "entry", default)]
    pub entries: Vec<Entry>,
}

/// 一个路径在写入前的样子。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Entry {
    pub path: PathBuf,
    #[serde(flatten)]
    pub state: Saved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Saved {
    Absent,
    File { hash: String, mode: u32 },
    Symlink { target: PathBuf },
    Dir,
}

/// 留一个快照：把 `paths` 现在的样子存下来，返回编号。
pub fn capture(
    home: &FangcunHome,
    scope: &SnapshotScope,
    title: &str,
    paths: &[&Path],
) -> Result<SnapshotId> {
    let scope = &scope.normalized();
    let objects = home.snapshots_dir().join("objects");
    let mut entries = Vec::new();
    for path in paths {
        let state = match State::read(path)? {
            State::Absent => Saved::Absent,
            State::Dir => Saved::Dir,
            State::Symlink(target) => Saved::Symlink { target },
            State::File(_) => {
                let bytes = fs::read(path).map_err(io(path))?;
                let hash = sha256_hex(&bytes);
                let object = objects.join(&hash);
                if !object.exists() {
                    write_new(&objects, &object, &bytes)?;
                }
                Saved::File {
                    hash,
                    mode: file_mode(path),
                }
            }
        };
        entries.push(Entry {
            path: path.to_path_buf(),
            state,
        });
    }

    let dir = scope.dir(home);
    fs::create_dir_all(&dir).map_err(io(&dir))?;
    let mut id = next_id(&dir);
    loop {
        let snapshot = Snapshot {
            id,
            scope: scope.label(),
            at: Timestamp::now(),
            title: title.to_owned(),
            entries: entries.clone(),
        };
        let text = toml::to_string(&snapshot).map_err(|e| Error::Parse {
            origin: "快照".to_owned(),
            message: e.to_string(),
        })?;
        let file = dir.join(format!("{id}.toml"));
        // 同时有两次写入时编号可能撞上，`create_new` 保证不覆盖别人的快照。
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
        {
            Ok(mut handle) => {
                handle.write_all(text.as_bytes()).map_err(io(&file))?;
                return Ok(SnapshotId(id));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => id += 1,
            Err(source) => return Err(Error::Io { path: file, source }),
        }
    }
}

/// 把一个路径还原成快照里的样子。
pub fn restore(home: &FangcunHome, entry: &Entry) -> Result<()> {
    let path = &entry.path;
    match &entry.state {
        Saved::Absent => remove(path),
        Saved::Dir => fs::create_dir_all(path).map_err(io(path)),
        Saved::Symlink { target } => {
            remove(path)?;
            symlink(target, path)
        }
        Saved::File { hash, mode } => {
            let bytes =
                fs::read(home.snapshots_dir().join("objects").join(hash)).map_err(io(path))?;
            if matches!(State::read(path)?, State::Dir | State::Symlink(_)) {
                remove(path)?;
            }
            super::write_atomic(path, &bytes, Some(*mode))
        }
    }
}

/// 读一个快照。回滚（M1-11）用。
pub fn load(home: &FangcunHome, scope: &SnapshotScope, id: SnapshotId) -> Result<Snapshot> {
    let scope = &scope.normalized();
    let file = scope.dir(home).join(format!("{}.toml", id.0));
    let text = fs::read_to_string(&file).map_err(io(&file))?;
    toml::from_str(&text).map_err(|e| Error::Parse {
        origin: file.display().to_string(),
        message: e.to_string(),
    })
}

/// 删掉一个文件、软链或空目录。不存在就算了。
pub(super) fn remove(path: &Path) -> Result<()> {
    match State::read(path)? {
        State::Absent => Ok(()),
        State::Dir => fs::remove_dir(path).map_err(io(path)),
        State::File(_) | State::Symlink(_) => fs::remove_file(path).map_err(io(path)),
    }
}

pub(super) fn symlink(target: &Path, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, path).map_err(io(path))
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        Err(Error::NotImplemented("materializer::symlink（非 Unix）"))
    }
}

fn next_id(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    e.file_name()
                        .to_str()?
                        .strip_suffix(".toml")?
                        .parse::<u32>()
                        .ok()
                })
                .max()
                .map_or(1, |max| max + 1)
        })
        .unwrap_or(1)
}

/// 先写临时文件再改名，存一半的对象不会被当成完整的。
fn write_new(dir: &Path, object: &Path, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(dir).map_err(io(dir))?;
    super::write_atomic(object, bytes, None)
}

pub(super) fn file_mode(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).map_or(0o644, |m| m.permissions().mode() & 0o7777)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0o644
    }
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(super) fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}
