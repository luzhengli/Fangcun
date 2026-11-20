//! 按项目停用（7.4、M1-18）：清单的 `[disable]` 写进各工具自己的开关。
//!
//! - skill：适配器 `[[skill-switches]]` 里项目层那份（个人覆盖写个人层那份），例如 Claude Code
//!   `.claude/settings.json` 的 `skillOverrides`、Codex `.codex/config.toml` 的 `[[skills.config]]`。
//! - MCP：有 `[[mcp-switches]]` 的写那张名字表（Claude Code `~/.claude.json` 里这个项目的
//!   `disabledMcpServers`，只在本机生效，9.3）；没有的写项目层服务表里这一项的停用字段（Codex）。
//!
//! 通配在同步时按工具实际读到的名字展开。写过的每一项记进台账，从清单里删掉时只撤自己写的、
//! 而且值还是方寸写的那个的项，别人改过的不动。

use std::collections::BTreeSet;

use serde_json::Value;

use super::{ConfigDoc, KeyKind, OwnedKey, Run};
use crate::Result;
use crate::adapter::{Adapter, FileFormat, McpSwitches, MergeMode, SkillSwitches, SwitchShape};
use crate::manifest::glob;
use crate::model::{PlanNote, Scope, ToolId};

/// 一个工具在这个项目实际读到的名字，展开通配用。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Visible {
    pub skills: Vec<String>,
    pub mcp: Vec<String>,
}

/// 要写的一项开关。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Want {
    tool: ToolId,
    kind: KeyKind,
    /// 适配器里的写法，也是台账里记的 `file`。
    file: String,
    name: String,
}

impl Run<'_, '_> {
    pub(super) fn sync_disable(&mut self) -> Result<()> {
        let input = self.input;
        let team = input.project.team_declarations().disable;
        let local = input.project.local_declarations().disable;
        let mut wants: Vec<Want> = Vec::new();

        for tool in &input.project.manifest.project.targets {
            let Some(adapter) = input.adapters.get(tool) else {
                continue;
            };
            if local.is_empty() {
                continue;
            }
            let tool_name = adapter.tool.name.as_str();
            let supported = adapter
                .capabilities
                .as_ref()
                .and_then(|c| c.per_project_disable)
                == Some(true);
            if !supported {
                self.notes.push(PlanNote::info(format!(
                    "{tool_name} 不支持按项目停用，[disable] 对它不生效"
                )));
                continue;
            }
            let visible = input.visible.get(tool).cloned().unwrap_or_default();
            let team_skills = expand_names(&team.skills, &visible.skills);
            let local_skills = expand_names(&local.skills, &visible.skills);
            let team_mcp = expand_names(&team.mcp, &visible.mcp);
            let local_mcp = expand_names(&local.mcp, &visible.mcp);
            let personal_skills: Vec<String> = local_skills
                .iter()
                .filter(|n| !team_skills.contains(n))
                .cloned()
                .collect();
            let personal_mcp: Vec<String> = local_mcp
                .iter()
                .filter(|n| !team_mcp.contains(n))
                .cloned()
                .collect();
            let mut no_personal: Vec<String> = Vec::new();

            // skill：全队的写项目层那份，个人的写个人层那份。
            for (names, scope) in [
                (&team_skills, Scope::Project),
                (&personal_skills, Scope::Personal),
            ] {
                if names.is_empty() {
                    continue;
                }
                let Some(switches) = adapter.skill_switches.iter().find(|s| s.scope == scope)
                else {
                    if scope == Scope::Personal {
                        no_personal.extend(names.iter().cloned());
                    } else {
                        self.notes.push(PlanNote::info(format!(
                            "{tool_name} 的适配器没写项目层的 skill 开关，停用 skill 没有写"
                        )));
                    }
                    continue;
                };
                self.note_unmet(adapter, switches.requires.as_ref());
                for name in names {
                    wants.push(Want {
                        tool: tool.clone(),
                        kind: KeyKind::SkillOff,
                        file: switches.file.clone(),
                        name: name.clone(),
                    });
                }
            }

            // MCP：有名字表的全写进名字表（本机生效）；没有的写项目层服务表的停用字段，个人的不支持。
            if !local_mcp.is_empty() {
                if let Some(switches) = adapter.mcp_switches.first() {
                    for name in &local_mcp {
                        wants.push(Want {
                            tool: tool.clone(),
                            kind: KeyKind::McpOff,
                            file: switches.file.clone(),
                            name: name.clone(),
                        });
                    }
                } else if let Some((file, _, _)) = mcp_field_target(adapter) {
                    let location = adapter.mcp.iter().find(|l| l.file == file);
                    self.note_unmet(adapter, location.and_then(|l| l.requires.as_ref()));
                    for name in &team_mcp {
                        wants.push(Want {
                            tool: tool.clone(),
                            kind: KeyKind::McpField,
                            file: file.clone(),
                            name: name.clone(),
                        });
                    }
                    no_personal.extend(personal_mcp.iter().cloned());
                } else {
                    self.notes.push(PlanNote::info(format!(
                        "{tool_name} 没有按项目停用 MCP 的开关，[disable] 里的 MCP 对它不生效"
                    )));
                }
            }
            if !no_personal.is_empty() {
                self.notes.push(PlanNote::info(format!(
                    "{tool_name} 项目里没有不提交的配置，个人覆盖里的停用对它不生效：{}",
                    no_personal.join("、")
                )));
            }
        }

        // 写上要的，撤掉台账里有、现在不要的。
        for want in &wants {
            self.apply_switch(want, true)?;
        }
        let stale: Vec<Want> = self
            .old
            .keys
            .iter()
            .filter(|k| !k.kind.is_mcp())
            .filter_map(|k| {
                Some(Want {
                    tool: k.tool.clone()?,
                    kind: k.kind,
                    file: k.file.clone(),
                    name: k.asset.clone(),
                })
            })
            .filter(|w| !wants.contains(w))
            .collect();
        for want in &stale {
            self.apply_switch(want, false)?;
        }
        Ok(())
    }

