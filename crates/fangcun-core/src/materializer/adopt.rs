//! 收编（F1、M1-12）：把一项 skill 散在各处的副本归并进资产库，原位置换成指向正本的软链。
//!
//! 规则是人在 2026-11-19 定的（9.3）：
//! - 内容相同的副本归为一个版本，分叉的另存为独立版本；每个位置链到自己那一版，Agent 读到的
//!   内容不变，去留以后再定。
//! - 只换链接，不改清单；记在本机的收编台账（`state/adopted.toml`）。
//! - 只换工具目录里的实体目录。软链到别处（例如本地 skill 仓库）的副本保持不动；未登记项目、
//!   只读扫描的工具不在写入范围里，跳过并提示。
//!
//! 存进资产库发生在出计划时（资产库是方寸自己的缓存，和写锁一样）；替换原位置走变更计划，
//! 原内容进快照，回滚能还原。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::ledger::{Adopted, AdoptedLink};
use super::{PlanBuilder, State, WriteScope};
use crate::library::mcp::McpSpec;
use crate::library::{Library, Origin};
use crate::model::{AssetKind, ContentHash, PlanNote, SourceRef};
use crate::paths::FangcunHome;
use crate::scanner::fsutil::display_path;
use crate::scanner::{McpEntry, SkillCopy};
use crate::{Error, Result};

pub struct AdoptInput<'a> {
    pub name: &'a str,
    /// 扫描到的、叫这个名字的全部副本（全局层和各项目，各工具）。
    pub copies: Vec<&'a SkillCopy>,
    pub home: &'a FangcunHome,
    pub user_home: &'a Path,
    pub scope: &'a WriteScope,
    pub library: &'a Library,
}

