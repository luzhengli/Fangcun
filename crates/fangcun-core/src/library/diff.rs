//! 两份 skill 目录的差异（6.3 检查器的漂移块与可更新块，F6）：逐个文件比，文本按行，
//! 脚本的变更单独标出来（7.5「升级差异里，脚本、hook、MCP 启动命令的变化单独高亮」）。
//!
//! 只比参与内容哈希的文件（[`super::content::hashed_files`]），和锁、漂移检测的口径一样。

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use ts_rs::TS;

use super::content::hashed_files;
use crate::journal::FileOp;

/// 每个文件最多给界面多少行差异，再多的只计数。
const LINES_PER_FILE: usize = 60;

/// 一个文件的变化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileDiff {
    /// 相对 skill 目录，用 `/` 连接。
    pub path: String,
    pub op: FileOp,
    pub added: u32,
    pub removed: u32,
    /// 脚本：`scripts/` 下的、可执行的、以 `#!` 开头的、常见脚本扩展名的。界面标红。
    pub script: bool,
    /// 二进制文件只计数，不列行。
    pub binary: bool,
    /// 带上下文的差异行，最多 [`LINES_PER_FILE`] 行。
    pub lines: Vec<DiffLine>,
    /// 超出上限、没列出来的行数。
    pub truncated: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum DiffKind {
    /// `@@ SKILL.md · 第 14 行 @@`
    Hunk,
    Context,
    Add,
    Remove,
}

/// 加了几行、删了几行的一句话，例如「多 3 行、少 1 行」。
pub fn summary(diffs: &[FileDiff]) -> String {
    let added: u32 = diffs.iter().map(|d| d.added).sum();
    let removed: u32 = diffs.iter().map(|d| d.removed).sum();
    match (added, removed) {
        (0, 0) => format!("{} 个文件有变化", diffs.len()),
        (a, 0) => format!("多 {a} 行"),
        (0, r) => format!("少 {r} 行"),
        (a, r) => format!("多 {a} 行、少 {r} 行"),
    }
}

fn is_script(rel: &str, bytes: &[u8], executable: bool) -> bool {
    const EXT: &[&str] = &[
        "sh", "bash", "zsh", "py", "js", "mjs", "cjs", "ts", "rb", "pl", "ps1",
    ];
    executable
        || rel.starts_with("scripts/")
        || rel.contains("/scripts/")
        || bytes.starts_with(b"#!")
        || rel
            .rsplit_once('.')
            .is_some_and(|(_, ext)| EXT.contains(&ext))
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_path: &Path) -> bool {
    false
}

/// 从 `old` 到 `new` 的差异。`old` 为空时（原版取不回）每个文件都算新增。没变化的文件不列。
pub fn diff_dirs(old: Option<&Path>, new: &Path) -> Vec<FileDiff> {
    let before: BTreeMap<String, (Vec<u8>, bool)> = old
        .map(|dir| {
            hashed_files(dir)
                .into_iter()
                .map(|f| {
                    let x = executable(&f.source);
                    (f.rel, (f.bytes, x))
                })
                .collect()
        })
        .unwrap_or_default();
    let after: BTreeMap<String, (Vec<u8>, bool)> = hashed_files(new)
        .into_iter()
        .map(|f| {
            let x = executable(&f.source);
            (f.rel, (f.bytes, x))
        })
        .collect();
    let mut out = Vec::new();
    let names: std::collections::BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for rel in names {
        let (op, old_bytes, new_bytes, x) = match (before.get(rel), after.get(rel)) {
            (Some((a, _)), Some((b, x))) if a == b => {
                let _ = x;
                continue;
            }
            (Some((a, _)), Some((b, x))) => (FileOp::Modify, a.as_slice(), b.as_slice(), *x),
            (None, Some((b, x))) => (FileOp::Add, &[][..], b.as_slice(), *x),
            (Some((a, x)), None) => (FileOp::Remove, a.as_slice(), &[][..], *x),
            (None, None) => continue,
        };
        out.push(file_diff(rel, op, old_bytes, new_bytes, x));
    }
    out
}

fn file_diff(rel: &str, op: FileOp, old: &[u8], new: &[u8], executable: bool) -> FileDiff {
    let script = is_script(rel, if new.is_empty() { old } else { new }, executable);
    let (Ok(old_text), Ok(new_text)) = (std::str::from_utf8(old), std::str::from_utf8(new)) else {
        return FileDiff {
            path: rel.to_owned(),
            op,
            added: 0,
            removed: 0,
            script,
            binary: true,
            lines: Vec::new(),
            truncated: 0,
        };
    };
    let diff = TextDiff::from_lines(old_text, new_text);
    let mut lines = Vec::new();
    let mut added = 0u32;
    let mut removed = 0u32;
    let mut truncated = 0u32;
    for group in diff.grouped_ops(2) {
        let Some(first) = group.first() else {
            continue;
        };
        let start = first.new_range().start + 1;
        let mut chunk = vec![DiffLine {
            kind: DiffKind::Hunk,
            text: format!("@@ {rel} · 第 {start} 行 @@"),
        }];
        for op in &group {
            for change in diff.iter_changes(op) {
                let kind = match change.tag() {
                    ChangeTag::Equal => DiffKind::Context,
                    ChangeTag::Insert => {
                        added += 1;
                        DiffKind::Add
                    }
                    ChangeTag::Delete => {
                        removed += 1;
                        DiffKind::Remove
                    }
                };
                chunk.push(DiffLine {
                    kind,
                    text: change.value().trim_end_matches(['\n', '\r']).to_owned(),
                });
            }
        }
        for line in chunk {
            if lines.len() < LINES_PER_FILE {
                lines.push(line);
            } else if line.kind != DiffKind::Hunk {
                truncated += 1;
            }
        }
    }
    FileDiff {
        path: rel.to_owned(),
        op,
        added,
        removed,
        script,
        binary: false,
        lines,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn diffs_text_and_flags_scripts() {
        let base = std::env::temp_dir().join(format!("fangcun-diff-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(b.join("scripts")).unwrap();
        fs::write(a.join("SKILL.md"), "one\ntwo\nthree\n").unwrap();
        fs::write(b.join("SKILL.md"), "one\n2\nthree\nfour\n").unwrap();
        fs::write(b.join("scripts/run.sh"), "echo hi\n").unwrap();
        let diffs = diff_dirs(Some(&a), &b);
        assert_eq!(diffs.len(), 2);
        let md = diffs.iter().find(|d| d.path == "SKILL.md").unwrap();
        assert_eq!((md.added, md.removed, md.script), (2, 1, false));
        assert_eq!(md.lines[0].kind, DiffKind::Hunk);
        let sh = diffs.iter().find(|d| d.path == "scripts/run.sh").unwrap();
        assert!(sh.script && sh.op == FileOp::Add);
        assert_eq!(summary(&diffs), "多 3 行、少 1 行");
        fs::remove_dir_all(&base).unwrap();
    }
}
