//! 清单、锁、套装文件的读写（F3）。文件即真相：界面只是它们的编辑器。
//!
//! | 文件 | 位置 | 类型 |
//! | --- | --- | --- |
//! | `fangcun.toml` | 项目根目录，提交 | [`Manifest`] |
//! | `fangcun.local.toml` | 项目根目录，不提交 | [`Manifest`]（个人覆盖） |
//! | `fangcun.lock` | 项目根目录，提交 | [`Lock`] |
//! | `<名称>.toml` | `~/.fangcun/bundles/` | [`Bundle`] |
//!
//! 界面改清单时要保住人写的注释和顺序。这里的 serde 结构只负责读；
//! 写回用保留格式的编辑方式，实现 M1 的清单编辑时再引入。

mod interop;
mod lock;
mod schema;

pub use interop::{export_skills_lock, import_skills_lock};
pub use lock::Lock;
pub use schema::{Bundle, InstructionsDecl, Manifest, McpDecl, ProjectSection, SkillDecl};

use std::path::Path;

use crate::{Error, Result};

/// 解析一份清单文本。`origin` 只用于报错。
pub fn parse_manifest(origin: &str, text: &str) -> Result<Manifest> {
    toml::from_str(text).map_err(|e| Error::Parse {
        origin: origin.to_owned(),
        message: e.to_string(),
    })
}

/// 读项目的清单与个人覆盖。项目没有清单时返回 `None`。
pub fn load_project(_project_root: &Path) -> Result<Option<(Manifest, Option<Manifest>)>> {
    Err(Error::NotImplemented("manifest::load_project"))
}

/// 从目录现状反推清单（`fangcun init --from-existing`）。
pub fn infer_from_existing(_project_root: &Path) -> Result<Manifest> {
    Err(Error::NotImplemented("manifest::infer_from_existing"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SyncMode;

    /// 设计文档 7.4 的清单示例，原样照抄。示例改了，这里跟着改。
    const DESIGN_DOC_EXAMPLE: &str = r#"
# fangcun.toml —— 提交到 git
[project]
targets = ["claude-code", "codex"]
mode    = "link"                 # link 或 vendor

bundles = ["工程基线"]

[skills]
tdd             = { source = "github:obra/superpowers", track = "main" }
skill-b         = { source = "github:acme/skills", pin = "v2.1.0" }
project-harness = { source = "local:luyao-skills", pin = "4d9b1eb" }

[mcp.github]
command = "npx"
args    = ["-y", "@modelcontextprotocol/server-github"]
env     = { GITHUB_TOKEN = "secret:github-token" }   # 值在钥匙串

[instructions]
canonical = "AGENTS.md"          # 正本
mirror    = ["CLAUDE.md"]        # 生成为对正本的引用
"#;

    #[test]
    fn parses_the_design_doc_example() {
        let manifest = parse_manifest("7.4", DESIGN_DOC_EXAMPLE).expect("7.4 的示例应能解析");

        assert_eq!(manifest.project.targets.len(), 2);
        assert_eq!(manifest.project.mode, Some(SyncMode::Link));
        assert_eq!(manifest.project.bundles, ["工程基线"]);

        assert_eq!(manifest.skills.len(), 3);
        assert_eq!(manifest.skills["tdd"].track.as_deref(), Some("main"));
        assert_eq!(manifest.skills["skill-b"].pin.as_deref(), Some("v2.1.0"));

        let github = &manifest.mcp["github"];
        assert_eq!(github.command, "npx");
        assert_eq!(github.env["GITHUB_TOKEN"], "secret:github-token");

        let instructions = manifest.instructions.expect("示例里有指令文件一节");
        assert_eq!(instructions.canonical, "AGENTS.md");
        assert_eq!(instructions.mirror, ["CLAUDE.md"]);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(parse_manifest("test", "[project]\ntargets = []\nsurprise = 1\n").is_err());
    }
}
