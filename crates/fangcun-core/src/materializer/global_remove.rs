//! 从全局移除（M1-34、6.6、4.3「同名冲突」）：把全局层的 skill 链接或目录、MCP 配置项拿掉。
//!
//! 规则（7.5、9.3「全局层写不写」）：
//! - 只对已收编进资产库的项开放：skill 看资产库里有没有它这份内容，MCP 看它换成 `secret:`
//!   引用后的那一版在不在库里。没收编的报 [`Error::NotAdopted`]，先 `fangcun adopt`。
//! - 只动选中的那个工具的全局位置（6.6 的目标工具切换），资产库里的正本不动。
//! - skill 是软链就删链接，是实体目录就逐个删里面的文件；原内容都进快照，回滚逐字节还原。
//! - MCP 在共享的配置文件里按键删，别的键不动；同一个文件只写一次。配置里用户自己写的明文
//!   密钥在快照里换成占位，回滚时从钥匙串取回（规则 9）。
//! - 计划里列出受影响的项目：近 30 天用到它的，和项目层有同名项、移除后那一份单独生效的。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::ledger::Adopted;
use super::sync::ConfigDoc;
use super::{PlanBuilder, State, WriteScope};
use crate::adapter::{Adapter, AdapterRegistry};
use crate::global::{GlobalItemRef, UseMark};
use crate::library::Library;
use crate::library::mcp::McpSpec;
use crate::model::{AssetKind, PlanNote, Scope, TokenEstimate};
use crate::paths::FangcunHome;
use crate::scanner::fsutil::display_path;
use crate::scanner::{McpEntry, Scan, SkillCopy, format_k};
use crate::secrets::SecretStore;
use crate::{Error, Result};

pub struct RemoveInput<'a> {
    /// 要动哪几个工具的全局层。界面传当前选中的工具；命令行不写时是全部完整支持的工具。
    pub tools: Vec<&'a Adapter>,
    pub adapters: &'a AdapterRegistry,
    pub items: &'a [GlobalItemRef],
    pub scan: &'a Scan,
    pub home: &'a FangcunHome,
    pub user_home: &'a Path,
    pub scope: &'a WriteScope,
    pub library: &'a Library,
    pub secrets: &'a dyn SecretStore,
}

/// 选中的一项落到某个工具全局层里的哪一处。
enum Hit<'a> {
    Skill(&'a SkillCopy),
    Mcp(&'a McpEntry, &'a Adapter),
}

