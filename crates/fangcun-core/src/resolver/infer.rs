//! 从目录现状反推清单与锁（`fangcun init --from-existing`，F3）。规则是人在 2026-11-19 定的
//! （DECISIONS「登记时反推清单的四条规则」）：
//!
//! - **目标工具**：项目里有这个工具的痕迹（读到了它的 skill、MCP、指令、其他加载项），或近 30 天
//!   在这个项目用过它。只算完整支持的工具；一个都没有时全写。
//! - **Skill**：只写查得出来源的。依次看项目的 `skills-lock.json` 与全局锁记的来源、软链指向的
//!   本机目录（记成 `local:` 来源）、资产库里内容一样的那一项。GitHub 来源沿历史找内容一样的
//!   提交锁定，清单里不写版本（跟随默认分支）。其余的不进清单，留在原处照常生效，计划里写明。
//! - **MCP**：项目层的服务写进 `[mcp]`；`env` 里的明文密钥不抄，换成 `secret:` 引用并标红。
//! - **指令**：一份只是对另一份的引用（`@AGENTS.md` 或软链）时，被引用的是正本、它是镜像；
//!   两份各自独立时正本取 `AGENTS.md`，另一份不设为镜像。
//!
//! 解析 skill 时会把那一份存进资产库，和写锁一样（资产库是方寸自己的缓存）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::adapter::{AdapterRegistry, SupportLevel, expand};
use crate::library::Library;
use crate::library::source::{Installed, Resolved, Sources};
use crate::manifest::{
    InstructionsDecl, Lock, LockedSkill, Manifest, McpDecl, ProjectSection, SkillDecl,
    SkillsLockEntry, read_skills_lock,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{PlanNote, Scope, SourceRef, SyncMode, ToolId, VersionPolicy};
use crate::scanner::{Collected, SkillCopy, looks_like_secret};
use crate::{Error, Result};

/// 登记时建议的物化方式（9.2）：个人项目默认链接，多人协作的仓库建议入库。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModeSuggestion {
    pub mode: SyncMode,
    /// git 历史里的提交者人数，`[bot]` 不算。不是 git 仓库时为 0。
    pub committers: u32,
}

/// 往回看多少个提交来数提交者。
const COMMITTER_WINDOW: &str = "500";

/// 数提交者：两位及以上算多人协作，建议入库。
pub fn suggest_mode(root: &Path) -> ModeSuggestion {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["log", "-n", COMMITTER_WINDOW, "--format=%an%x00%ae"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let text = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => String::new(),
    };
    let people: std::collections::BTreeSet<String> = text
        .lines()
        .filter_map(|line| line.split_once('\0'))
        .filter(|(name, _)| !name.ends_with("[bot]"))
        .map(|(_, email)| email.to_ascii_lowercase())
        .collect();
    let committers = u32::try_from(people.len()).unwrap_or(u32::MAX);
    ModeSuggestion {
        mode: if committers >= 2 {
            SyncMode::Vendor
        } else {
            SyncMode::Link
        },
        committers,
    }
}

/// 反推的输入：一个项目在各工具下读到的东西。
#[derive(Debug)]
pub struct InferInput<'a> {
    pub root: &'a Path,
    pub user_home: &'a Path,
    /// `~/.fangcun`：指向这里面的软链不算本地来源。
    pub fangcun_root: &'a Path,
    pub adapters: &'a AdapterRegistry,
    /// 每个工具在这个项目里读到的项目层与个人层。
    pub collected: &'a BTreeMap<ToolId, Collected>,
    /// 近 30 天在这个项目用过的工具。
    pub used: &'a [ToolId],
}

/// 反推的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inferred {
    pub manifest: Manifest,
    /// 已经解析好的 skill。交给 [`super::resolve_lock`] 当「上一份锁」，约束没变就原样沿用。
    pub seed: Lock,
    pub notes: Vec<PlanNote>,
    /// 改写成 `secret:` 引用的明文：登记时一并存进钥匙串（M1-17，9.3）。名称 → 值。
    pub secrets: BTreeMap<String, crate::secrets::SecretValue>,
}

