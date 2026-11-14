//! 适配器层：每个 Agent 工具一份声明式描述（7.3）。
//!
//! 内核的其他模块不写死任何工具的路径或格式，一律从这里查。

mod registry;
mod schema;

pub use registry::AdapterRegistry;
pub use schema::{
    Adapter, Capabilities, ConfigList, FileFormat, InstallIndex, InstructionLocation, LockFormat,
    LockRef, McpFields, McpLocation, MergeMode, ObservedLocation, Pick, Precedence, Requirement,
    RulesLocation, SessionFormat, SessionSource, SkillLocation, SkillSwitches, SupportLevel,
    ToolInfo, Walk,
};

use std::path::{Path, PathBuf};

/// 把适配器里的路径写法换成绝对路径。
///
/// `~/…` 拼在主目录下，`/…` 原样，其余拼在项目根目录下。
/// 相对路径而没有项目时返回 `None`。
pub fn expand(spec: &str, home: &Path, project: Option<&Path>) -> Option<PathBuf> {
    if let Some(rest) = spec.strip_prefix("~/") {
        Some(home.join(rest))
    } else if spec.starts_with('/') {
        Some(PathBuf::from(spec))
    } else {
        project.map(|root| root.join(spec))
    }
}

/// 项目层位置要看的目录，按从上到下的顺序。
pub fn walk_dirs(walk: Walk, project: &Path) -> Vec<PathBuf> {
    match walk {
        Walk::Root => vec![project.to_path_buf()],
        Walk::Ancestors => {
            let mut dirs: Vec<PathBuf> = project.ancestors().map(Path::to_path_buf).collect();
            dirs.reverse();
            dirs
        }
        Walk::GitRootDown => {
            let Some(git_root) = project.ancestors().find(|dir| dir.join(".git").exists()) else {
                return vec![project.to_path_buf()];
            };
            let mut dirs: Vec<PathBuf> = project
                .ancestors()
                .take_while(|dir| *dir != git_root)
                .map(Path::to_path_buf)
                .collect();
            dirs.push(git_root.to_path_buf());
            dirs.reverse();
            dirs
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_the_three_path_forms() {
        let home = Path::new("/Users/me");
        let project = Path::new("/Users/me/code/p");
        assert_eq!(
            expand("~/.claude/skills", home, None),
            Some(PathBuf::from("/Users/me/.claude/skills"))
        );
        assert_eq!(expand("/etc/x", home, None), Some(PathBuf::from("/etc/x")));
        assert_eq!(
            expand(".mcp.json", home, Some(project)),
            Some(PathBuf::from("/Users/me/code/p/.mcp.json"))
        );
        assert_eq!(expand(".mcp.json", home, None), None);
    }

    #[test]
    fn ancestors_run_from_the_top_down() {
        let dirs = walk_dirs(Walk::Ancestors, Path::new("/a/b"));
        assert_eq!(
            dirs,
            [
                PathBuf::from("/"),
                PathBuf::from("/a"),
                PathBuf::from("/a/b")
            ]
        );
    }
}
