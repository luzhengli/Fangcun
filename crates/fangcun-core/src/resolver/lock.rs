//! 写锁（F3、7.4）：把全队共享的声明（套装与项目清单，不含个人覆盖）解析成 [`Lock`]。
//!
//! - 约束没变的 skill（来源、`pin`/`track`、`path` 都和锁里一样）沿用锁里的版本，不联网。
//!   新增的、换了约束的重新解析；清单里删掉的从锁里去掉。升级跟随的项在 M1-16（`update`）。
//! - MCP 记启动配置的哈希：命令、参数、环境变量（值只是 `secret:` 引用）。
//! - 指令记正本的内容哈希。

use std::collections::BTreeMap;

use crate::library::Library;
use crate::library::content::hash_bytes;
use crate::library::source::Sources;
use crate::manifest::{
    Declared, Layer, Lock, LockedInstruction, LockedMcp, LockedSkill, McpDecl, Project, SkillDecl,
};
use crate::model::{SourceRef, VersionPolicy};
use crate::{Error, Result};

pub fn resolve_lock(
    project: &Project,
    previous: Option<&Lock>,
    sources: &Sources,
    library: &Library,
) -> Result<Lock> {
    let declared = project.team_declarations();
    let mut lock = Lock::default();

    for (name, item) in &declared.skills {
        let bundle = bundle_of(&item.layer);
        let kept = previous
            .and_then(|p| p.skills.get(name))
            .filter(|old| unchanged(&item.decl, old));
        let locked = match kept {
            Some(old) => LockedSkill {
                bundle,
                ..old.clone()
            },
            None => resolve_skill(name, item, sources, library)?,
        };
        lock.skills.insert(name.clone(), locked);
    }

    for (name, item) in &declared.mcp {
        lock.mcp.insert(
            name.clone(),
            LockedMcp {
                hash: mcp_hash(&item.decl),
                bundle: bundle_of(&item.layer),
            },
        );
    }

    if let Some(item) = &declared.instructions {
        let path = item.decl.canonical.clone();
        let file = project.root.join(&path);
        let bytes = std::fs::read(&file)
            .map_err(|_| Error::NotFound(format!("清单里写的正本 {path} 不在项目里")))?;
        lock.instructions.insert(
            path.clone(),
            LockedInstruction {
                path,
                hash: hash_bytes(&bytes),
                bundle: bundle_of(&item.layer),
            },
        );
    }
    Ok(lock)
}

fn resolve_skill(
    name: &str,
    item: &Declared<SkillDecl>,
    sources: &Sources,
    library: &Library,
) -> Result<LockedSkill> {
    let decl = &item.decl;
    let policy = match (&decl.pin, &decl.track) {
        (Some(_), Some(_)) => {
            return Err(Error::Source(format!("{name}：pin 与 track 只能写一个")));
        }
        (Some(pin), None) => Some(VersionPolicy::Pin(pin.clone())),
        (None, Some(track)) => Some(VersionPolicy::Track(track.clone())),
        (None, None) => None,
    };
    let resolved = sources
        .resolve_skill(
            library,
            name,
            &SourceRef(decl.source.clone()),
            policy.as_ref(),
            decl.path.as_deref(),
        )
        .map_err(|e| Error::Source(format!("{name}：{e}")))?;
    Ok(LockedSkill {
        source: decl.source.clone(),
        track: decl.track.clone(),
        pin: decl.pin.clone(),
        tag: resolved.tag,
        commit: resolved.commit,
        path: resolved.path,
        hash: resolved.version.hash,
        bundle: bundle_of(&item.layer),
    })
}

/// 约束没变：来源、`pin`、`track` 一样，清单写了 `path` 时也一样。
fn unchanged(decl: &SkillDecl, old: &LockedSkill) -> bool {
    decl.source == old.source
        && decl.pin == old.pin
        && decl.track == old.track
        && decl
            .path
            .as_deref()
            .is_none_or(|p| p.trim_matches('/') == old.path)
}

fn bundle_of(layer: &Layer) -> Option<String> {
    match layer {
        Layer::Bundle(name) => Some(name.clone()),
        Layer::Project | Layer::Personal => None,
    }
}

/// 启动配置的哈希：按固定顺序拼成 JSON（环境变量按名字排序），再 SHA-256。
fn mcp_hash(decl: &McpDecl) -> crate::model::ContentHash {
    let env: BTreeMap<&String, &String> = decl.env.iter().collect();
    let canonical = serde_json::json!({
        "command": decl.command,
        "args": decl.args,
        "env": env,
    });
    hash_bytes(canonical.to_string().as_bytes())
}

#[cfg(all(test, unix))]
mod tests;
