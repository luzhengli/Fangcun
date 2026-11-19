//! 来源（F2、4.4、9.3「来源」）：把清单里的一项解析到确切的提交、标签、仓库里的目录，
//! 并把那一份存进资产库。
//!
//! - `github:<owner>/<repo>`：裸仓库缓存在 `~/.fangcun/git/github.com/<owner>/<repo>.git`，
//!   每次解析先拉一次。
//! - `local:<名字>`：名字对应的目录记在 `~/.fangcun/sources.toml` 的 `[local]` 里。写了 `pin`
//!   或 `track` 时按那个目录的 git 历史解析；都没写时取工作目录现在的样子，没有提交。
//! - 仓库里按 `SKILL.md` 的 `name` 找目录，和 `skills` 命令行一致；找到多个就要求清单写 `path`。
//! - 开发模式（`fangcun.local.toml` 的 `[dev]`）直接链到 [`Sources::local_dir`]，不经这里解析。
//! - 市场（skills.sh）只用来搜索，见 [`super::market`]，搜到的项落成 `github:` 来源。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::content::frontmatter_value;
use super::{Library, Origin, git};
use crate::model::{AssetKind, SourceRef, Version, VersionPolicy};
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 清单里一个来源的写法拆开后的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Github { owner: String, repo: String },
    Local { name: String },
}

impl Source {
    pub fn parse(source: &SourceRef) -> Result<Self> {
        let text = source.0.as_str();
        if let Some(rest) = text.strip_prefix("github:") {
            let (owner, repo) = rest
                .split_once('/')
                .filter(|(o, r)| is_plain(o) && is_plain(r))
                .ok_or_else(|| {
                    Error::Source(format!("来源写法不对：{text}，应为 github:<owner>/<repo>"))
                })?;
            return Ok(Self::Github {
                owner: owner.to_owned(),
                repo: repo.trim_end_matches(".git").to_owned(),
            });
        }
        if let Some(name) = text.strip_prefix("local:").filter(|n| is_plain(n)) {
            return Ok(Self::Local {
                name: name.to_owned(),
            });
        }
        Err(Error::Source(format!(
            "不认识的来源：{text}。支持 github:<owner>/<repo> 和 local:<名字>"
        )))
    }
}

/// 名字里只许字母、数字和 `-_.`，不许以 `.` 或 `-` 开头：拼进路径和 git 参数都安全。
fn is_plain(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with(['.', '-'])
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// 解析结果，写锁用（7.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// 本地来源没写 `pin`/`track` 时为空。
    pub commit: Option<String>,
    pub tag: Option<String>,
    /// skill 在仓库里的目录，用 `/` 连接；在仓库根目录时为空字符串。
    pub path: String,
    /// 存进资产库的版本。
    pub version: Version,
}

/// `~/.fangcun/sources.toml`。
#[derive(Debug, Default, Serialize, Deserialize)]
struct SourcesFile {
    #[serde(default)]
    local: BTreeMap<String, PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Sources {
    home: FangcunHome,
    github_base: String,
}

impl Sources {
    pub fn open(home: &FangcunHome) -> Self {
        Self {
            home: home.clone(),
            github_base: "https://github.com".to_owned(),
        }
    }

    /// 换掉 GitHub 的地址。测试用本地的裸仓库。
    #[must_use]
    pub fn with_github_base(mut self, base: impl Into<String>) -> Self {
        self.github_base = base.into();
        self
    }

    /// `local:<名字>` 对应的目录。没记过时报「本机没有来源」。
    pub fn local_dir(&self, name: &str) -> Result<PathBuf> {
        self.read()?
            .local
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Source(format!("本机没有来源 {name}，请选它所在的目录")))
    }