    /// 工具只在满足条件时才读这份配置（例如 Codex 只读受信任项目的），不满足时记一条提示。
    fn note_unmet(&mut self, adapter: &Adapter, requires: Option<&crate::adapter::Requirement>) {
        let Some(req) = requires else {
            return;
        };
        let home = self.input.user_home;
        if crate::scanner::requirement_met(req, home, Some(self.input.root)) {
            return;
        }
        let text = format!(
            "{}：{}，写进去的停用要等它满足后才生效",
            adapter.tool.name, req.unmet
        );
        if !self.notes.iter().any(|n| n.text == text) {
            self.notes.push(PlanNote::warning(text));
        }
    }

    /// 写上（`on = true`）或撤掉一项开关，并记进新台账。
    fn apply_switch(&mut self, want: &Want, on: bool) -> Result<()> {
        let input = self.input;
        let Some(adapter) = input.adapters.get(&want.tool) else {
            return Ok(());
        };
        let Some(spot) = Spot::find(adapter, want) else {
            return Ok(());
        };
        let Some(path) = crate::adapter::expand(&want.file, input.user_home, Some(input.root))
        else {
            return Ok(());
        };
        let Some(mut pending) = self.take_doc(&path, spot.format(), &want.file) else {
            return Ok(());
        };
        let project = input.root.to_string_lossy().into_owned();
        let display = spot.display(&want.name);
        if on {
            if spot.set(&mut pending.doc, &want.name, &project) {
                pending.rows.push(format!("停用 {display}"));
            }
            self.new.keys.push(OwnedKey {
                file: want.file.clone(),
                key: display,
                asset: want.name.clone(),
                kind: want.kind,
                tool: Some(want.tool.clone()),
            });
        } else if spot.unset(&mut pending.doc, &want.name, &project) {
            pending.rows.push(format!("撤销停用 {display}"));
        }
        self.put_doc(path, pending);
        Ok(())
    }
}

/// 展开通配：带 `*` 的按读到的名字匹配，不带的照写（还没装上的也先停用）。
fn expand_names(patterns: &[String], visible: &[String]) -> Vec<String> {
    let mut out = BTreeSet::new();
    for pattern in patterns {
        if pattern.contains('*') {
            out.extend(visible.iter().filter(|n| glob(pattern, n)).cloned());
        } else {
            out.insert(pattern.clone());
        }
    }
    out.into_iter().collect()
}

