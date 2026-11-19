//! 内容寻址（F2、7.4）：资产的内容哈希。资产库按它存正本，扫描按它归并副本，锁按它校验。

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::ContentHash;

/// 看作脚本的扩展名。Skill 里有这些文件时标「含脚本」。
pub const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "zsh", "py", "js", "mjs", "cjs", "ts", "rb"];

/// 参与哈希的一个文件。
#[derive(Debug, Clone)]
pub struct HashedFile {
    /// 相对路径，用 `/` 连接。
    pub rel: String,
    /// 读的是哪个文件（软链已跟随）。
    pub source: PathBuf,
    pub bytes: Vec<u8>,
}

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
    hash_files(&hashed_files(dir))
}

/// 参与哈希的文件，按相对路径的字节序排好。
pub fn hashed_files(dir: &Path) -> Vec<HashedFile> {
    let root = canonical(dir);
    let mut files = Vec::new();
    collect_hashed(dir, "", &root, &mut Vec::new(), &mut files);
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    files
}

/// 已经排好序的文件的哈希。
pub fn hash_files(files: &[HashedFile]) -> ContentHash {
    let mut hasher = Sha256::new();
    for file in files {
        let content = normalize_line_endings(&file.bytes);
        hasher.update(file.rel.as_bytes());
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

/// 收集参与哈希的文件。`ancestors` 是当前这条路径上已进入的真实目录，
/// 防止软链成环。
fn collect_hashed(
    dir: &Path,
    prefix: &str,
    root: &Path,
    ancestors: &mut Vec<PathBuf>,
    out: &mut Vec<HashedFile>,
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
            out.push(HashedFile {
                rel: path,
                source: full,
                bytes,
            });
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

/// 旧版（v1）`skills-lock.json` 的目录哈希，只用来校验导入的旧锁（7.4），照 `skills` 1.9.0 的
/// `computeLegacySkillFolderHash`：
///
/// - 取目录下的普通文件，跳过 `.git`、`node_modules` 目录；软链（文件和目录）一律不算。
/// - 相对路径用 `/` 连接，按 JS 的 `localeCompare` 排序（见 [`locale_order`]）。
/// - 依次喂入「路径、内容」，不加分隔，不换行尾。SHA-256，小写十六进制。
pub fn legacy_hash_dir(dir: &Path) -> ContentHash {
    fn collect(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{prefix}{name}");
            if file_type.is_dir() {
                if name != ".git" && name != "node_modules" {
                    collect(&entry.path(), &format!("{rel}/"), out);
                }
            } else if file_type.is_file() {
                out.push((rel, entry.path()));
            }
        }
    }
    let mut files = Vec::new();
    collect(dir, "", &mut files);
    files.sort_by(|a, b| locale_order(&a.0, &b.0));
    let mut hasher = Sha256::new();
    for (rel, path) in &files {
        hasher.update(rel.as_bytes());
        if let Ok(bytes) = fs::read(path) {
            hasher.update(&bytes);
        }
    }
    ContentHash(hex(&hasher.finalize()))
}

/// JS `localeCompare` 的近似：ICU 根排序（CLDR root）对 ASCII 的规则。
///
/// 先比主权重：标点和空白按 ICU 的顺序排在最前，其次数字，再其次字母（不分大小写）；
/// 主权重相同再比大小写，小写在前。非 ASCII 字符排在最后，按码位。路径里常见的字符都覆盖了，
/// 本机 21 个 v1 锁的哈希全部对上（M1-08）。
pub fn locale_order(a: &str, b: &str) -> std::cmp::Ordering {
    /// ICU 根排序里 ASCII 标点与空白的先后。
    const PUNCT: &str = "\t\n\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    fn primary(c: char) -> (u8, u32) {
        if let Some(i) = PUNCT.find(c) {
            (0, u32::try_from(i).unwrap_or(u32::MAX))
        } else if c.is_ascii_digit() {
            (1, u32::from(c))
        } else if c.is_ascii_alphabetic() {
            (2, u32::from(c.to_ascii_lowercase()))
        } else {
            (3, u32::from(c))
        }
    }
    let primaries = a.chars().map(primary).cmp(b.chars().map(primary));
    primaries.then_with(|| {
        a.chars()
            .map(|c| c.is_ascii_uppercase())
            .cmp(b.chars().map(|c| c.is_ascii_uppercase()))
    })
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

/// 工作目录里所有 `SKILL.md` 的相对路径（用 `/` 连接）。跳过 `.git`、`node_modules`、
/// `__pycache__`，不跟随软链，最深 8 层。
pub fn walk_skill_files(root: &Path, out: &mut Vec<String>) {
    fn walk(dir: &Path, prefix: &str, depth: u32, out: &mut Vec<String>) {
        if depth > 8 {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                if !matches!(name.as_str(), ".git" | "node_modules" | "__pycache__") {
                    walk(&entry.path(), &format!("{prefix}{name}/"), depth + 1, out);
                }
            } else if kind.is_file() && name == "SKILL.md" {
                out.push(format!("{prefix}{name}"));
            }
        }
    }
    walk(root, "", 0, out);
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 期望值由照 `skills` 1.9.0 `computeSkillFolderHash` 写的独立参考实现（Python）对同一组文件
    /// 算出，样例见 [`super::super::hash_samples::write_v2_sample`]。CI 另外拿真正的 `skills`
    /// 命令行对同一组文件比对一次（W-13）。
    #[cfg(unix)]
    #[test]
    fn hash_dir_matches_skills_lock_v2() {
        let base = std::env::temp_dir().join(format!("fangcun-hash-v2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let skill = base.join("outer/skill");
        crate::library::hash_samples::write_v2_sample(&skill).unwrap();
        std::os::unix::fs::symlink("outer/skill", base.join("alias")).unwrap();

        let expected = "b0704d40f9bcdc3f31b9b44dacfe1dc99e8e5483df353631ab831329c154b7cf";
        assert_eq!(hash_dir(&skill).0, expected);
        assert_eq!(hash_dir(&base.join("alias")).0, expected);
        let _ = fs::remove_dir_all(&base);
    }

    /// v1 的期望值由同一套排序规则的 Python 参考实现算出，样例见
    /// [`super::super::hash_samples::write_v1_sample`]。这套规则对上了本机
    /// `~/Project/article/skills-lock.json` 的全部 21 个 v1 哈希（M1-08）。
    #[cfg(unix)]
    #[test]
    fn legacy_hash_dir_matches_skills_lock_v1() {
        let dir = std::env::temp_dir().join(format!("fangcun-hash-v1-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        crate::library::hash_samples::write_v1_sample(&dir).unwrap();
        assert_eq!(
            legacy_hash_dir(&dir).0,
            "fe76ef5c8b19a373e0c107918cbeee7ccac65f30f2affd1462451bab0dc197bc"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn locale_order_follows_icu_root() {
        let mut names = vec![
            "SKILL.md",
            "scripts/main.ts",
            "Z.md",
            "a-b",
            "a_b",
            "a.b",
            "a1",
            "ab",
        ];
        names.sort_by(|a, b| locale_order(a, b));
        assert_eq!(
            names,
            [
                "a_b",
                "a-b",
                "a.b",
                "a1",
                "ab",
                "scripts/main.ts",
                "SKILL.md",
                "Z.md"
            ]
        );
    }

    #[test]
    fn crlf_only_normalized_without_nul() {
        assert_eq!(&*normalize_line_endings(b"a\r\nb\r"), b"a\nb\r");
        assert_eq!(&*normalize_line_endings(b"\0\r\n"), b"\0\r\n");
    }
}
