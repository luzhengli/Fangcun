//! 扫描用的只读文件工具：内容哈希、frontmatter、配置文件的键路径。

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::ContentHash;

/// 不计入内容哈希的文件。
const IGNORED: &[&str] = &[".DS_Store", ".git"];

/// 看作脚本的扩展名。Skill 里有这些文件时标「含脚本」。
const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "zsh", "py", "js", "mjs", "cjs", "ts", "rb"];

/// 内容哈希跳过的文件名，任何层级都算。
const HASH_EXCLUDED_FILES: &[&str] = &["metadata.json"];
/// 内容哈希跳过的目录名，任何层级都算。
const HASH_EXCLUDED_DIRS: &[&str] = &[".git", "__pycache__", "__pypackages__"];

/// 一个目录的内容哈希，与 `skills-lock.json` v2 同算法（7.4），照 `skills` 1.9.0 的
/// `computeSkillFolderHash` 实现：
///
/// - 取目录下全部文件，跳过 `metadata.json` 文件和 `.git`、`__pycache__`、`__pypackages__`
///   目录。别的都算，包括 `.DS_Store`、`node_modules`。
/// - 软链只跟随到目录以内，指向目录外或断掉的跳过。目录本身是软链时按它的目标算，
///   所以同样内容的目录不论放在哪里，哈希都相同。
/// - 相对路径用 `/` 连接，按 UTF-8 字节排序。依次喂入「路径、`\0`、内容长度（十进制）、
///   `\0`、内容」，SHA-256，小写十六进制。
/// - 不含 NUL 字节的文件把 CRLF 换成 LF，长度按换过的算。含 NUL 的看作二进制，原样喂入。
///
/// 读不了的文件跳过（CLI 遇到会整个失败）。
pub fn hash_dir(dir: &Path) -> ContentHash {
    let root = canonical(dir);
    let mut files = Vec::new();
    collect_hashed(dir, "", &root, &mut Vec::new(), &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    for (path, content) in &files {
        let content = normalize_line_endings(content);
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(content.len().to_string().as_bytes());
        hasher.update([0]);
        hasher.update(&content);
    }
    ContentHash(hex(&hasher.finalize()))
}

fn hash_excluded(name: &str, is_dir: bool) -> bool {
    HASH_EXCLUDED_FILES.contains(&name) || (is_dir && HASH_EXCLUDED_DIRS.contains(&name))
}

/// 收集参与哈希的文件：（相对路径，内容）。`ancestors` 是当前这条路径上已进入的真实目录，
/// 防止软链成环。
fn collect_hashed(
    dir: &Path,
    prefix: &str,
    root: &Path,
    ancestors: &mut Vec<PathBuf>,
    out: &mut Vec<(String, Vec<u8>)>,
) {
    let real = canonical(dir);
    if ancestors.contains(&real) {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    ancestors.push(real);
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if hash_excluded(&name, file_type.is_dir()) {
            continue;
        }
        let full = entry.path();
        let path = format!("{prefix}{name}");
        let mut is_dir = file_type.is_dir();
        if file_type.is_symlink() {
            let Ok(target) = fs::canonicalize(&full) else {
                continue;
            };
            if !target.starts_with(root) {
                continue;
            }
            is_dir = target.is_dir();
            if is_dir && hash_excluded(&name, true) {
                continue;
            }
        } else if !is_dir && !file_type.is_file() {
            continue;
        }
        if is_dir {
            collect_hashed(&full, &format!("{path}/"), root, ancestors, out);
        } else if let Ok(bytes) = fs::read(&full) {
            out.push((path, bytes));
        }
    }
    ancestors.pop();
}

/// 不含 NUL 字节且有 CRLF 时，把 CRLF 换成 LF。
fn normalize_line_endings(content: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if content.contains(&0) || !content.windows(2).any(|w| w == b"\r\n") {
        return std::borrow::Cow::Borrowed(content);
    }
    let mut out = Vec::with_capacity(content.len());
    let mut i = 0;
    while i < content.len() {
        if content[i] == b'\r' && content.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(content[i]);
            i += 1;
        }
    }
    std::borrow::Cow::Owned(out)
}

/// 一个文件的内容哈希。
pub fn hash_file(path: &Path) -> Option<ContentHash> {
    let bytes = fs::read(path).ok()?;
    Some(ContentHash(hex(&Sha256::digest(&bytes))))
}

/// 一段字节的内容哈希。
pub fn hash_bytes(bytes: &[u8]) -> ContentHash {
    ContentHash(hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 递归列出目录下的文件，返回相对路径。深度到 8 层为止，防止软链成环。
pub fn list_files(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if IGNORED.iter().any(|ignored| name == *ignored) {
            continue;
        }
        let path = entry.path();
        let rel_path = rel.join(&name);
        if path.is_dir() {
            list_files(&path, &rel_path, out, depth + 1);
        } else if path.is_file() {
            out.push(rel_path);
        }
    }
}

/// 目录里有没有脚本：`scripts/` 子目录，或常见脚本扩展名的文件。
pub fn has_scripts(dir: &Path) -> bool {
    let mut files = Vec::new();
    list_files(dir, Path::new(""), &mut files, 0);
    files.iter().any(|rel| {
        rel.components()
            .next()
            .is_some_and(|c| c.as_os_str() == "scripts")
            || rel
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| SCRIPT_EXTENSIONS.contains(&e))
    })
}