pub fn infer_project(
    input: &InferInput<'_>,
    sources: &Sources,
    library: &Library,
) -> Result<Inferred> {
    let mut out = Inferred {
        manifest: Manifest {
            project: ProjectSection {
                targets: infer_targets(input),
                ..ProjectSection::default()
            },
            ..Manifest::default()
        },
        seed: Lock::default(),
        notes: Vec::new(),
        secrets: BTreeMap::new(),
    };
    infer_skills(input, sources, library, &mut out)?;
    infer_mcp(input, &mut out);
    infer_instructions(input, &mut out);
    Ok(out)
}

/// 只看完整支持的工具，按适配器的顺序。
fn full_tools(adapters: &AdapterRegistry) -> Vec<ToolId> {
    adapters
        .iter()
        .filter(|a| a.tool.support == SupportLevel::Full)
        .map(|a| a.tool.id.clone())
        .collect()
}

/// 目标工具：有痕迹或近 30 天用过的完整支持的工具，一个都没有时全写。
pub fn infer_targets(input: &InferInput<'_>) -> Vec<ToolId> {
    let full = full_tools(input.adapters);
    let local = |scope: Scope| matches!(scope, Scope::Project | Scope::Personal);
    let found: Vec<ToolId> = full
        .iter()
        .filter(|tool| {
            let traced = input.collected.get(*tool).is_some_and(|c| {
                c.skills.iter().any(|s| local(s.scope))
                    || c.mcp.iter().any(|m| local(m.scope))
                    || c.observed.iter().any(|o| local(o.scope))
                    // 退而读取的（Claude Code 没有 CLAUDE.md 时读 AGENTS.md）不算它的痕迹。
                    || c.instructions
                        .iter()
                        .any(|i| local(i.scope) && !i.fallback && !i.above_root)
            });
            traced || input.used.contains(tool)
        })
        .cloned()
        .collect();
    if found.is_empty() { full } else { found }
}

// ───────────── Skill ─────────────

/// 一项 skill 能查到的来源线索。
enum Clue {
    /// 锁（项目的 `skills-lock.json` 或全局锁）记了 GitHub 来源。
    Github {
        source: String,
        entry: Option<SkillsLockEntry>,
    },
    /// 软链到本机某个目录：`root` 是来源目录（git 仓库根，不在仓库里时是上一级）。
    Local { root: PathBuf },
    /// 资产库里有内容一样的一项。
    Library {
        source: String,
        commit: Option<String>,
    },
}

fn infer_skills(
    input: &InferInput<'_>,
    sources: &Sources,
    library: &Library,
    out: &mut Inferred,
) -> Result<()> {
    // 同名的几份（例如 .claude/skills 软链到 .agents/skills）内容一样就是同一项。
    let mut by_name: BTreeMap<&str, Vec<&SkillCopy>> = BTreeMap::new();
    for tool in full_tools(input.adapters) {
        let Some(collected) = input.collected.get(&tool) else {
            continue;
        };
        for skill in collected
            .skills
            .iter()
            .filter(|s| s.scope == Scope::Project)
        {
            by_name.entry(skill.name.as_str()).or_default().push(skill);
        }
    }
    let locks = project_skills_locks(input)?;
    let library_entries = library.entries()?;
    let mut unknown = Vec::new();

    for (name, copies) in by_name {
        let first = copies[0];
        if copies.iter().any(|c| c.hash != first.hash) {
            out.notes.push(PlanNote::warning(format!(
                "{name} 在项目里有内容不同的几份，未纳入清单"
            )));
            continue;
        }
        let clue = copies
            .iter()
            .find_map(|copy| clue_for(input, copy, &locks, &library_entries));
        let Some(clue) = clue else {
            unknown.push(name);
            continue;
        };
        match resolve_clue(name, first, clue, sources, library) {
            Ok((decl, locked, note)) => {
                out.notes.extend(note);
                out.seed.skills.insert(name.to_owned(), locked);
                out.manifest.skills.insert(name.to_owned(), decl);
            }
            Err(e) => out
                .notes
                .push(PlanNote::warning(format!("{name}：{e}。未纳入清单"))),
        }
    }
    if !unknown.is_empty() {
        out.notes.push(PlanNote::info(format!(
            "{} 项 skill 来源未知，未纳入清单，留在原处照常生效：{}。收编后再加",
            unknown.len(),
            unknown.join("、")
        )));
    }
    Ok(())
}