pub fn plan(input: &AdoptInput<'_>, builder: &mut PlanBuilder) -> Result<()> {
    let name = input.name;
    if input.copies.is_empty() {
        return Err(Error::NotFound(format!("叫 {name} 的 skill")));
    }
    let objects = input.library.objects_dir();
    let mut places: BTreeMap<PathBuf, &SkillCopy> = BTreeMap::new();
    // 同一个位置会被几个工具各扫到一次，提示里只列一次。
    let mut linked_elsewhere = std::collections::BTreeSet::new();
    let mut out_of_scope = std::collections::BTreeSet::new();
    for copy in &input.copies {
        if copy
            .source
            .as_deref()
            .is_some_and(|s| s.starts_with("builtin:"))
        {
            continue;
        }
        if copy.link_target.is_some() {
            // 软链：指向资产库的已经收编过；指向另一份副本的，那一份换掉它就跟着变；
            // 指向别处（本地 skill 仓库）的保持不动。
            let managed = copy.real_dir.starts_with(&objects);
            let to_copy = input
                .copies
                .iter()
                .any(|c| c.link_target.is_none() && c.real_dir == copy.real_dir);
            if !managed && !to_copy {
                linked_elsewhere.insert(display_path(&copy.dir, input.user_home));
            }
            continue;
        }
        if input.scope.check(&copy.dir).is_err() {
            out_of_scope.insert(display_path(&copy.dir, input.user_home));
            continue;
        }
        // 扫描结果可能是旧的（收编过一次、还没重新体检）：磁盘上已经是软链的不再动，
        // 否则逐个删的会是链接那头资产库里的文件。
        if State::read(&copy.dir)? != State::Dir {
            continue;
        }
        places.entry(copy.dir.clone()).or_insert(copy);
    }
    if places.is_empty() {
        let mut why = vec![format!("{name} 没有可以收编的副本")];
        if !out_of_scope.is_empty() {
            let list: Vec<&str> = out_of_scope.iter().map(String::as_str).collect();
            why.push(format!("不在写入范围里：{}", list.join("、")));
        }
        return Err(Error::NotFound(why.join("。")));
    }

    // 每种内容存一版。来源只有锁里记过的才知道，其余是「来源未知」。
    let mut versions: BTreeMap<ContentHash, (PathBuf, usize)> = BTreeMap::new();
    for copy in places.values() {
        let entry = versions
            .entry(copy.hash.clone())
            .or_insert((PathBuf::new(), 0));
        entry.1 += 1;
        if entry.0.as_os_str().is_empty() {
            let source = copy
                .source
                .as_deref()
                .filter(|s| s.split('/').count() == 2)
                .map(|s| SourceRef(format!("github:{s}")));
            let version = input.library.ingest(
                name,
                AssetKind::Skill,
                &copy.dir,
                &Origin {
                    source,
                    commit: None,
                    tag: None,
                },
            )?;
            entry.0 = input
                .library
                .locate(&version.hash)?
                .ok_or_else(|| Error::NotFound(format!("资产库里的 {name}")))?;
        }
    }

    let mut ledger = Adopted::load(input.home)?;
    for (dir, copy) in &places {
        let (object, _) = &versions[&copy.hash];
        let short = &copy.hash.0[..copy.hash.0.len().min(7)];
        builder.set_quiet(true);
        let removed = remove_tree(builder, dir);
        builder.set_quiet(false);
        let removed = removed?;
        builder.symlink(
            dir,
            object,
            format!("改由资产库管理，链接到 {name}@{short}；原来的 {removed} 个文件已备份"),
        )?;
        ledger.links.retain(|l| l.path != *dir);
        ledger.links.push(AdoptedLink {
            path: dir.clone(),
            asset: name.to_owned(),
            hash: copy.hash.0.clone(),
        });
    }
    ledger.links.sort_by(|a, b| a.path.cmp(&b.path));
    builder.write_file(
        &Adopted::file(input.home),
        ledger.to_toml()?.into_bytes(),
        format!("收编台账：{name} 的 {} 个位置", places.len()),
    )?;

    builder.set_title(format!(
        "收编 {name}：{} 份副本归并为 {} 份正本",
        places.len(),
        versions.len()
    ));
    if versions.len() > 1 {
        let mut counts: Vec<(usize, String)> = versions
            .iter()
            .map(|(hash, (_, n))| (*n, hash.0[..hash.0.len().min(7)].to_owned()))
            .collect();
        counts.sort_by_key(|c| std::cmp::Reverse(c.0));
        let forks: Vec<String> = counts[1..]
            .iter()
            .map(|(n, h)| format!("{h}（{n} 份）"))
            .collect();
        builder.note(PlanNote::warning(format!(
            "有 {} 份已分叉，各自另存为独立版本，内容不变：{}。去留以后在升级或漂移处理时再定",
            versions.len() - 1,
            forks.join("、")
        )));
    }
    if !out_of_scope.is_empty() {
        let list: Vec<&str> = out_of_scope.iter().map(String::as_str).collect();
        builder.note(PlanNote::info(format!(
            "{} 份不在写入范围里（未登记的项目或只读扫描的工具），没有收编：{}",
            list.len(),
            list.join("、")
        )));
    }
    if !linked_elsewhere.is_empty() {
        let list: Vec<&str> = linked_elsewhere.iter().map(String::as_str).collect();
        builder.note(PlanNote::info(format!(
            "{} 份是指向本地目录的软链，保持不动：{}",
            list.len(),
            list.join("、")
        )));
    }
    Ok(())
}

/// 收编 MCP 的输入（M1-20）。
pub struct McpAdoptInput<'a> {
    pub name: &'a str,
    /// 扫描到的、叫这个名字的全部 MCP 配置（全局层、项目层、个人层，各工具）。
    pub entries: Vec<&'a McpEntry>,
    pub user_home: &'a Path,
    pub library: &'a Library,
    pub secrets: &'a dyn crate::secrets::SecretStore,
}

