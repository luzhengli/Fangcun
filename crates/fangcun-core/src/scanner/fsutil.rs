//! 扫描用的只读文件工具：frontmatter、配置文件的键路径。内容哈希在 `library::content`，这里转出。

use std::fs;
use std::path::{Path, PathBuf};

pub use crate::library::content::{
    SCRIPT_EXTENSIONS, frontmatter, frontmatter_value, hash_bytes, hash_dir, hash_file,
};

/// 不计入内容哈希的文件。
const IGNORED: &[&str] = &[".DS_Store", ".git"];

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