    /// 记下一个本地来源。同名已记成别的目录时覆盖。
    pub fn remember_local(&self, name: &str, dir: &Path) -> Result<()> {
        if !is_plain(name) {
            return Err(Error::Source(format!("来源名字不合适：{name}")));
        }
        let mut file = self.read()?;
        file.local.insert(name.to_owned(), dir.to_path_buf());
        let path = self.home.sources_file();
        let text = toml::to_string(&file).map_err(|e| Error::Parse {
            origin: "本地来源表".to_owned(),
            message: e.to_string(),
        })?;
        fs::create_dir_all(self.home.root()).map_err(io(self.home.root()))?;
        let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
        fs::write(&tmp, text).map_err(io(&tmp))?;
        fs::rename(&tmp, &path).map_err(io(&path))
    }

    fn read(&self) -> Result<SourcesFile> {
        let path = self.home.sources_file();
        match fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| Error::Parse {
                origin: path.display().to_string(),
                message: e.to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SourcesFile::default()),
            Err(source) => Err(Error::Io { path, source }),
        }
    }

    /// 解析一项 skill 并存进资产库。
    ///
    /// `path` 是清单里写的仓库内目录；不写时按名字找。
    pub fn resolve_skill(
        &self,
        library: &Library,
        name: &str,
        source: &SourceRef,
        policy: Option<&VersionPolicy>,
        path: Option<&str>,
    ) -> Result<Resolved> {
        let repo = match Source::parse(source)? {
            Source::Github { owner, repo } => self.fetch_github(&owner, &repo)?,
            Source::Local { name: local } => {
                let dir = self.local_dir(&local)?;
                if policy.is_none() {
                    return resolve_working_tree(library, name, source, &dir, path);
                }
                dir
            }
        };
        let (commit, tag) = resolve_policy(&repo, policy)?;
        let dir = match path {
            Some(path) => checked_path(path)?,
            None => find_in_commit(&repo, &commit, name)?,
        };
        let staging = library.staging()?;
        let result = git::export(&repo, &commit, &dir, &staging).and_then(|()| {
            if !staging.join("SKILL.md").is_file() {
                return Err(Error::Source(format!(
                    "{} 的 {dir} 里没有 SKILL.md",
                    source.0
                )));
            }
            library.ingest(
                name,
                AssetKind::Skill,
                &staging,
                &Origin {
                    source: Some(source.clone()),
                    commit: Some(commit.clone()),
                    tag: tag.clone(),
                },
            )
        });
        let _ = fs::remove_dir_all(&staging);
        Ok(Resolved {
            commit: Some(commit),
            tag,
            path: dir,
            version: result?,
        })
    }

    /// 裸仓库缓存：没有就克隆，有就拉一次。
    fn fetch_github(&self, owner: &str, repo: &str) -> Result<PathBuf> {
        let dir = self
            .home
            .git_cache_dir()
            .join("github.com")
            .join(owner)
            .join(format!("{repo}.git"));
        if dir.join("HEAD").exists() {
            git::run(
                Some(&dir),
                &[
                    "fetch",
                    "--quiet",
                    "--prune",
                    "--tags",
                    "--force",
                    "origin",
                    "+refs/heads/*:refs/heads/*",
                ],
            )?;
        } else {
            let url = format!("{}/{owner}/{repo}.git", self.github_base);
            if let Some(parent) = dir.parent() {
                fs::create_dir_all(parent).map_err(io(parent))?;
            }
            let target = dir.to_string_lossy().into_owned();
            git::run(None, &["clone", "--bare", "--quiet", "--", &url, &target])?;
        }
        Ok(dir)
    }
}

/// `track` 取分支上最新的提交；`pin` 先当标签找，再当提交找。标签另外按提交反查。
/// 都没写时取默认分支（`HEAD`）。
fn resolve_policy(repo: &Path, policy: Option<&VersionPolicy>) -> Result<(String, Option<String>)> {
    let (commit, tag) = match policy {
        Some(VersionPolicy::Track(branch)) => {
            let commit = git::commit_of(repo, &format!("refs/heads/{branch}"))
                .ok_or_else(|| Error::Source(format!("找不到分支 {branch}")))?;
            (commit, None)
        }
        Some(VersionPolicy::Pin(rev)) => {
            if let Some(commit) = git::commit_of(repo, &format!("refs/tags/{rev}")) {
                (commit, Some(rev.clone()))
            } else if rev.len() >= 4 && rev.bytes().all(|b| b.is_ascii_hexdigit()) {
                let commit = git::commit_of(repo, rev)
                    .ok_or_else(|| Error::Source(format!("找不到提交 {rev}")))?;
                (commit, None)
            } else {
                return Err(Error::Source(format!("找不到标签或提交 {rev}")));
            }
        }
        None => {
            let commit = git::commit_of(repo, "HEAD")
                .ok_or_else(|| Error::Source("仓库没有默认分支".to_owned()))?;
            (commit, None)
        }
    };
    let tag = match tag {
        Some(tag) => Some(tag),
        None => git::run_text(Some(repo), &["tag", "--points-at", &commit])?
            .lines()
            .next()
            .map(str::to_owned),
    };
    Ok((commit, tag))
}