/// Markdown 开头 `---` 之间的 frontmatter，按「键: 值」逐行读。只取单行的值。
pub fn frontmatter(text: &str) -> Vec<(String, String)> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if line.starts_with([' ', '\t', '-']) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            out.push((key.trim().to_owned(), value.to_owned()));
        }
    }
    out
}

pub fn frontmatter_value(text: &str, key: &str) -> Option<String> {
    frontmatter(text)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

/// 读一份 JSON 或 TOML 配置，统一成 JSON 值。文件不存在或解析失败时为 `None`。
pub fn read_config(path: &Path, toml_format: bool) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    if toml_format {
        let value: toml::Value = toml::from_str(&text).ok()?;
        serde_json::to_value(value).ok()
    } else {
        serde_json::from_str(&text).ok()
    }
}

/// 沿键路径取值。
pub fn lookup<'a>(value: &'a serde_json::Value, key: &[String]) -> Option<&'a serde_json::Value> {
    key.iter()
        .try_fold(value, |node, segment| node.get(segment))
}

/// 用 `~` 代替主目录，界面和命令行显示路径时用。
pub fn display_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

/// 软链的目标。不是软链时为 `None`。
pub fn link_target(path: &Path) -> Option<PathBuf> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.file_type().is_symlink() {
        return None;
    }
    fs::read_link(path).ok()
}

pub fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_single_line_frontmatter() {
        let text = "---\nname: tdd\ndescription: \"Test first.\"\nmetadata:\n  a: b\n---\n# body";
        assert_eq!(frontmatter_value(text, "name").as_deref(), Some("tdd"));
        assert_eq!(
            frontmatter_value(text, "description").as_deref(),
            Some("Test first.")
        );
        assert_eq!(frontmatter_value("# no frontmatter", "name"), None);
    }

    #[test]
    fn display_path_uses_tilde() {
        let home = Path::new("/Users/me");
        assert_eq!(
            display_path(Path::new("/Users/me/.claude/skills"), home),
            "~/.claude/skills"
        );
        assert_eq!(display_path(Path::new("/opt/x"), home), "/opt/x");
    }

    /// 期望值由照 `skills` 1.9.0 `computeSkillFolderHash` 写的独立参考实现（Python）对同一组文件
    /// 算出。样例覆盖：CRLF 换 LF、含 NUL 的不换、三种跳过的目录与任意层级的 `metadata.json`、
    /// 不跳过的 `.DS_Store` 和 `node_modules`、`a-b` `a.b` `a/b` 的字节序、非 ASCII 文件名、
    /// 目录内的文件与目录软链、指向目录外、断掉和指向自身的软链、空目录。
    #[cfg(unix)]
    #[test]
    fn hash_dir_matches_skills_lock_v2() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!("fangcun-hash-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let skill = base.join("outer/skill");
        let write = |rel: &str, bytes: &[u8]| {
            let path = skill.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        };
        write("SKILL.md", b"---\r\nname: demo\r\n---\r\nbody\r\n");
        write("a-b.md", b"dash\n");
        write("a.b.md", b"dot\n");
        write("a/b.md", b"slash\n");
        write("中文.md", "unicode\n".as_bytes());
        write("bin.dat", b"\0\r\n\x01");
        write("metadata.json", b"{}");
        write("sub/metadata.json", b"{}");
        write("sub/note.md", b"nested\r\n");
        write("sub/__pycache__/x.pyc", b"pyc");
        write(".git/HEAD", b"ref");
        write("__pypackages__/y", b"y");
        write(".DS_Store", b"ds");
        write("node_modules/m.js", b"m");
        write("scripts/run.sh", b"echo hi\n");
        fs::create_dir_all(skill.join("empty")).unwrap();
        fs::write(base.join("outer/outside.txt"), b"out").unwrap();
        symlink("SKILL.md", skill.join("inner-link.md")).unwrap();
        symlink("sub", skill.join("linkdir")).unwrap();
        symlink("../outside.txt", skill.join("outside.md")).unwrap();
        symlink(".", skill.join("self")).unwrap();
        symlink("missing", skill.join("dangling")).unwrap();
        symlink("outer/skill", base.join("alias")).unwrap();

        let expected = "b0704d40f9bcdc3f31b9b44dacfe1dc99e8e5483df353631ab831329c154b7cf";
        assert_eq!(hash_dir(&skill).0, expected);
        assert_eq!(hash_dir(&base.join("alias")).0, expected);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn crlf_only_normalized_without_nul() {
        assert_eq!(&*normalize_line_endings(b"a\r\nb\r"), b"a\nb\r");
        assert_eq!(&*normalize_line_endings(b"\0\r\n"), b"\0\r\n");
    }
}