/// 没有 MCP 名字表时，按项目停用写在哪：项目层服务表所在的文件、键路径、停用字段。
/// 要求工具按字段合并同名服务（只写一个停用字段，别的沿用全局那份）。
fn mcp_field_target(adapter: &Adapter) -> Option<(String, Vec<String>, String)> {
    if adapter.precedence.mcp_merge != MergeMode::Fields {
        return None;
    }
    let field = adapter.mcp_fields.as_ref()?.enabled.clone()?;
    let location = adapter
        .mcp
        .iter()
        .find(|l| l.scope == Scope::Project && !l.file.starts_with(['/', '~', '$']))?;
    Some((location.file.clone(), location.key.clone(), field))
}

/// 一项开关落在配置里的哪个位置、怎么写。
enum Spot<'a> {
    Skill(&'a SkillSwitches),
    McpList(&'a McpSwitches),
    McpField {
        format: FileFormat,
        key: Vec<String>,
        field: String,
    },
}

impl<'a> Spot<'a> {
    fn find(adapter: &'a Adapter, want: &Want) -> Option<Self> {
        match want.kind {
            KeyKind::SkillOff => adapter
                .skill_switches
                .iter()
                .find(|s| s.file == want.file)
                .map(Spot::Skill),
            KeyKind::McpOff => adapter
                .mcp_switches
                .iter()
                .find(|s| s.file == want.file)
                .map(Spot::McpList),
            KeyKind::McpField => {
                let (file, key, field) = mcp_field_target(adapter)?;
                let format = adapter.mcp.iter().find(|l| l.file == file)?.format;
                (file == want.file).then_some(Spot::McpField { format, key, field })
            }
            KeyKind::Mcp => None,
        }
    }

    fn format(&self) -> FileFormat {
        match self {
            Spot::Skill(s) => s.format,
            Spot::McpList(s) => s.format,
            Spot::McpField { format, .. } => *format,
        }
    }

    /// 计划和台账里的写法，例如 `skillOverrides.lark-doc`、`skills.config[name=lark-doc]`。
    fn display(&self, name: &str) -> String {
        match self {
            Spot::Skill(s) => match s.shape {
                SwitchShape::Map => format!("{}.{name}", s.key.join(".")),
                SwitchShape::List => format!(
                    "{}[{}={name}]",
                    s.key.join("."),
                    s.name_field.as_deref().unwrap_or("name")
                ),
            },
            Spot::McpList(s) => format!("{}[{name}]", s.key.join(".")),
            Spot::McpField { key, field, .. } => format!("{}.{name}.{field}", key.join(".")),
        }
    }

    /// 写上。返回有没有改动。
    fn set(&self, doc: &mut ConfigDoc, name: &str, project: &str) -> bool {
        match self {
            Spot::Skill(s) => match s.shape {
                SwitchShape::Map => {
                    let Some(off) = s.off_values.first() else {
                        return false;
                    };
                    let Some(mut map) = object_at(doc, &s.key, true) else {
                        return false;
                    };
                    map.set_str(name, off)
                }
                SwitchShape::List => {
                    let (Some(name_field), Some(enabled_field)) = (&s.name_field, &s.enabled_field)
                    else {
                        return false;
                    };
                    list_set_off(doc, &s.key, name_field, enabled_field, name)
                }
            },
            Spot::McpList(s) => {
                let key = with_project(&s.key, project);
                string_list(doc, &key, name, true)
            }
            Spot::McpField { key, field, .. } => {
                let mut path = key.clone();
                path.push(name.to_owned());
                let Some(mut map) = object_at(doc, &path, true) else {
                    return false;
                };
                map.set_bool(field, false)
            }
        }
    }

    /// 撤掉。值已经被人改过的不动。返回有没有改动。
    fn unset(&self, doc: &mut ConfigDoc, name: &str, project: &str) -> bool {
        match self {
            Spot::Skill(s) => match s.shape {
                SwitchShape::Map => {
                    let Some(off) = s.off_values.first() else {
                        return false;
                    };
                    let Some(mut map) = object_at(doc, &s.key, false) else {
                        return false;
                    };
                    map.remove_if_str(name, off)
                }
                SwitchShape::List => {
                    let (Some(name_field), Some(enabled_field)) = (&s.name_field, &s.enabled_field)
                    else {
                        return false;
                    };
                    list_remove_off(doc, &s.key, name_field, enabled_field, name)
                }
            },
            Spot::McpList(s) => {
                let key = with_project(&s.key, project);
                string_list(doc, &key, name, false)
            }
            Spot::McpField { key, field, .. } => {
                let mut path = key.clone();
                path.push(name.to_owned());
                let removed = object_at(doc, &path, false)
                    .is_some_and(|mut map| map.remove_if_bool(field, false));
                if removed && object_at(doc, &path, false).is_some_and(|map| map.is_empty()) {
                    remove_key(doc, key, name);
                }
                removed
            }
        }
    }
}

fn with_project(key: &[String], project: &str) -> Vec<String> {
    key.iter()
        .map(|segment| segment.replace("{project}", project))
        .collect()
}

// ───────────── JSON 与 TOML 上的几种小操作 ─────────────

/// 一个对象（JSON 对象或 TOML 表）。
enum Obj<'d> {
    Json(&'d mut serde_json::Map<String, Value>),
    Toml(&'d mut dyn toml_edit::TableLike),
}

impl Obj<'_> {
    fn set_str(&mut self, key: &str, value: &str) -> bool {
        match self {
            Obj::Json(map) => {
                if map.get(key).and_then(Value::as_str) == Some(value) {
                    return false;
                }
                map.insert(key.to_owned(), Value::String(value.to_owned()));
                true
            }
            Obj::Toml(table) => {
                if table.get(key).and_then(|v| v.as_str()) == Some(value) {
                    return false;
                }
                table.insert(key, toml_edit::value(value));
                true
            }
        }
    }

    fn set_bool(&mut self, key: &str, value: bool) -> bool {
        match self {
            Obj::Json(map) => {
                if map.get(key).and_then(Value::as_bool) == Some(value) {
                    return false;
                }
                map.insert(key.to_owned(), Value::Bool(value));
                true
            }
            Obj::Toml(table) => {
                if table.get(key).and_then(|v| v.as_bool()) == Some(value) {
                    return false;
                }
                table.insert(key, toml_edit::value(value));
                true
            }
        }
    }

    fn remove_if_str(&mut self, key: &str, value: &str) -> bool {
        match self {
            Obj::Json(map) => {
                map.get(key).and_then(Value::as_str) == Some(value)
                    && map.shift_remove(key).is_some()
            }
            Obj::Toml(table) => {
                table.get(key).and_then(|v| v.as_str()) == Some(value)
                    && table.remove(key).is_some()
            }
        }
    }

    fn remove_if_bool(&mut self, key: &str, value: bool) -> bool {
        match self {
            Obj::Json(map) => {
                map.get(key).and_then(Value::as_bool) == Some(value)
                    && map.shift_remove(key).is_some()
            }
            Obj::Toml(table) => {
                table.get(key).and_then(|v| v.as_bool()) == Some(value)
                    && table.remove(key).is_some()
            }
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Obj::Json(map) => map.is_empty(),
            Obj::Toml(table) => table.is_empty(),
        }
    }
}

/// 键路径上的对象。`create` 时缺的一路建出来；路上碰到不是对象的值时不动它，返回空。
fn object_at<'d>(doc: &'d mut ConfigDoc, path: &[String], create: bool) -> Option<Obj<'d>> {
    match doc {
        ConfigDoc::Json(root) => {
            let mut node = root;
            for key in path {
                let map = node.as_object_mut()?;
                node = if create {
                    map.entry(key.clone())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()))
                } else {
                    map.get_mut(key)?
                };
            }
            node.as_object_mut().map(Obj::Json)
        }
        ConfigDoc::Toml(doc) => {
            let mut table: &mut dyn toml_edit::TableLike = doc.as_table_mut();
            for key in path {
                if create && table.get(key).is_none() {
                    let mut t = toml_edit::Table::new();
                    t.set_implicit(true);
                    table.insert(key, toml_edit::Item::Table(t));
                }
                table = table.get_mut(key)?.as_table_like_mut()?;
            }
            Some(Obj::Toml(table))
        }
    }
}