/// 项目里的 `skills-lock.json`，位置从适配器查（项目层 skill 目录的 `lock`）。名称 → 那一项。
fn project_skills_locks(input: &InferInput<'_>) -> Result<BTreeMap<String, SkillsLockEntry>> {
    let mut entries = BTreeMap::new();
    let mut read = Vec::new();
    for adapter in input.adapters.iter() {
        for location in adapter.skills.iter().filter(|l| l.scope == Scope::Project) {
            let Some(lock) = &location.lock else { continue };
            let (Some(path), Some(installed)) = (
                expand(&lock.path, input.user_home, Some(input.root)),
                expand(&location.path, input.user_home, Some(input.root)),
            ) else {
                continue;
            };
            if read.contains(&path) {
                continue;
            }
            read.push(path.clone());
            if let Some(found) = read_skills_lock(&path, &installed)? {
                for (name, entry) in found.skills {
                    entries.entry(name).or_insert(entry);
                }
            }
        }
    }
    Ok(entries)
}

fn clue_for(
    input: &InferInput<'_>,
    copy: &SkillCopy,
    locks: &BTreeMap<String, SkillsLockEntry>,
    library_entries: &[crate::library::LibraryEntry],
) -> Option<Clue> {
    if let Some(entry) = locks.get(&copy.name)
        && let Some(source) = entry.github_source()
    {
        return Some(Clue::Github {
            source,
            entry: Some(entry.clone()),
        });
    }
    // 全局锁记的来源是 `owner/repo`；`builtin:` 这类不是能解析的来源。
    if let Some(source) = copy.source.as_deref().and_then(github_shorthand) {
        return Some(Clue::Github {
            source,
            entry: None,
        });
    }
    if copy.link_target.is_some() && is_local_source(input, &copy.real_dir) {
        let root = copy
            .real_dir
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .or_else(|| copy.real_dir.parent())?
            .to_path_buf();
        return Some(Clue::Local { root });
    }
    library_entries
        .iter()
        .filter(|e| e.kind == crate::model::AssetKind::Skill)
        .find_map(|e| {
            let source = e.source.as_ref()?.0.clone();
            let version = e.versions.iter().find(|v| v.hash == copy.hash)?;
            Some(Clue::Library {
                source,
                commit: version.commit.clone(),
            })
        })
}

/// `owner/repo` → `github:owner/repo`。
fn github_shorthand(source: &str) -> Option<String> {
    let mut parts = source.split('/');
    let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    let plain = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    (plain(owner) && plain(repo)).then(|| format!("github:{owner}/{repo}"))
}

/// 软链指向的真实目录算不算本地来源：不在项目里、不在方寸目录里、不在任何工具的全局目录里。
fn is_local_source(input: &InferInput<'_>, real_dir: &Path) -> bool {
    let canonical = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if real_dir.starts_with(canonical(input.root))
        || real_dir.starts_with(canonical(input.fangcun_root))
    {
        return false;
    }
    !input.adapters.iter().any(|adapter| {
        adapter
            .skills
            .iter()
            .filter(|l| l.scope == Scope::Global)
            .filter_map(|l| expand(&l.path, input.user_home, None))
            .any(|dir| real_dir.starts_with(canonical(&dir)))
    })
}

type Resolution = (SkillDecl, LockedSkill, Option<PlanNote>);

