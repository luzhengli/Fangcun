//! 资产库（F2、7.2）：本机唯一正本，内容寻址，多版本并存。
//!
//! - 正本在 `~/.fangcun/library/objects/<哈希>`：skill 是一个目录，指令是一个文件。
//!   同样的内容只存一份；正本只增不删，回滚也不删（6.7 回滚规则第 5 条）。
//! - 索引是 `~/.fangcun/index.sqlite`（7.2），见 `index`。
//! - 来源见 [`source`]：git 来源走 `~/.fangcun/git/` 下的本地裸仓库缓存；本地来源的名字记在
//!   `~/.fangcun/sources.toml`。市场搜索见 [`market`]。
//! - 「被哪些项目使用」不存在索引里，从各项目的锁算（M1-07）。
//!
//! 写的都是方寸自己的目录，不经变更计划；由收编、同步这些写入用例在 `apply` 时调用。

pub mod content;
pub mod diff;
mod git;
#[doc(hidden)]
pub mod hash_samples;
mod index;
pub mod market;
pub mod source;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::model::{AssetKind, ContentHash, SourceRef, Version};
use crate::paths::FangcunHome;
use crate::{Error, Result};

use content::{HashedFile, SCRIPT_EXTENSIONS, hash_bytes, hash_dir, hash_files, hashed_files};

/// 资产库里的一项资产。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    pub name: String,
    pub kind: AssetKind,
    /// 来源未知的资产没有来源，等待收编。
    pub source: Option<SourceRef>,
    /// 先存进来的在前。
    pub versions: Vec<Version>,
    pub has_scripts: bool,
}

/// 存进来的这一份从哪来。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    /// 来源未知时为空。
    pub source: Option<SourceRef>,
    pub commit: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    index: PathBuf,
}

impl Library {
    /// 只算路径，不创建目录。
    pub fn open(home: &FangcunHome) -> Result<Self> {
        Ok(Self {
            root: home.library_dir(),
            index: home.index_db(),
        })
    }

    /// 正本所在的目录：`objects/<哈希>`。指向这里的软链就是受管链接。
    pub fn objects_dir(&self) -> PathBuf {
        self.objects()
    }

    fn objects(&self) -> PathBuf {
        self.root.join("objects")
    }

    /// 按种类、名字排序。
    pub fn entries(&self) -> Result<Vec<LibraryEntry>> {
        let mut entries: Vec<LibraryEntry> = index::read(&self.index)?
            .into_iter()
            .map(|asset| LibraryEntry {
                name: asset.name,
                kind: asset.kind,
                source: asset.source,
                versions: asset.versions,
                has_scripts: asset.has_scripts,
            })
            .collect();
        entries.sort_by(|a, b| (a.kind, &a.name).cmp(&(b.kind, &b.name)));
        Ok(entries)
    }