/// 删掉 `path` 下的 `name` 一项。
fn remove_key(doc: &mut ConfigDoc, path: &[String], name: &str) {
    match object_at(doc, path, false) {
        Some(Obj::Json(map)) => {
            map.shift_remove(name);
        }
        Some(Obj::Toml(table)) => {
            table.remove(name);
        }
        None => {}
    }
}

/// 字符串数组里加上或去掉一个名字。返回有没有改动。
fn string_list(doc: &mut ConfigDoc, key: &[String], name: &str, add: bool) -> bool {
    let Some((last, parent)) = key.split_last() else {
        return false;
    };
    match doc {
        ConfigDoc::Json(_) => {
            let Some(Obj::Json(map)) = object_at(doc, parent, add) else {
                return false;
            };
            if add {
                let list = map
                    .entry(last.clone())
                    .or_insert_with(|| Value::Array(Vec::new()));
                let Some(items) = list.as_array_mut() else {
                    return false;
                };
                if items.iter().any(|v| v.as_str() == Some(name)) {
                    return false;
                }
                items.push(Value::String(name.to_owned()));
                true
            } else {
                let Some(items) = map.get_mut(last).and_then(Value::as_array_mut) else {
                    return false;
                };
                let before = items.len();
                items.retain(|v| v.as_str() != Some(name));
                items.len() != before
            }
        }
        ConfigDoc::Toml(_) => {
            let Some(Obj::Toml(table)) = object_at(doc, parent, add) else {
                return false;
            };
            if add && table.get(last).is_none() {
                table.insert(last, toml_edit::value(toml_edit::Array::new()));
            }
            let Some(items) = table.get_mut(last).and_then(|i| i.as_array_mut()) else {
                return false;
            };
            let has = items.iter().any(|v| v.as_str() == Some(name));
            if add && !has {
                items.push(name);
                true
            } else if !add && has {
                items.retain(|v| v.as_str() != Some(name));
                true
            } else {
                false
            }
        }
    }
}

