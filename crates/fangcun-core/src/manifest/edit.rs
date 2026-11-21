//! 改项目清单：用 `toml_edit` 改，保住人写的注释、顺序和其余的键（F3「文件即真相」）。
//!
//! 界面上「加入同步计划」的改动（6.8）在这里落成清单文本，再和同步一起出计划。

use serde::{Deserialize, Serialize};
use toml_edit::{Array, DocumentMut, Item, Table};
use ts_rs::TS;

use crate::model::AssetKind;
use crate::{Error, Result};

/// 对清单的一处改动。界面先暂存，点「同步 · N」时和同步一起出计划（6.8）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "op")]
#[ts(export)]
pub enum ManifestEdit {
    /// 在本项目停用（`[disable]` 加上这些名字，7.4）。
    Disable { kind: AssetKind, names: Vec<String> },
    /// 撤掉本项目的停用（`[disable]` 去掉这些名字）。
    Enable { kind: AssetKind, names: Vec<String> },
}

impl ManifestEdit {
    /// 计划里这一行的说明。
    pub fn describe(&self) -> String {
        let (verb, kind, names) = match self {
            Self::Disable { kind, names } => ("停用", kind, names),
            Self::Enable { kind, names } => ("恢复", kind, names),
        };
        let what = match kind {
            AssetKind::Skill => "skill",
            AssetKind::Mcp => "MCP",
            AssetKind::Instruction => "指令文件",
        };
        let list = if names.len() <= 3 {
            names.join("、")
        } else {
            format!("{} 等 {} 项", names[..2].join("、"), names.len())
        };
        format!("[disable] {verb} {what} {list}")
    }
}

/// 把改动用到清单文本上。返回新文本。
pub fn apply_edits(origin: &str, text: &str, edits: &[ManifestEdit]) -> Result<String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Error::Parse {
            origin: origin.to_owned(),
            message: e.to_string(),
        })?;
    for edit in edits {
        let (kind, names, add) = match edit {
            ManifestEdit::Disable { kind, names } => (kind, names, true),
            ManifestEdit::Enable { kind, names } => (kind, names, false),
        };
        let key = match kind {
            AssetKind::Skill => "skills",
            AssetKind::Mcp => "mcp",
            AssetKind::Instruction => {
                return Err(Error::Parse {
                    origin: origin.to_owned(),
                    message: "指令文件不能按项目停用".to_owned(),
                });
            }
        };
        let list = disable_list(&mut doc, key, add);
        let Some(list) = list else {
            continue;
        };
        for name in names {
            let has = list.iter().any(|v| v.as_str() == Some(name.as_str()));
            if add && !has {
                list.push(name.as_str());
            } else if !add && has {
                list.retain(|v| v.as_str() != Some(name.as_str()));
            }
        }
    }
    // 去掉改空了的 [disable]，免得清单里留一节空表。
    let empty = doc
        .get("disable")
        .and_then(|d| d.as_table_like())
        .is_some_and(|t| {
            t.iter()
                .all(|(_, v)| v.as_array().is_some_and(|a| a.is_empty()))
        });
    if empty {
        doc.remove("disable");
    }
    Ok(doc.to_string())
}

/// `[disable]` 里 `skills` 或 `mcp` 那个数组。`create` 时缺的建出来。
fn disable_list<'d>(doc: &'d mut DocumentMut, key: &str, create: bool) -> Option<&'d mut Array> {
    if create && doc.get("disable").is_none() {
        doc.insert("disable", Item::Table(Table::new()));
    }
    let table = doc.get_mut("disable")?.as_table_like_mut()?;
    if create && table.get(key).is_none() {
        table.insert(key, toml_edit::value(Array::new()));
    }
    table.get_mut(key)?.as_array_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_keeps_comments_and_enable_removes_the_empty_section() {
        let text = "# 团队清单\n[project]\ntargets = [\"claude-code\"]\n\n[skills]\ntdd = { source = \"github:a/b\" } # 测试\n";
        let added = apply_edits(
            "t",
            text,
            &[ManifestEdit::Disable {
                kind: AssetKind::Skill,
                names: vec!["lark-doc".into(), "lark-im".into()],
            }],
        )
        .unwrap();
        assert!(added.starts_with(text), "原文不动，只在后面加：{added}");
        assert!(added.contains("[disable]\nskills = [\"lark-doc\", \"lark-im\"]"));
        let parsed = crate::manifest::parse_manifest("t", &added).unwrap();
        assert_eq!(parsed.disable.skills, ["lark-doc", "lark-im"]);

        let removed = apply_edits(
            "t",
            &added,
            &[ManifestEdit::Enable {
                kind: AssetKind::Skill,
                names: vec!["lark-doc".into(), "lark-im".into()],
            }],
        )
        .unwrap();
        assert_eq!(removed, text);
    }

    #[test]
    fn disable_twice_does_not_duplicate() {
        let text = "[project]\ntargets = []\n\n[disable]\nmcp = [\"playwright\"]\n";
        let out = apply_edits(
            "t",
            text,
            &[ManifestEdit::Disable {
                kind: AssetKind::Mcp,
                names: vec!["playwright".into()],
            }],
        )
        .unwrap();
        assert_eq!(out, text);
    }
}