fn resolve_clue(
    name: &str,
    copy: &SkillCopy,
    clue: Clue,
    sources: &Sources,
    library: &Library,
) -> Result<Resolution> {
    match clue {
        Clue::Github { source, entry } => {
            let entry = entry.as_ref();
            let source = SourceRef(source);
            let matched = sources.match_installed(
                library,
                &Installed {
                    name,
                    source: &source,
                    git_ref: entry.and_then(|e| e.git_ref.as_deref()),
                    commit: entry.and_then(|e| e.commit.as_deref()),
                    path: entry.and_then(|e| e.path.as_deref()),
                    hash: &copy.hash,
                },
            )?;
            let gone = matched.upstream_missing.then(|| {
                PlanNote::warning(format!(
                    "{name}：{} 的最新版里已经没有它（改名或删掉了），锁定在历史里的 {}",
                    source.0, matched.resolved.path
                ))
            });
            let note = (!matched.exact).then(|| {
                PlanNote::warning(format!(
                    "{name}：本机这份和 {} 近 200 个改过它的提交都对不上（可能改过），锁定为{}；同步时会换成锁定的版本",
                    source.0,
                    match &matched.policy {
                        Some(VersionPolicy::Track(b)) => format!(" {b} 分支的最新版"),
                        Some(VersionPolicy::Pin(p)) => format!(" {p}"),
                        None => "默认分支的最新版".to_owned(),
                    }
                ))
            });
            let (pin, track) = match matched.policy {
                Some(VersionPolicy::Pin(p)) => (Some(p), None),
                Some(VersionPolicy::Track(b)) => (None, Some(b)),
                None => (None, None),
            };
            let (decl, locked) = declare(name, source.0, pin, track, matched.resolved);
            Ok((decl, locked, note.or(gone)))
        }
        Clue::Library { source, commit } => {
            let source_ref = SourceRef(source.clone());
            if source.starts_with("github:") {
                let matched = sources.match_installed(
                    library,
                    &Installed {
                        name,
                        source: &source_ref,
                        git_ref: None,
                        commit: commit.as_deref(),
                        path: None,
                        hash: &copy.hash,
                    },
                )?;
                let (decl, locked) = declare(name, source, None, None, matched.resolved);
                return Ok((decl, locked, None));
            }
            let resolved = resolve_by_name_or_path(sources, library, name, &source_ref, None)?;
            let (decl, locked) = declare(name, source, None, None, resolved);
            Ok((decl, locked, None))
        }
        Clue::Local { root } => {
            let local = root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| Error::Source(format!("{} 不能当来源目录", root.display())))?;
            match sources.local_dir(&local) {
                Ok(known) if known != root => {
                    return Err(Error::Source(format!(
                        "本机来源 {local} 已经记成 {}，和 {} 不是同一个目录",
                        known.display(),
                        root.display()
                    )));
                }
                Ok(_) => {}
                Err(_) => sources.remember_local(&local, &root)?,
            }
            let source = format!("local:{local}");
            let rel = copy
                .real_dir
                .strip_prefix(&root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            let resolved = resolve_by_name_or_path(
                sources,
                library,
                name,
                &SourceRef(source.clone()),
                rel.as_deref(),
            )?;
            let (decl, locked) = declare(name, source, None, None, resolved);
            Ok((decl, locked, None))
        }
    }
}

/// 先按名字找；名字对不上（例如 `SKILL.md` 里的 `name` 和目录名不同）再按目录找。
fn resolve_by_name_or_path(
    sources: &Sources,
    library: &Library,
    name: &str,
    source: &SourceRef,
    path: Option<&str>,
) -> Result<Resolved> {
    match sources.resolve_skill(library, name, source, None, None) {
        Ok(resolved) => Ok(resolved),
        Err(e) => match path {
            Some(path) => sources.resolve_skill(library, name, source, None, Some(path)),
            None => Err(e),
        },
    }
}

/// 清单里的一项和锁里对应的一项。目录名和 skill 名字一样时清单不写 `path`（F2：按名字找）。
fn declare(
    name: &str,
    source: String,
    pin: Option<String>,
    track: Option<String>,
    resolved: Resolved,
) -> (SkillDecl, LockedSkill) {
    let found_by_name = resolved.path.rsplit('/').next() == Some(name);
    let decl = SkillDecl {
        source: source.clone(),
        pin: pin.clone(),
        track: track.clone(),
        path: (!found_by_name).then(|| resolved.path.clone()),
    };
    let locked = LockedSkill {
        source,
        track,
        pin,
        tag: resolved.tag,
        commit: resolved.commit,
        path: resolved.path,
        hash: resolved.version.hash,
        bundle: None,
    };
    (decl, locked)
}

// ───────────── MCP ─────────────

