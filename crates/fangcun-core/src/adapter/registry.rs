use std::collections::BTreeMap;

use super::schema::{Adapter, SCHEMA_VERSION};
use crate::model::ToolId;
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 编译时打进内核的适配器。新增工具时在这里登记。
const BUILTIN: &[(&str, &str)] = &[
    (
        "adapters/claude-code.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/claude-code.toml"
        )),
    ),
    (
        "adapters/codex.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/codex.toml"
        )),
    ),
    (
        "adapters/cursor.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/cursor.toml"
        )),
    ),
    (
        "adapters/copilot.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/copilot.toml"
        )),
    ),
    (
        "adapters/gemini.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/gemini.toml"
        )),
    ),
    (
        "adapters/opencode.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/opencode.toml"
        )),
    ),
    (
        "adapters/windsurf.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/windsurf.toml"
        )),
    ),
    (
        "adapters/trae.toml",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../adapters/trae.toml"
        )),
    ),
];

/// 所有已知工具的适配器，按工具标识索引。
#[derive(Debug, Clone)]
pub struct AdapterRegistry {
    adapters: BTreeMap<ToolId, Adapter>,
}

impl AdapterRegistry {
    /// 只含内置适配器。
    pub fn builtin() -> Result<Self> {
        let mut adapters = BTreeMap::new();
        for (origin, text) in BUILTIN {
            let adapter = parse(origin, text)?;
            adapters.insert(adapter.tool.id.clone(), adapter);
        }
        Ok(Self { adapters })
    }

    /// 内置适配器，再叠上 `~/.fangcun/adapters/` 里热更新下来的。
    ///
    /// 热更新还没实现，现在等同于 [`AdapterRegistry::builtin`]。
    pub fn load(_home: &FangcunHome) -> Result<Self> {
        Self::builtin()
    }

    pub fn get(&self, tool: &ToolId) -> Option<&Adapter> {
        self.adapters.get(tool)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Adapter> {
        self.adapters.values()
    }
}

fn parse(origin: &str, text: &str) -> Result<Adapter> {
    let adapter: Adapter = toml::from_str(text).map_err(|e| Error::Parse {
        origin: origin.to_owned(),
        message: e.to_string(),
    })?;
    if adapter.schema != SCHEMA_VERSION {
        return Err(Error::Parse {
            origin: origin.to_owned(),
            message: format!(
                "描述格式版本是 {}，内核只认 {SCHEMA_VERSION}",
                adapter.schema
            ),
        });
    }
    let mut adapter = adapter;
    apply_path_vars(&mut adapter, |name| std::env::var(name).ok());
    Ok(adapter)
}

/// 把路径开头的 `$名称` 换成环境变量的值，没有就用适配器里的默认值。
fn apply_path_vars(adapter: &mut Adapter, env: impl Fn(&str) -> Option<String>) {
    let vars: Vec<(String, String)> = adapter
        .path_vars
        .iter()
        .map(|(name, default)| {
            let value = env(name)
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| default.clone());
            (format!("${name}"), value)
        })
        .collect();
    if vars.is_empty() {
        return;
    }
    let fix = |path: &mut String| {
        for (token, value) in &vars {
            if let Some(rest) = path.strip_prefix(token.as_str()) {
                *path = format!("{value}{rest}");
            }
        }
    };
    for location in &mut adapter.skills {
        fix(&mut location.path);
        if let Some(lock) = &mut location.lock {
            fix(&mut lock.path);
        }
    }
    for location in &mut adapter.instructions {
        location.names.iter_mut().for_each(&fix);
        if let Some(extra) = &mut location.extra_names {
            fix(&mut extra.file);
        }
    }
    for location in &mut adapter.rules {
        fix(&mut location.dir);
    }
    for location in &mut adapter.mcp {
        fix(&mut location.file);
        if let Some(req) = &mut location.requires {
            fix(&mut req.file);
        }
    }
    for location in &mut adapter.observed {
        location.dir.iter_mut().for_each(&fix);
        location.file.iter_mut().for_each(&fix);
        if let Some(index) = &mut location.install_index {
            fix(&mut index.file);
        }
    }
    for switches in &mut adapter.skill_switches {
        fix(&mut switches.file);
        if let Some(req) = &mut switches.requires {
            fix(&mut req.file);
        }
    }
    if let Some(sessions) = &mut adapter.sessions {
        fix(&mut sessions.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::SupportLevel;

    #[test]
    fn builtin_adapters_parse_and_fully_support_the_first_two_tools() {
        let registry = AdapterRegistry::builtin().expect("内置适配器应能解析");
        for id in [ToolId::CLAUDE_CODE, ToolId::CODEX] {
            let adapter = registry.get(&ToolId::from(id)).expect("首批工具应有适配器");
            assert_eq!(adapter.tool.support, SupportLevel::Full);
        }
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text =
            "schema = 1\nsurprise = true\n[tool]\nid = \"x\"\nname = \"X\"\nsupport = \"full\"\n";
        assert!(parse("test", text).is_err());
    }

    #[test]
    fn path_vars_use_the_environment_then_the_default() {
        let text = "schema = 1\n[tool]\nid = \"x\"\nname = \"X\"\nsupport = \"full\"\n\
                    [path-vars]\nX_HOME = \"~/.x\"\n[sessions]\ndir = \"$X_HOME/sessions\"\nformat = \"codex-rollout-jsonl\"\n";
        let mut adapter: Adapter = toml::from_str(text).expect("解析");
        let mut other = adapter.clone();
        apply_path_vars(&mut adapter, |_| None);
        assert_eq!(adapter.sessions.expect("有会话").dir, "~/.x/sessions");
        apply_path_vars(&mut other, |_| Some("/opt/x".to_owned()));
        assert_eq!(other.sessions.expect("有会话").dir, "/opt/x/sessions");
    }

    #[test]
    fn newer_schema_is_rejected() {
        let text = "schema = 2\n[tool]\nid = \"x\"\nname = \"X\"\nsupport = \"scan-only\"\n";
        assert!(parse("test", text).is_err());
    }
}