/// 开关数组里这个名字那一项设成停用，没有就加一项。返回有没有改动。
fn list_set_off(
    doc: &mut ConfigDoc,
    key: &[String],
    name_field: &str,
    enabled_field: &str,
    name: &str,
) -> bool {
    let Some((last, parent)) = key.split_last() else {
        return false;
    };
    match doc {
        ConfigDoc::Json(_) => {
            let Some(Obj::Json(map)) = object_at(doc, parent, true) else {
                return false;
            };
            let list = map
                .entry(last.clone())
                .or_insert_with(|| Value::Array(Vec::new()));
            let Some(items) = list.as_array_mut() else {
                return false;
            };
            if let Some(entry) = items
                .iter_mut()
                .find(|e| e.get(name_field).and_then(Value::as_str) == Some(name))
            {
                if entry.get(enabled_field).and_then(Value::as_bool) == Some(false) {
                    return false;
                }
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert(enabled_field.to_owned(), Value::Bool(false));
                }
                return true;
            }
            let mut entry = serde_json::Map::new();
            entry.insert(name_field.to_owned(), Value::String(name.to_owned()));
            entry.insert(enabled_field.to_owned(), Value::Bool(false));
            items.push(Value::Object(entry));
            true
        }
        ConfigDoc::Toml(_) => {
            let Some(Obj::Toml(table)) = object_at(doc, parent, true) else {
                return false;
            };
            if table.get(last).is_none() {
                table.insert(
                    last,
                    toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
                );
            }
            let Some(tables) = table.get_mut(last).and_then(|i| i.as_array_of_tables_mut()) else {
                return false;
            };
            if let Some(entry) = tables
                .iter_mut()
                .find(|t| t.get(name_field).and_then(|v| v.as_str()) == Some(name))
            {
                if entry.get(enabled_field).and_then(|v| v.as_bool()) == Some(false) {
                    return false;
                }
                entry.insert(enabled_field, toml_edit::value(false));
                return true;
            }
            let mut entry = toml_edit::Table::new();
            entry.insert(name_field, toml_edit::value(name));
            entry.insert(enabled_field, toml_edit::value(false));
            tables.push(entry);
            true
        }
    }
}