    /// 按内容哈希取正本：skill 是目录，指令是文件。没有就是 `None`。
    pub fn locate(&self, hash: &ContentHash) -> Result<Option<PathBuf>> {
        // 哈希来自锁文件，可能被手改过；只认十六进制，防止拼出库外的路径。
        if hash.0.is_empty() || !hash.0.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok(None);
        }
        let path = self.objects().join(&hash.0);
        Ok(path.exists().then_some(path))
    }

    /// 把一份副本存进库，返回它的版本。原位置的替换由物化器做，不在这里。
    ///
    /// - skill：`from` 是目录。存的是参与哈希的那些文件（`.git`、`__pycache__`、
    ///   `metadata.json` 不存），软链按内容存成普通文件，保留可执行权限。存完再算一遍哈希，
    ///   对不上（存的时候源目录在变）就不收。
    /// - 指令：`from` 是文件，原样存。
    /// - MCP：配置里可能有明文密钥，等 M1-17 接上钥匙串再存。
    ///
    /// 内容已经在库里时不再复制；索引里同名、同类、同来源的资产已有这个版本时不重复记。
    pub fn ingest(
        &self,
        name: &str,
        kind: AssetKind,
        from: &Path,
        origin: &Origin,
    ) -> Result<Version> {
        let (hash, has_scripts) = match kind {
            AssetKind::Skill => self.store_dir(from)?,
            AssetKind::Instruction => (self.store_file(from)?, false),
            AssetKind::Mcp => {
                return Err(Error::NotImplemented("library::ingest（MCP，随 M1-17）"));
            }
        };
        let version = Version {
            hash: hash.clone(),
            commit: origin.commit.clone(),
            tag: origin.tag.clone(),
        };
        index::record(
            &self.index,
            name,
            kind,
            origin.source.as_ref(),
            has_scripts,
            &version,
        )?;
        Ok(version)
    }

    /// 向来源查有没有新版本。只查不改。M1-16。
    pub fn check_upstream(&self, _name: &str) -> Result<Option<Version>> {
        Err(Error::NotImplemented("library::Library::check_upstream"))
    }

    fn store_dir(&self, from: &Path) -> Result<(ContentHash, bool)> {
        if !from.is_dir() {
            return Err(Error::NotFound(from.display().to_string()));
        }
        let files = hashed_files(from);
        let hash = hash_files(&files);
        let has_scripts = files.iter().any(is_script);
        let object = self.objects().join(&hash.0);
        if object.exists() {
            return Ok((hash, has_scripts));
        }
        let tmp = self.staging()?;
        let result = (|| {
            for file in &files {
                let path = tmp.join(&file.rel);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(io(parent))?;
                }
                fs::write(&path, &file.bytes).map_err(io(&path))?;
                copy_mode(&file.source, &path)?;
            }
            if hash_dir(&tmp) != hash {
                return Err(Error::Parse {
                    origin: from.display().to_string(),
                    message: "存进资产库时内容变了，请重试".to_owned(),
                });
            }
            self.commit(&tmp, &object)
        })();
        if tmp.exists() {
            let _ = fs::remove_dir_all(&tmp);
        }
        result.map(|()| (hash, has_scripts))
    }

    fn store_file(&self, from: &Path) -> Result<ContentHash> {
        let bytes = fs::read(from).map_err(io(from))?;
        let hash = hash_bytes(&bytes);
        let object = self.objects().join(&hash.0);
        if object.exists() {
            return Ok(hash);
        }
        let tmp = self.staging()?;
        fs::remove_dir(&tmp).map_err(io(&tmp))?;
        fs::write(&tmp, &bytes).map_err(io(&tmp))?;
        let result = self.commit(&tmp, &object);
        if tmp.exists() {
            let _ = fs::remove_file(&tmp);
        }
        result.map(|()| hash)
    }

    /// 暂存目录：存一半的东西不会出现在 `objects/` 里。
    ///
    /// 名字在本进程里永不重复。用完的目录改名进 `objects/` 后名字就空了，调用方随后按名字
    /// 清理；名字要是会被别的线程重新领走，就会删掉别人复制到一半的目录（2026-11-19 CI 上
    /// `concurrent_ingests_all_reach_the_index` 偶发失败就是这样）。
    pub(crate) fn staging(&self) -> Result<PathBuf> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = self.root.join("tmp");
        fs::create_dir_all(&dir).map_err(io(&dir))?;
        fs::create_dir_all(self.objects()).map_err(io(&self.objects()))?;
        let mut attempts = 0;
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let tmp = dir.join(format!("{}-{n}", std::process::id()));
            match fs::create_dir(&tmp) {
                Ok(()) => return Ok(tmp),
                // 上次崩溃留下的同名目录：跳过这个号。
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempts < 10_000 => {
                    attempts += 1;
                }
                Err(source) => return Err(Error::Io { path: tmp, source }),
            }
        }
    }

    /// 改名进 `objects/`。别人刚好存了同样的内容时，用别人的那份。
    fn commit(&self, tmp: &Path, object: &Path) -> Result<()> {
        match fs::rename(tmp, object) {
            Ok(()) => Ok(()),
            Err(_) if object.exists() => Ok(()),
            Err(source) => Err(Error::Io {
                path: object.to_path_buf(),
                source,
            }),
        }
    }
}

fn is_script(file: &HashedFile) -> bool {
    file.rel.split('/').next() == Some("scripts")
        || Path::new(&file.rel)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SCRIPT_EXTENSIONS.contains(&e))
}

fn copy_mode(from: &Path, to: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(from).map_err(io(from))?.permissions().mode() & 0o777;
        fs::set_permissions(to, fs::Permissions::from_mode(mode)).map_err(io(to))?;
    }
    #[cfg(not(unix))]
    let _ = (from, to);
    Ok(())
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    let path = path.to_path_buf();
    move |source| Error::Io { path, source }
}

#[cfg(all(test, unix))]
mod tests;

/// 文件有没有可执行位。回流写 git 时用。
pub(crate) fn diff_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}