/// 在某个提交里按名字找 skill 目录。
fn find_in_commit(repo: &Path, commit: &str, name: &str) -> Result<String> {
    let listing = git::run(Some(repo), &["ls-tree", "-r", "-z", "--name-only", commit])?;
    let mut found = Vec::new();
    for path in listing.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = String::from_utf8_lossy(path);
        let Some(dir) = skill_dir_of(&path) else {
            continue;
        };
        let text = git::run(Some(repo), &["show", &format!("{commit}:{path}")])?;
        if declared_name(&String::from_utf8_lossy(&text), dir) == name {
            found.push(dir.to_owned());
        }
    }
    pick(found, name)
}

/// 本地来源没写版本：在工作目录里按名字找，取现在的样子。
fn resolve_working_tree(
    library: &Library,
    name: &str,
    source: &SourceRef,
    root: &Path,
    path: Option<&str>,
) -> Result<Resolved> {
    let dir = match path {
        Some(path) => checked_path(path)?,
        None => {
            let mut files = Vec::new();
            crate::library::content::walk_skill_files(root, &mut files);
            let found = files
                .iter()
                .filter_map(|rel| {
                    let dir = skill_dir_of(rel)?;
                    let text = fs::read_to_string(root.join(rel)).ok()?;
                    (declared_name(&text, dir) == name).then(|| dir.to_owned())
                })
                .collect();
            pick(found, name)?
        }
    };
    let from = if dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(&dir)
    };
    let version = library.ingest(
        name,
        AssetKind::Skill,
        &from,
        &Origin {
            source: Some(source.clone()),
            commit: None,
            tag: None,
        },
    )?;
    Ok(Resolved {
        commit: None,
        tag: None,
        path: dir,
        version,
    })
}

/// `a/b/SKILL.md` → `a/b`，根目录的 `SKILL.md` → 空字符串，别的文件 → `None`。
fn skill_dir_of(path: &str) -> Option<&str> {
    if path == "SKILL.md" {
        return Some("");
    }
    path.strip_suffix("/SKILL.md")
}

/// frontmatter 的 `name`，没写时用目录名。
fn declared_name<'a>(skill_md: &str, dir: &'a str) -> std::borrow::Cow<'a, str> {
    match frontmatter_value(skill_md, "name") {
        Some(name) => std::borrow::Cow::Owned(name),
        None => std::borrow::Cow::Borrowed(dir.rsplit('/').next().unwrap_or(dir)),
    }
}

fn pick(mut found: Vec<String>, name: &str) -> Result<String> {
    found.sort();
    found.dedup();
    match found.len() {
        0 => Err(Error::Source(format!("仓库里没有名为 {name} 的 skill"))),
        1 => Ok(found.remove(0)),
        _ => Err(Error::Source(format!(
            "仓库里有 {} 个名为 {name} 的 skill：{}。请在清单里写 path",
            found.len(),
            found.join("、")
        ))),
    }
}

/// 清单里写的 `path` 只许是仓库里的相对路径。
fn checked_path(path: &str) -> Result<String> {
    let path = path.trim_matches('/');
    let ok = path
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != ".." && !part.starts_with('-'))
        || path.is_empty();
    if ok {
        Ok(path.to_owned())
    } else {
        Err(Error::Source(format!("path 写法不对：{path}")))
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}

#[cfg(all(test, unix))]
mod tests;
