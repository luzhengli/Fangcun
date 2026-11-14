//! 扫描用的只读文件工具：内容哈希、frontmatter、配置文件的键路径。

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::model::ContentHash;

/// 不计入内容哈希的文件。
const IGNORED: &[&str] = &[".DS_Store", ".git"];

/// 看作脚本的扩展名。Skill 里有这些文件时标「含脚本」。
const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "zsh", "py", "js", "mjs", "cjs", "ts", "rb"];

/// 一个目录的内容哈希：按相对路径排序，依次喂入路径和内容，SHA-256，十六进制。
///
/// 软链跟随到目标。同样内容的目录不论放在哪里，哈希都相同。
pub fn hash_dir(dir: &Path) -> ContentHash {
    let mut files = Vec::new();
    list_files(dir, Path::new(""), &mut files, 0);
    files.sort();
    let mut hasher = Sha256::new();
    for rel in &files {
        let Ok(bytes) = fs::read(dir.join(rel)) else {
            continue;
        };
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    ContentHash(hex(&hasher.finalize()))
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
}