/// 撤掉开关数组里这个名字、而且还是停用的那一项。返回有没有改动。
fn list_remove_off(
    doc: &mut ConfigDoc,
    key: &[String],
    name_field: &str,
    enabled_field: &str,
    name: &str,
) -> bool {
    let Some((last, parent)) = key.split_last() else {
        return false;
    };
    match doc {
        ConfigDoc::Json(_) => {
            let Some(Obj::Json(map)) = object_at(doc, parent, false) else {
                return false;
            };
            let Some(items) = map.get_mut(last).and_then(Value::as_array_mut) else {
                return false;
            };
            let before = items.len();
            items.retain(|e| {
                !(e.get(name_field).and_then(Value::as_str) == Some(name)
                    && e.get(enabled_field).and_then(Value::as_bool) == Some(false))
            });
            items.len() != before
        }
        ConfigDoc::Toml(_) => {
            let Some(Obj::Toml(table)) = object_at(doc, parent, false) else {
                return false;
            };
            let Some(tables) = table.get_mut(last).and_then(|i| i.as_array_of_tables_mut()) else {
                return false;
            };
            let before = tables.len();
            tables.retain(|t| {
                !(t.get(name_field).and_then(|v| v.as_str()) == Some(name)
                    && t.get(enabled_field).and_then(|v| v.as_bool()) == Some(false))
            });
            let changed = tables.len() != before;
            if changed && tables.is_empty() {
                table.remove(last);
            }
            changed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_stars_only() {
        assert!(glob("lark-*", "lark-doc"));
        assert!(glob("*-doc", "lark-doc"));
        assert!(glob("l*k-*c", "lark-doc"));
        assert!(glob("*", "anything"));
        assert!(!glob("lark-*", "larkdoc"));
        assert!(!glob("lark-*x", "lark-doc"));
        assert!(!glob("ab*ba", "aba"));
    }

    #[test]
    fn literal_names_stay_even_when_not_seen() {
        let seen = vec![
            "lark-doc".to_owned(),
            "lark-im".to_owned(),
            "tdd".to_owned(),
        ];
        assert_eq!(
            expand_names(&["lark-*".to_owned(), "playwright".to_owned()], &seen),
            ["lark-doc", "lark-im", "playwright"]
        );
    }

    #[test]
    fn toml_switch_list_round_trip_keeps_other_entries() {
        let text = "[[skills.config]]\nname = \"mine\"\nenabled = false\n";
        let mut doc = ConfigDoc::parse(FileFormat::Toml, Some(text)).expect("能读");
        let key = vec!["skills".to_owned(), "config".to_owned()];
        assert!(list_set_off(&mut doc, &key, "name", "enabled", "lark-doc"));
        assert!(!list_set_off(&mut doc, &key, "name", "enabled", "lark-doc"));
        let written = doc.render();
        assert!(written.contains("name = \"lark-doc\""));
        assert!(list_remove_off(
            &mut doc, &key, "name", "enabled", "lark-doc"
        ));
        assert_eq!(doc.render(), text);
    }

    #[test]
    fn json_map_unset_leaves_values_changed_by_hand() {
        let mut doc =
            ConfigDoc::parse(FileFormat::Json, Some("{\"theme\": \"dark\"}")).expect("能读");
        let key = vec!["skillOverrides".to_owned()];
        assert!(
            object_at(&mut doc, &key, true)
                .expect("对象")
                .set_str("lark-doc", "off")
        );
        assert!(
            object_at(&mut doc, &key, true)
                .expect("对象")
                .set_str("tdd", "on")
        );
        let mut map = object_at(&mut doc, &key, false).expect("对象");
        assert!(map.remove_if_str("lark-doc", "off"));
        assert!(!map.remove_if_str("tdd", "off"), "人改成 on 的不动");
        assert!(doc.render().contains("\"theme\": \"dark\""));
    }

    #[test]
    fn json_string_list_under_project_key() {
        let mut doc = ConfigDoc::parse(FileFormat::Json, Some("{\"projects\": {}}")).expect("能读");
        let key = with_project(
            &[
                "projects".to_owned(),
                "{project}".to_owned(),
                "disabledMcpServers".to_owned(),
            ],
            "/p/demo",
        );
        assert!(string_list(&mut doc, &key, "playwright", true));
        assert!(!string_list(&mut doc, &key, "playwright", true));
        assert!(doc.render().contains("\"/p/demo\""));
        assert!(string_list(&mut doc, &key, "playwright", false));
    }
}