/// 收编一个 MCP 服务：每种写法存一版进资产库，`env` 里的明文先换成 `secret:` 引用，
/// 明文在应用时存进钥匙串（9.3「MCP 的明文密钥」）。
///
/// 工具的配置原样不动：MCP 没有「受管链接」可换，Agent 读到的内容不变；收编后全局层的这一项
/// 就能从全局移除（7.5）。和 skill 一样，资产库在出计划时就存（方寸自己的缓存）。
pub fn plan_mcp(input: &McpAdoptInput<'_>, builder: &mut PlanBuilder) -> Result<()> {
    let name = input.name;
    if input.entries.is_empty() {
        return Err(Error::NotFound(format!("叫 {name} 的 skill 或 MCP")));
    }
    let mut versions: BTreeMap<ContentHash, Vec<String>> = BTreeMap::new();
    let mut remote = std::collections::BTreeSet::new();
    let mut plain: BTreeMap<String, crate::secrets::SecretValue> = BTreeMap::new();
    let mut clashes = std::collections::BTreeSet::new();
    for entry in &input.entries {
        let place = format!(
            "{} · {}",
            display_path(&entry.file, input.user_home),
            entry.key
        );
        let Some(command) = &entry.command else {
            remote.insert(place);
            continue;
        };
        let (spec, found) = McpSpec::from_config(name, command, &entry.args, &entry.env);
        for (secret, value) in found {
            match plain.get(&secret) {
                Some(existing) if *existing != value => {
                    clashes.insert(secret);
                }
                Some(_) => {}
                None => {
                    plain.insert(secret, value);
                }
            }
        }
        let version = input.library.ingest_mcp(name, &spec, &Origin::default())?;
        let places = versions.entry(version.hash).or_default();
        if !places.contains(&place) {
            places.push(place);
        }
    }
    if versions.is_empty() {
        return Err(Error::NotFound(format!(
            "{name} 没有可以收编的配置：只有远程服务（url），资产库还不支持"
        )));
    }

    let mut stored = Vec::new();
    for (secret, value) in plain {
        if clashes.contains(&secret) {
            continue;
        }
        match input.secrets.get(&secret) {
            Ok(Some(existing)) if existing == value => stored.push(secret),
            Ok(Some(_)) => builder.note(PlanNote::warning(format!(
                "钥匙串里已经有 {secret}，值和配置里的不同，没有替换。要换用 fangcun secret set {secret}"
            ))),
            Ok(None) => {
                builder.store_secret(&secret, value);
                stored.push(secret);
            }
            Err(e) => builder.note(PlanNote::warning(format!(
                "{e}。{secret} 没有存进钥匙串，之后用 fangcun secret set {secret}"
            ))),
        }
    }
    for secret in &clashes {
        builder.note(PlanNote::warning(format!(
            "{name} 的几处配置里 {secret} 的值不一样，没有存进钥匙串。用 fangcun secret set {secret} 定一个"
        )));
    }

    let places: usize = versions.values().map(Vec::len).sum();
    builder.set_title(format!(
        "收编 MCP {name}：{places} 处配置归并为 {} 份正本",
        versions.len()
    ));
    let list: Vec<String> = versions
        .iter()
        .map(|(hash, at)| {
            format!(
                "{name}@{}（{}）",
                &hash.0[..hash.0.len().min(7)],
                at.join("、")
            )
        })
        .collect();
    builder.note(PlanNote::info(format!(
        "已存进资产库：{}。工具的配置原样不动",
        list.join("；")
    )));
    if !stored.is_empty() {
        builder.note(PlanNote::danger(format!(
            "{} 个明文密钥在资产库里换成了 secret: 引用：{}",
            stored.len(),
            stored.join("、")
        )));
    }
    if !remote.is_empty() {
        let list: Vec<&str> = remote.iter().map(String::as_str).collect();
        builder.note(PlanNote::info(format!(
            "{} 处是远程服务（url），资产库还不支持，没有收编：{}",
            list.len(),
            list.join("、")
        )));
    }
    Ok(())
}

/// 删掉目录里的全部东西（不跟随软链），子目录先删里面的。返回删了几个文件。
pub(crate) fn remove_tree(builder: &mut PlanBuilder, dir: &Path) -> Result<usize> {
    let mut count = 0;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|e| e.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    entries.sort();
    for entry in entries {
        if State::read(&entry)? == State::Dir {
            count += remove_tree(builder, &entry)?;
        } else {
            count += 1;
        }
        builder.remove(&entry, String::new())?;
    }
    Ok(count)
}
