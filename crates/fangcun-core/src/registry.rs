//! 登记表 `~/.fangcun/projects.toml`（F3、M1-pre-06）：已登记的项目，每个一节。
//!
//! 写入范围按它算（M1-04）。三个登记入口出的计划经 [`with_registered`] 写它（M1-09）；带清单的
//! 仓库一经发现就自动登记，经 [`register_found`] 直接写，和资产库一样是方寸自己的目录。
//!
//! ```toml
//! [[project]]
//! path = "/Users/me/Project/evals_studio"
//! registered = "2026-11-19T08:00:00Z"
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Timestamp;
use crate::paths::FangcunHome;
use crate::scanner::fsutil::canonical;
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

/// 按真实路径比较：同一个项目经软链的不同写法算同一个。
pub fn contains(projects: &[RegisteredProject], root: &Path) -> bool {
    let wanted = canonical(root);
    projects.iter().any(|p| canonical(&p.path) == wanted)
}

/// 加上这些项目之后登记表的全文。已经登记过的不重复加。按路径排序，同样的登记表总是同样的字节。
pub fn with_registered(home: &FangcunHome, roots: &[PathBuf], at: &Timestamp) -> Result<String> {
    let mut projects = load(home)?;
    for root in roots {
        if !contains(&projects, root) {
            projects.push(RegisteredProject {
                path: root.clone(),
                registered: at.clone(),
            });
        }
    }
    projects.sort_by(|a, b| a.path.cmp(&b.path));
    toml::to_string(&File { project: projects }).map_err(|e| Error::Parse {
        origin: "登记表".to_owned(),
        message: e.to_string(),
    })
}

/// 自动登记带清单的仓库（F3）：直接写登记表，不出计划。没有要加的就不写。
pub fn register_found(home: &FangcunHome, roots: &[PathBuf]) -> Result<()> {
    let projects = load(home)?;
    if roots.iter().all(|root| contains(&projects, root)) {
        return Ok(());
    }
    let text = with_registered(home, roots, &Timestamp::now())?;
    let path = home.projects_file();
    fs::create_dir_all(home.root()).map_err(|source| Error::Io {
        path: home.root().to_path_buf(),
        source,
    })?;
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    fs::write(&tmp, text).map_err(|source| Error::Io {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, &path).map_err(|source| Error::Io { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> FangcunHome {
        let root =
            std::env::temp_dir().join(format!("fangcun-registry-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        FangcunHome::at(root)
    }

    #[test]
    fn registering_twice_keeps_one_entry_and_sorted_order() {
        let home = temp_home("twice");
        register_found(&home, &[PathBuf::from("/p/b"), PathBuf::from("/p/a")]).unwrap();
        let first = fs::read_to_string(home.projects_file()).unwrap();
        register_found(&home, &[PathBuf::from("/p/a")]).unwrap();
        assert_eq!(fs::read_to_string(home.projects_file()).unwrap(), first);
        let loaded = load(&home).unwrap();
        let paths: Vec<_> = loaded.iter().map(|p| p.path.clone()).collect();
        assert_eq!(paths, [PathBuf::from("/p/a"), PathBuf::from("/p/b")]);
        assert!(contains(&loaded, Path::new("/p/a")));
        assert!(!contains(&loaded, Path::new("/p/c")));
        let _ = fs::remove_dir_all(home.root());
    }

    #[test]
    fn nothing_to_add_writes_nothing() {
        let home = temp_home("none");
        register_found(&home, &[]).unwrap();
        assert!(!home.root().exists());
    }
}