pub fn plan(input: &RemoveInput<'_>, builder: &mut PlanBuilder) -> Result<()> {
    if input.items.is_empty() {
        return Err(Error::NotFound("要从全局移除的项".to_owned()));
    }
    // 1. 每一项在各工具全局层里的位置。名字可以带 `*` 通配（例如 `lark-*`）。
    let mut hits: Vec<(String, Hit<'_>)> = Vec::new();
    let mut missing = Vec::new();
    let mut builtin = BTreeSet::new();
    for item in input.items {
        let mut found = false;
        for adapter in &input.tools {
            let Some(global) = input.scan.global.get(&adapter.tool.id) else {
                continue;
            };
            if item.kind.is_none_or(|k| k == AssetKind::Skill) {
                for copy in global
                    .skills
                    .iter()
                    .filter(|s| s.scope == Scope::Global)
                    .filter(|s| crate::manifest::glob(&item.name, &s.name))
                {
                    found = true;
                    if copy
                        .source
                        .as_deref()
                        .is_some_and(|s| s.starts_with("builtin:"))
                    {
                        builtin.insert(copy.name.clone());
                        continue;
                    }
                    hits.push((copy.name.clone(), Hit::Skill(copy)));
                }
            }
            if item.kind.is_none_or(|k| k == AssetKind::Mcp) {
                for entry in global
                    .mcp
                    .iter()
                    .filter(|m| m.scope == Scope::Global)
                    .filter(|m| crate::manifest::glob(&item.name, &m.name))
                {
                    found = true;
                    hits.push((entry.name.clone(), Hit::Mcp(entry, adapter)));
                }
            }
        }
        if !found {
            missing.push(item.name.clone());
        }
    }
    if !missing.is_empty() {
        let tools: Vec<&str> = input.tools.iter().map(|a| a.tool.name.as_str()).collect();
        return Err(Error::NotFound(format!(
            "{} 的全局层里没有 {}",
            tools.join("、"),
            missing.join("、")
        )));
    }
    if hits.is_empty() {
        return Err(Error::NotFound(format!(
            "可以移除的项（{} 是工具自带的，方寸不管）",
            builtin.into_iter().collect::<Vec<_>>().join("、")
        )));
    }

    // 2. 只对已收编的开放（7.5）。
    let mut not_adopted = BTreeSet::new();
    for (name, hit) in &hits {
        let adopted = match hit {
            Hit::Skill(copy) => input.library.locate(&copy.hash)?.is_some(),
            Hit::Mcp(entry, _) => match &entry.command {
                Some(command) => {
                    let (spec, _) = McpSpec::from_config(name, command, &entry.args, &entry.env);
                    input.library.locate(&spec.hash()?)?.is_some()
                }
                None => false,
            },
        };
        if !adopted {
            not_adopted.insert(name.clone());
        }
    }
    if !not_adopted.is_empty() {
        let list: Vec<String> = not_adopted.into_iter().collect();
        return Err(Error::NotAdopted(list.join("、")));
    }

    // 3. 动作。
    let mut removed_dirs = BTreeSet::new();
    let mut removed_real = BTreeSet::new();
    let mut out_of_scope = BTreeSet::new();
    let mut docs: BTreeMap<PathBuf, (ConfigDoc, Vec<String>)> = BTreeMap::new();
    let mut resident: BTreeMap<&str, (BTreeSet<&Path>, u32)> = BTreeMap::new();
    let mut names = BTreeSet::new();
    for (name, hit) in &hits {
        match hit {
            Hit::Skill(copy) => {
                if input.scope.check(&copy.dir).is_err() {
                    out_of_scope.insert(display_path(&copy.dir, input.user_home));
                    continue;
                }
                names.insert(name.clone());
                let (dirs, total) = resident.entry(copy.tool.as_str()).or_default();
                if dirs.insert(copy.real_dir.as_path()) {
                    *total = total.saturating_add(copy.resident.0);
                }
                if !removed_dirs.insert(copy.dir.clone()) {
                    continue;
                }
                let short = &copy.hash.0[..copy.hash.0.len().min(7)];
                // 按磁盘现在的样子判断，不信扫描结果：收编过还没重新体检时，扫描里还是实体目录，
                // 逐个删的会是链接那头资产库里的文件。
                match State::read(&copy.dir)? {
                    State::Absent => continue,
                    State::Dir => {}
                    _ => {
                        builder.remove(
                            &copy.dir,
                            format!("移出全局层（软链指向 {name}@{short}），资产库保留正本"),
                        )?;
                        continue;
                    }
                }
                {
                    removed_real.insert(copy.real_dir.clone());
                    builder.set_quiet(true);
                    let files = super::adopt::remove_tree(builder, &copy.dir);
                    builder.set_quiet(false);
                    let files = files?;
                    builder.remove(
                        &copy.dir,
                        format!(
                            "移出全局层（{name}@{short}，{files} 个文件已备份），资产库保留正本"
                        ),
                    )?;
                }
            }
            Hit::Mcp(entry, adapter) => {
                if input.scope.check(&entry.file).is_err() {
                    out_of_scope.insert(display_path(&entry.file, input.user_home));
                    continue;
                }
                let Some(location) = adapter.mcp.iter().find(|l| l.scope == Scope::Global) else {
                    continue;
                };
                names.insert(name.clone());
                if !docs.contains_key(&entry.file) {
                    let text = match std::fs::read_to_string(&entry.file) {
                        Ok(text) => Some(text),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                        Err(source) => {
                            return Err(Error::Io {
                                path: entry.file.clone(),
                                source,
                            });
                        }
                    };
                    let doc =
                        ConfigDoc::parse(location.format, text.as_deref()).map_err(|message| {
                            Error::Parse {
                                origin: entry.file.display().to_string(),
                                message,
                            }
                        })?;
                    docs.insert(entry.file.clone(), (doc, Vec::new()));
                }
                let Some((doc, removed)) = docs.get_mut(&entry.file) else {
                    continue;
                };
                if doc.remove(&location.key, name) {
                    removed.push(entry.key.clone());
                }
                // 用户自己写在配置里的明文：快照里换成占位，回滚从钥匙串取回。
                let (_, plain) = McpSpec::from_config(
                    name,
                    entry.command.as_deref().unwrap_or_default(),
                    &entry.args,
                    &entry.env,
                );
                for (secret, value) in plain {
                    match input.secrets.get(&secret)? {
                        Some(existing) if existing == value => builder.redact(&secret, value),
                        Some(_) => {
                            builder.note(PlanNote::danger(format!(
                                "钥匙串里的 {secret} 和 {} 里 {name} 的值不同，快照里没法不留明文。先用 fangcun secret set {secret} 统一",
                                display_path(&entry.file, input.user_home)
                            )));
                            builder.block();
                        }
                        None => {
                            builder.store_secret(&secret, value.clone());
                            builder.redact(&secret, value);
                        }
                    }
                }
            }
        }
    }
    for (file, (doc, removed)) in docs {
        if removed.is_empty() {
            continue;
        }
        builder.write_file(
            &file,
            doc.render().into_bytes(),
            format!("移出全局层（{}），资产库保留正本", removed.join("、")),
        )?;
    }

    // 收编台账里这些位置不再是受管链接。
    let mut ledger = Adopted::load(input.home)?;
    let before = ledger.links.len();
    ledger.links.retain(|l| !removed_dirs.contains(&l.path));
    if ledger.links.len() != before {
        builder.write_file(
            &Adopted::file(input.home),
            ledger.to_toml()?.into_bytes(),
            format!("收录台账：去掉 {} 个全局位置", before - ledger.links.len()),
        )?;
    }

    if builder.is_empty() && !out_of_scope.is_empty() {
        let list: Vec<&str> = out_of_scope.iter().map(String::as_str).collect();
        return Err(Error::OutOfScope(PathBuf::from(list.join("、"))));
    }

    // 4. 提示：受影响的项目、链到这些目录的别处、常驻的变化。
    let count = names.len();
    builder.set_title(if count == 1 {
        format!(
            "从全局层移除 {}",
            names.iter().next().cloned().unwrap_or_default()
        )
    } else {
        format!("从全局层移除 {count} 项")
    });
    affected(input, &names, builder);
    linked_from_elsewhere(input, &removed_dirs, &removed_real, builder);
    let total = input.scan.projects.len();
    for (tool, (_, tokens)) in &resident {
        if *tokens == 0 {
            continue;
        }
        let id = crate::model::ToolId((*tool).to_owned());
        let tool_name = input.scan.tool_name(&id).to_owned();
        builder.note(PlanNote::info(format!(
            "{tool_name}：{total} 个项目每个常驻 −{}（估算）。资产库里的正本不受影响",
            format_k(TokenEstimate(*tokens))
        )));
    }
    if !builtin.is_empty() {
        let list: Vec<String> = builtin.into_iter().collect();
        builder.note(PlanNote::info(format!(
            "{} 是工具自带的 skill，方寸不管，没有移除",
            list.join("、")
        )));
    }
    if !out_of_scope.is_empty() {
        let list: Vec<&str> = out_of_scope.iter().map(String::as_str).collect();
        builder.note(PlanNote::info(format!(
            "{} 处不在写入范围里（企业托管或系统目录），没有动：{}",
            list.len(),
            list.join("、")
        )));
    }
    Ok(())
}

/// 用到它的项目移除后就读不到了；项目层有同名项的，移除后那一份单独生效。
fn affected(input: &RemoveInput<'_>, names: &BTreeSet<String>, builder: &mut PlanBuilder) {
    let mut using: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut same: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for adapter in &input.tools {
        let overview = crate::global::overview(input.scan, input.adapters, adapter);
        for row in &overview.rows {
            let picked: Vec<&String> = row.members.iter().filter(|m| names.contains(*m)).collect();
            if picked.is_empty() {
                continue;
            }
            let label = if row.members.len() == picked.len() {
                row.name.clone()
            } else {
                picked
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            };
            for usage in &row.usage {
                let bucket = match usage.mark {
                    UseMark::Used => &mut using,
                    UseMark::Overridden | UseMark::Conflict => &mut same,
                    UseMark::Unused => continue,
                };
                bucket
                    .entry(usage.name.clone())
                    .or_default()
                    .insert(label.clone());
            }
        }
    }
    let render = |map: &BTreeMap<String, BTreeSet<String>>| -> String {
        map.iter()
            .map(|(project, items)| {
                format!(
                    "{project}（{}）",
                    items.iter().cloned().collect::<Vec<_>>().join("、")
                )
            })
            .collect::<Vec<_>>()
            .join("、")
    };
    if using.is_empty() {
        builder.note(PlanNote::info("近 30 天没有项目用到这些项"));
    } else {
        builder.note(PlanNote::warning(format!(
            "受影响的项目：{} 个近 30 天用到过，移除后读不到，要用的话加进项目清单：{}",
            using.len(),
            render(&using)
        )));
    }
    if !same.is_empty() {
        builder.note(PlanNote::info(format!(
            "{} 个项目在项目层有同名项，移除后项目那一份单独生效：{}",
            same.len(),
            render(&same)
        )));
    }
}

/// 别的工具、别的位置是链到这些目录的：移除后它们也读不到。
fn linked_from_elsewhere(
    input: &RemoveInput<'_>,
    removed: &BTreeSet<PathBuf>,
    removed_real: &BTreeSet<PathBuf>,
    builder: &mut PlanBuilder,
) {
    let mut others = BTreeSet::new();
    let all = input.scan.global.values().chain(
        input
            .scan
            .project_items
            .values()
            .flat_map(|per_tool| per_tool.values()),
    );
    for collected in all {
        for copy in &collected.skills {
            if copy.link_target.is_some()
                && !removed.contains(&copy.dir)
                && removed_real.contains(&copy.real_dir)
            {
                others.insert(display_path(&copy.dir, input.user_home));
            }
        }
    }
    if !others.is_empty() {
        let list: Vec<String> = others.into_iter().collect();
        builder.note(PlanNote::warning(format!(
            "{} 处是链到被移除目录的软链，移除后也读不到：{}",
            list.len(),
            list.join("、")
        )));
    }
}