fn infer_mcp(input: &InferInput<'_>, out: &mut Inferred) {
    let mut origin: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut secrets = Vec::new();
    for tool in &out.manifest.project.targets.clone() {
        let Some(collected) = input.collected.get(tool) else {
            continue;
        };
        // 个人层（例如 ~/.claude.json 里这个项目的那一节）不进全队共享的清单。
        for entry in collected.mcp.iter().filter(|m| m.scope == Scope::Project) {
            let file = relative(input.root, &entry.file);
            let Some(command) = &entry.command else {
                if entry.url.is_some() {
                    out.notes.push(PlanNote::info(format!(
                        "{} 是远程服务（url），清单还不支持，未纳入",
                        entry.name
                    )));
                }
                continue;
            };
            if !entry.enabled {
                out.notes.push(PlanNote::info(format!(
                    "{} 在 {file} 里停用了，未纳入",
                    entry.name
                )));
                continue;
            }
            let (env, plain) =
                crate::secrets::split_env(&entry.name, entry.env.iter().map(|(k, v)| (k, v)));
            for (key, value) in &entry.env {
                if looks_like_secret(key, value) {
                    secrets.push(format!("{} 的 {key}", entry.name));
                }
            }
            for (reference, value) in plain {
                out.secrets.entry(reference).or_insert(value);
            }
            let decl = McpDecl {
                command: command.clone(),
                args: entry.args.clone(),
                env,
            };
            match out.manifest.mcp.get(&entry.name) {
                None => {
                    origin.insert(entry.name.clone(), entry.file.clone());
                    out.manifest.mcp.insert(entry.name.clone(), decl);
                }
                Some(existing) if *existing == decl => {}
                Some(_) => {
                    let first = origin
                        .get(&entry.name)
                        .map(|f| relative(input.root, f))
                        .unwrap_or_default();
                    out.notes.push(PlanNote::warning(format!(
                        "{} 在 {first} 和 {file} 里写得不一样，清单取 {first} 的",
                        entry.name
                    )));
                }
            }
        }
    }
    secrets.dedup();
    if !secrets.is_empty() {
        out.notes.push(PlanNote::danger(format!(
            "{} 个明文密钥没有抄进清单，改写成了 secret: 引用：{}。明文存进钥匙串；提交的配置在同步后只留环境变量引用",
            secrets.len(),
            secrets.join("、")
        )));
    }
}

// ───────────── 指令 ─────────────

/// 正本取这个名字（两份各自独立时）。它是 Codex 等工具共读的那份。
const PREFERRED_CANONICAL: &str = "AGENTS.md";

fn infer_instructions(input: &InferInput<'_>, out: &mut Inferred) {
    // 项目根目录下、常驻加载的指令文件，按相对路径去重。规则目录、子目录里的不算。
    let mut files: BTreeMap<String, (PathBuf, Option<String>)> = BTreeMap::new();
    for collected in input.collected.values() {
        for file in &collected.instructions {
            if file.scope != Scope::Project || file.above_root || file.on_demand {
                continue;
            }
            if file.path.parent() != Some(input.root) {
                continue;
            }
            let rel = relative(input.root, &file.path);
            files
                .entry(rel)
                .or_insert_with(|| (file.real_path.clone(), file.mirror_of.clone()));
        }
    }
    if files.is_empty() {
        return;
    }
    // 只是对另一份的引用（`@AGENTS.md` 或软链）：记下引用的是谁。
    let mirror_of = |rel: &str| -> Option<String> {
        let (real, mirror) = files.get(rel)?;
        if let Some(target) = mirror {
            let target = target.trim_start_matches("./").to_owned();
            if target != rel && files.contains_key(&target) {
                return Some(target);
            }
        }
        files
            .iter()
            .find(|(other, (other_real, _))| {
                *other != rel && other_real == real && files[*other].1.is_none()
            })
            .map(|(other, _)| other.clone())
    };
    let roots: Vec<&String> = files
        .keys()
        .filter(|rel| mirror_of(rel).is_none())
        .collect();
    let canonical = roots
        .iter()
        .find(|r| r.as_str() == PREFERRED_CANONICAL)
        .or_else(|| roots.first())
        .map(|r| (*r).clone())
        .unwrap_or_else(|| PREFERRED_CANONICAL.to_owned());
    let mut mirror = Vec::new();
    for rel in files.keys() {
        if *rel == canonical {
            continue;
        }
        if mirror_of(rel).as_deref() == Some(canonical.as_str()) {
            mirror.push(rel.clone());
        } else {
            out.notes.push(PlanNote::info(format!(
                "{rel} 和正本 {canonical} 内容各自独立，未设为镜像"
            )));
        }
    }
    out.manifest.instructions = Some(InstructionsDecl { canonical, mirror });
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(all(test, unix))]
mod tests;
