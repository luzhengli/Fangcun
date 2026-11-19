//! 把一份新清单写成文本，写法照设计文档 7.4 的示例：`[project]` 在前，skill 一行一项的行内表，
//! MCP 一个服务一节。
//!
//! 只用来生成新文件（`fangcun init`）。改已有的清单要保住人写的注释和顺序，不走这里。

use std::fmt::Write as _;

use super::Manifest;

/// 生成清单文本。读回来和传进来的一样（有测试守着）。
pub fn render_manifest(manifest: &Manifest) -> String {
    let mut out = String::new();
    let project = &manifest.project;
    out.push_str("[project]\n");
    let targets: Vec<&str> = project.targets.iter().map(|t| t.as_str()).collect();
    let _ = writeln!(out, "targets = {}", array(&targets));
    if let Some(mode) = project.mode {
        let mode = match mode {
            crate::model::SyncMode::Link => "link",
            crate::model::SyncMode::Vendor => "vendor",
        };
        let _ = writeln!(out, "mode    = {}", string(mode));
    }
    if let Some(budget) = project.budget {
        let _ = writeln!(out, "budget  = {budget}");
    }
    if !project.bundles.is_empty() {
        let bundles: Vec<&str> = project.bundles.iter().map(String::as_str).collect();
        let _ = writeln!(out, "\nbundles = {}", array(&bundles));
    }

    if !manifest.skills.is_empty() {
        out.push_str("\n[skills]\n");
        let width = manifest.skills.keys().map(|k| key(k).chars().count()).max();
        for (name, decl) in &manifest.skills {
            let mut fields = vec![format!("source = {}", string(&decl.source))];
            for (label, value) in [
                ("pin", &decl.pin),
                ("track", &decl.track),
                ("path", &decl.path),
            ] {
                if let Some(value) = value {
                    fields.push(format!("{label} = {}", string(value)));
                }
            }
            let name = key(name);
            let pad = width.unwrap_or(0).saturating_sub(name.chars().count());
            let _ = writeln!(
                out,
                "{name}{} = {{ {} }}",
                " ".repeat(pad),
                fields.join(", ")
            );
        }
    }

    for (name, decl) in &manifest.mcp {
        let _ = writeln!(out, "\n[mcp.{}]", key(name));
        let _ = writeln!(out, "command = {}", string(&decl.command));
        if !decl.args.is_empty() {
            let args: Vec<&str> = decl.args.iter().map(String::as_str).collect();
            let _ = writeln!(out, "args    = {}", array(&args));
        }
        if !decl.env.is_empty() {
            let env: Vec<String> = decl
                .env
                .iter()
                .map(|(k, v)| format!("{} = {}", key(k), string(v)))
                .collect();
            let _ = writeln!(out, "env     = {{ {} }}", env.join(", "));
        }
    }

    if let Some(instructions) = &manifest.instructions {
        out.push_str("\n[instructions]\n");
        let _ = writeln!(out, "canonical = {}", string(&instructions.canonical));
        if !instructions.mirror.is_empty() {
            let mirror: Vec<&str> = instructions.mirror.iter().map(String::as_str).collect();
            let _ = writeln!(out, "mirror    = {}", array(&mirror));
        }
    }

    let disable = &manifest.disable;
    if !disable.is_empty() {
        out.push_str("\n[disable]\n");
        if !disable.skills.is_empty() {
            let skills: Vec<&str> = disable.skills.iter().map(String::as_str).collect();
            let _ = writeln!(out, "skills = {}", array(&skills));
        }
        if !disable.mcp.is_empty() {
            let mcp: Vec<&str> = disable.mcp.iter().map(String::as_str).collect();
            let _ = writeln!(out, "mcp    = {}", array(&mcp));
        }
    }
    out
}

/// TOML 基本字符串，按需转义。
fn string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

fn array(items: &[&str]) -> String {
    let items: Vec<String> = items.iter().map(|s| string(s)).collect();
    format!("[{}]", items.join(", "))
}

/// 键：只含字母、数字、`-`、`_` 时裸写，否则加引号。
fn key(name: &str) -> String {
    let bare = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if bare { name.to_owned() } else { string(name) }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::manifest::{
        DisableSection, InstructionsDecl, McpDecl, ProjectSection, SkillDecl, parse_manifest,
    };
    use crate::model::{SyncMode, ToolId};

    #[test]
    fn rendered_manifest_reads_back_the_same() {
        let mut skills = BTreeMap::new();
        skills.insert(
            "tdd".to_owned(),
            SkillDecl {
                source: "github:obra/superpowers".to_owned(),
                pin: None,
                track: Some("main".to_owned()),
                path: None,
            },
        );
        skills.insert(
            "带空格 的名字".to_owned(),
            SkillDecl {
                source: "local:luyao-skills".to_owned(),
                pin: Some("4d9b1eb".to_owned()),
                track: None,
                path: Some("skills/a \"b\"".to_owned()),
            },
        );
        let mut env = BTreeMap::new();
        env.insert("GITHUB_TOKEN".to_owned(), "secret:github-token".to_owned());
        let mut mcp = BTreeMap::new();
        mcp.insert(
            "github".to_owned(),
            McpDecl {
                command: "npx".to_owned(),
                args: vec![
                    "-y".to_owned(),
                    "@modelcontextprotocol/server-github".to_owned(),
                ],
                env,
            },
        );
        mcp.insert(
            "a.b".to_owned(),
            McpDecl {
                command: "uvx".to_owned(),
                args: Vec::new(),
                env: BTreeMap::new(),
            },
        );
        let manifest = Manifest {
            project: ProjectSection {
                targets: vec![ToolId::from("claude-code"), ToolId::from("codex")],
                mode: Some(SyncMode::Link),
                bundles: vec!["工程基线".to_owned()],
                budget: Some(20000),
            },
            skills,
            mcp,
            instructions: Some(InstructionsDecl {
                canonical: "AGENTS.md".to_owned(),
                mirror: vec!["CLAUDE.md".to_owned()],
            }),
            disable: DisableSection {
                skills: vec!["lark-*".to_owned()],
                mcp: Vec::new(),
            },
            ..Manifest::default()
        };
        let text = render_manifest(&manifest);
        assert_eq!(parse_manifest("rendered", &text).unwrap(), manifest);
        assert!(
            text.contains("tdd       = { source = \"github:obra/superpowers\", track = \"main\" }"),
            "{text}"
        );
    }

    #[test]
    fn an_empty_manifest_still_names_its_targets() {
        let manifest = Manifest {
            project: ProjectSection {
                targets: vec![ToolId::from("codex")],
                ..ProjectSection::default()
            },
            ..Manifest::default()
        };
        assert_eq!(
            render_manifest(&manifest),
            "[project]\ntargets = [\"codex\"]\n"
        );
    }
}
