//! 物化同步（F4、6.8、M1-10）：按清单与锁算出要对项目做的变更，填进变更计划。只算不写。
//!
//! - **Skill**：实体只放一份在 `.agents/skills/<名称>`（F4）。链接模式是指向资产库正本的软链，
//!   入库模式是实体目录，开发模式（`[dev]`）指向本地来源目录。目标工具自己的 skill 目录里放
//!   相对软链指过去。个人覆盖加的项一律用链接。
//! - **校验**（7.5）：正本的内容哈希必须等于锁里的；资产库里没有时按锁里的提交重新取，取到的
//!   对不上就不同步这一项。
//! - **MCP**：写进目标工具的项目配置，按键合并、按字段合并，别的键和字段不动（4.6 第 5 条）。
//!   `secret:` 引用写成环境变量引用；明文、个人覆盖加的服务写进工具的个人层（M1-17、M1-27）。
//! - **指令**：镜像写成对正本的引用（`@AGENTS.md`）。正本是项目自己的文件，不动。
//! - **`.gitignore`**：维护一段带起止标记的「方寸管理」块（人定，9.3）：链接模式的受管路径、
//!   个人覆盖与开发模式的项、`fangcun.local.toml`。块外的行不动。
//! - **已有的副本**（人定，9.3）：目标位置已有不归方寸管的东西时也替换；内容和锁定版本不同的
//!   标橙，替换前进快照，回滚能找回。
//! - **台账**（`ledger`）：记下这次方寸写过的路径和键。清单里去掉的项按台账删；台账里的路径
//!   被人改过时不删，等漂移处理（M1-15）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::ledger::{KeyKind, Ledger, OwnedKey, OwnedPath, PathKind};
use super::{PlanBuilder, State};
use crate::adapter::{AdapterRegistry, FileFormat, SupportLevel};
use crate::library::Library;
use crate::library::content::{hash_bytes, hash_dir, hashed_files};
use crate::library::source::Sources;
use crate::manifest::{Lock, LockedSkill, McpDecl, Project};
use crate::model::{ContentHash, PlanNote, Scope, SourceRef, SyncMode, VersionPolicy};
use crate::paths::FangcunHome;
use crate::secrets::{self, SecretStore, SecretValue};
use crate::{Error, Result};

/// 所有工具共读的 skill 目录（F4）：实体只放这里一份。
pub const SHARED_SKILLS: &str = ".agents/skills";

/// `.gitignore` 里方寸管理的那一段的起止标记。
const IGNORE_BEGIN: &str = "# >>> fangcun：方寸管理的这一段，同步时会改写";
const IGNORE_END: &str = "# <<< fangcun";

pub struct SyncInput<'a> {
    pub root: &'a Path,
    /// 每个目标工具在这个项目实际读到的 skill 与 MCP 名字，展开 `[disable]` 的通配用。
    pub visible: &'a BTreeMap<crate::model::ToolId, disable::Visible>,
    pub home: &'a FangcunHome,
    pub user_home: &'a Path,
    pub adapters: &'a AdapterRegistry,
    pub project: &'a Project,
    pub lock: &'a Lock,
    pub library: &'a Library,
    pub sources: &'a Sources,
    /// 钥匙串：用了 `secret:` 的服务同步时取明文写进个人层（M1-17）。
    pub secrets: &'a dyn SecretStore,
}

/// 一项要放进 `.agents/skills` 的 skill。
struct Placed {
    name: String,
    entity: Entity,
    hash: ContentHash,
    /// 计划里的写法，例如「tdd@a1b2c3d」。
    label: String,
    /// 不该提交：个人覆盖加的、开发模式的。
    private: bool,
}

enum Entity {
    /// 指向资产库正本（链接模式、个人项）。
    Link(PathBuf),
    /// 把正本复制进项目（入库模式）。
    Vendor(PathBuf),
    /// 指向本地来源目录（开发模式）。
    Dev(PathBuf),
}

/// 填计划。标题按变更数写，例如「同步 3 处变更」。
pub fn plan(input: &SyncInput<'_>, builder: &mut PlanBuilder) -> Result<()> {
    let root = input.root;
    let project = input.project;
    let mismatches = crate::resolver::lock_mismatches(project, input.lock);
    if !mismatches.is_empty() {
        return Err(Error::Frozen(format!(
            "{}。先运行 fangcun lock",
            mismatches.join("；")
        )));
    }
    let mode = project.manifest.project.mode.unwrap_or(SyncMode::Link);
    let old = Ledger::load(input.home, root)?;
    let mut state = Run {
        input,
        builder,
        old: &old,
        new: Ledger {
            project: root.to_path_buf(),
            mode: Some(mode),
            ..Ledger::default()
        },
        notes: Vec::new(),
        replaced: Vec::new(),
        ignored: BTreeSet::from(["/fangcun.local.toml".to_owned()]),
        docs: BTreeMap::new(),
        bad_docs: BTreeSet::new(),
    };

    let placed = state.collect_skills(mode)?;
    for skill in &placed {
        state.place_skill(skill, mode)?;
    }
    state.sync_mcp()?;
    state.sync_disable()?;
    state.flush_docs()?;
    state.sync_instructions()?;
    state.remove_dropped()?;
    state.sync_gitignore()?;

    if !state.replaced.is_empty() {
        state.notes.push(PlanNote::warning(format!(
            "{} 处本机原有的内容和锁定版本不同，替换前已备份，回滚能找回：{}",
            state.replaced.len(),
            state.replaced.join("、")
        )));
    }

    // 台账：方寸自己的文件，随计划写，回滚时一起还原。
    let rows = state.builder.change_count();
    let ledger_file = Ledger::file(input.home, root);
    let text = state.new.to_toml()?;
    let ledger_changed = std::fs::read(&ledger_file).ok().as_deref() != Some(text.as_bytes());
    if ledger_changed {
        state.builder.write_file(
            &ledger_file,
            text.into_bytes(),
            format!(
                "台账：方寸管的 {} 个路径、{} 个配置键",
                state.new.paths.len(),
                state.new.keys.len()
            ),
        )?;
    }
    let name = crate::scanner::project_name(root);
    let title = match (rows, ledger_changed) {
        (0, false) => format!("{name} 已是最新"),
        (0, true) => format!("{name} 已是最新，记下方寸管的文件"),
        (n, _) => format!("同步 {n} 处变更"),
    };
    state.builder.set_title(title);
    for note in state.notes {
        state.builder.note(note);
    }
    Ok(())
}

struct Run<'a, 'b> {
    input: &'a SyncInput<'a>,
    builder: &'b mut PlanBuilder,
    old: &'a Ledger,
    new: Ledger,
    notes: Vec<PlanNote>,
    /// 替换掉的、和锁定版本不同的本机内容。
    replaced: Vec<String>,
    /// `.gitignore` 方寸那一段的行。
    ignored: BTreeSet<String>,
    /// 改在内存里、还没写进计划的配置文件。
    docs: BTreeMap<PathBuf, PendingDoc>,
    bad_docs: BTreeSet<PathBuf>,
}

/// 一份改在内存里的配置，和改之前的原文。
struct PendingDoc {
    doc: ConfigDoc,
    original: Option<String>,
    /// 计划里这一行的说明，例如「新增键 mcpServers.github」。
    rows: Vec<String>,
}

impl Run<'_, '_> {
    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(self.input.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }

    // ───────────── Skill ─────────────

    fn collect_skills(&mut self, mode: SyncMode) -> Result<Vec<Placed>> {
        let input = self.input;
        let team = input.project.team_declarations();
        let local = input.project.local_declarations();
        let dev: BTreeSet<&String> = local.dev.iter().collect();
        let mut out = Vec::new();
        for (name, item) in &local.skills {
            let personal = !team.skills.contains_key(name);
            if dev.contains(name) {
                match self.dev_dir(name, &item.decl) {
                    Ok(dir) => {
                        let hash = hash_dir(&dir);
                        out.push(Placed {
                            name: name.clone(),
                            label: format!(
                                "本地 {}",
                                crate::scanner::fsutil::display_path(&dir, input.user_home)
                            ),
                            entity: Entity::Dev(dir),
                            hash,
                            private: true,
                        });
                    }
                    Err(e) => self
                        .notes
                        .push(PlanNote::warning(format!("{name}：{e}。开发模式没有同步"))),
                }
                continue;
            }
            let locked = if personal {
                match self.resolve_personal(name, &item.decl) {
                    Ok(locked) => locked,
                    Err(e) => {
                        self.notes.push(PlanNote::warning(format!(
                            "个人覆盖里的 {name}：{e}。没有同步"
                        )));
                        continue;
                    }
                }
            } else {
                match input.lock.skills.get(name) {
                    Some(locked) => locked.clone(),
                    None => continue,
                }
            };
            let object = match ensure_object(input.library, input.sources, name, &locked) {
                Ok(object) => object,
                Err(e) => {
                    self.notes
                        .push(PlanNote::danger(format!("{e}，没有同步这一项")));
                    continue;
                }
            };
            let short = &locked.hash.0[..locked.hash.0.len().min(7)];
            let label = match locked.tag.as_deref() {
                Some(tag) => format!("{name}@{tag}"),
                None => format!("{name}@{short}"),
            };
            let entity = if mode == SyncMode::Vendor && !personal {
                Entity::Vendor(object)
            } else {
                Entity::Link(object)
            };
            out.push(Placed {
                name: name.clone(),
                entity,
                hash: locked.hash.clone(),
                label,
                private: personal,
            });
        }
        Ok(out)
    }

    /// 开发模式：本地来源目录里这一项的位置。
    fn dev_dir(&self, name: &str, decl: &crate::manifest::SkillDecl) -> Result<PathBuf> {
        let local = decl
            .source
            .strip_prefix("local:")
            .ok_or_else(|| Error::Source("不是本地来源，不能走开发模式".to_owned()))?;
        let dir = self.input.sources.local_dir(local)?;
        let path = self
            .input
            .lock
            .skills
            .get(name)
            .map(|l| l.path.clone())
            .or_else(|| decl.path.clone())
            .unwrap_or_default();
        let dir = if path.is_empty() { dir } else { dir.join(path) };
        if !dir.join("SKILL.md").is_file() {
            return Err(Error::NotFound(format!(
                "{} 里没有 SKILL.md",
                dir.display()
            )));
        }
        Ok(dir)
    }

    /// 个人覆盖加的项不进锁（7.4），每次同步时解析。
    fn resolve_personal(
        &self,
        name: &str,
        decl: &crate::manifest::SkillDecl,
    ) -> Result<LockedSkill> {
        let policy = match (&decl.pin, &decl.track) {
            (Some(pin), _) => Some(VersionPolicy::Pin(pin.clone())),
            (None, Some(track)) => Some(VersionPolicy::Track(track.clone())),
            (None, None) => None,
        };
        let resolved = self.input.sources.resolve_skill(
            self.input.library,
            name,
            &SourceRef(decl.source.clone()),
            policy.as_ref(),
            decl.path.as_deref(),
        )?;
        Ok(LockedSkill {
            source: decl.source.clone(),
            track: decl.track.clone(),
            pin: decl.pin.clone(),
            tag: resolved.tag,
            commit: resolved.commit,
            path: resolved.path,
            hash: resolved.version.hash,
            bundle: None,
        })
    }

    fn place_skill(&mut self, skill: &Placed, mode: SyncMode) -> Result<()> {
        let rel = format!("{SHARED_SKILLS}/{}", skill.name);
        let path = self.input.root.join(&rel);
        let committed = mode == SyncMode::Vendor && !skill.private;
        match &skill.entity {
            Entity::Link(target) | Entity::Dev(target) => {
                let detail = match skill.entity {
                    Entity::Dev(_) => format!("开发模式，链接到{}", skill.label),
                    _ => format!("链接到 {}", skill.label),
                };
                self.place_link(&path, target, detail, Some(&skill.hash))?;
                self.new.paths.push(OwnedPath {
                    path: rel.clone(),
                    asset: skill.name.clone(),
                    kind: PathKind::SkillLink,
                    target: Some(target.to_string_lossy().into_owned()),
                    hash: Some(skill.hash.0.clone()),
                });
            }
            Entity::Vendor(object) => {
                self.place_dir(&path, object, skill)?;
                self.new.paths.push(OwnedPath {
                    path: rel.clone(),
                    asset: skill.name.clone(),
                    kind: PathKind::SkillDir,
                    target: None,
                    hash: Some(skill.hash.0.clone()),
                });
            }
        }
        if !committed {
            self.ignored.insert(format!("/{rel}"));
        }
        for tool_dir in self.tool_skill_dirs() {
            let link_rel = format!("{tool_dir}/{}", skill.name);
            let depth = tool_dir.split('/').filter(|p| !p.is_empty()).count();
            let target = PathBuf::from(format!("{}{rel}", "../".repeat(depth)));
            self.place_link(
                &self.input.root.join(&link_rel),
                &target,
                format!("相对链接到 {rel}"),
                Some(&skill.hash),
            )?;
            self.new.paths.push(OwnedPath {
                path: link_rel.clone(),
                asset: skill.name.clone(),
                kind: PathKind::ToolLink,
                target: Some(target.to_string_lossy().into_owned()),
                hash: None,
            });
            if !committed {
                self.ignored.insert(format!("/{link_rel}"));
            }
        }
        Ok(())
    }

    /// 目标工具自己的项目层 skill 目录，`.agents/skills` 以外的，按适配器的顺序。
    fn tool_skill_dirs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for tool in &self.input.project.manifest.project.targets {
            let Some(adapter) = self.input.adapters.get(tool) else {
                continue;
            };
            if adapter.tool.support != SupportLevel::Full {
                continue;
            }
            for location in adapter.skills.iter().filter(|l| l.scope == Scope::Project) {
                let dir = location.path.trim_end_matches('/');
                let relative = !dir.starts_with('/') && !dir.starts_with('~');
                if relative && dir != SHARED_SKILLS && !out.iter().any(|d| d == dir) {
                    out.push(dir.to_owned());
                }
            }
        }
        out
    }

    /// 放一个软链。已有的不归方寸管时也替换（人定，9.3），内容不同的记下来标橙。
    fn place_link(
        &mut self,
        path: &Path,
        target: &Path,
        detail: String,
        expected: Option<&ContentHash>,
    ) -> Result<()> {
        let current = State::read(path)?;
        if current == State::Symlink(target.to_path_buf()) {
            return Ok(());
        }
        let rel = self.rel(path);
        let owned = self.old.path(&rel).is_some();
        let detail = match &current {
            State::Absent => detail,
            _ if owned => format!("{detail}（原来是旧版本）"),
            _ => self.takeover_detail(path, &rel, expected, &detail),
        };
        if current == State::Dir {
            let count = self.remove_children(path)?;
            let detail = format!("{detail}，替换 {count} 个文件");
            return self.builder.symlink(path, target, detail);
        }
        self.builder.symlink(path, target, detail)
    }

    /// 接管一个不归方寸管的位置时那一行怎么写。
    fn takeover_detail(
        &mut self,
        path: &Path,
        rel: &str,
        expected: Option<&ContentHash>,
        detail: &str,
    ) -> String {
        let same = expected.is_some_and(|h| hash_dir(path) == *h);
        if same {
            format!("{detail}；改由方寸管理，内容相同")
        } else {
            self.replaced.push(rel.to_owned());
            format!("{detail}；本机原有的内容不同，已备份")
        }
    }

    /// 入库模式：把正本复制成实体目录。和正本一样就不动。
    fn place_dir(&mut self, path: &Path, object: &Path, skill: &Placed) -> Result<()> {
        let current = State::read(path)?;
        if current == State::Dir && hash_dir(path) == skill.hash {
            return Ok(());
        }
        let rel = self.rel(path);
        let owned = self.old.path(&rel).is_some();
        let files = hashed_files(object);
        let detail = match &current {
            State::Absent => format!("入库 {}（{} 个文件）", skill.label, files.len()),
            _ if owned => format!("更新到 {}（{} 个文件）", skill.label, files.len()),
            _ => {
                let base = format!("入库 {}（{} 个文件）", skill.label, files.len());
                self.takeover_detail(path, &rel, Some(&skill.hash), &base)
            }
        };
        let wanted: BTreeSet<PathBuf> = files.iter().map(|f| path.join(&f.rel)).collect();
        if current == State::Dir {
            // 删掉正本里没有的文件，其余的就地改写。
            self.builder.set_quiet(true);
            let result = self.remove_tree_except(path, &wanted);
            self.builder.set_quiet(false);
            result?;
        }
        self.builder.dir(path, detail)?;
        self.builder.set_quiet(true);
        let result = (|| {
            for file in &files {
                let target = path.join(&file.rel);
                let mode = file_mode(&file.source);
                if State::read(&target).ok() == Some(State::File(sha_state(&file.bytes)))
                    && current == State::Dir
                {
                    continue;
                }
                self.builder
                    .write_file_mode(&target, file.bytes.clone(), mode, String::new())?;
            }
            Ok(())
        })();
        self.builder.set_quiet(false);
        result
    }

    /// 删掉目录里的全部东西（不跟随软链），不列行。返回删了几个文件。
    fn remove_children(&mut self, dir: &Path) -> Result<usize> {
        self.builder.set_quiet(true);
        let result = self.remove_tree_except(dir, &BTreeSet::new());
        self.builder.set_quiet(false);
        result
    }

    /// 删掉目录里不在 `keep` 里的东西，子目录先删里面的。
    fn remove_tree_except(&mut self, dir: &Path, keep: &BTreeSet<PathBuf>) -> Result<usize> {
        let mut count = 0;
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|e| e.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        entries.sort();
        for entry in entries {
            match State::read(&entry)? {
                State::Dir => {
                    count += self.remove_tree_except(&entry, keep)?;
                    if !keep.iter().any(|k| k.starts_with(&entry)) {
                        self.builder.remove(&entry, String::new())?;
                    }
                }
                State::Absent => {}
                _ => {
                    if !keep.contains(&entry) {
                        self.builder.remove(&entry, String::new())?;
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    // ───────────── MCP ─────────────

    /// MCP 写两处（4.6 第 5 条按键合并，M1-17、M1-27）：
    ///
    /// - 项目层（提交）：全队的服务。`secret:` 引用按适配器写成环境变量引用（Claude Code 的
    ///   `${NAME}`，Codex 的 `env_vars`），不写明文。
    /// - 个人层（不提交，只在本机）：个人覆盖加的、改过的服务，以及用了密钥的服务带明文的那一条
    ///   （Claude Code 个人层优先、整条取用）。工具没有个人层时（Codex）写一条说明。
    ///
    /// 钥匙串里缺密钥的服务不同步，已经写过的不删。
    fn sync_mcp(&mut self) -> Result<()> {
        let input = self.input;
        let team = input.project.team_declarations();
        let local = input.project.local_declarations();

        // 1. 钥匙串：每个用到的密钥取一次。
        let mut values: BTreeMap<String, SecretValue> = BTreeMap::new();
        let mut missing: BTreeSet<String> = BTreeSet::new();
        let mut waiting: Vec<String> = Vec::new();
        for (name, item) in &local.mcp {
            let mut lacks = false;
            for secret in item.decl.env.values().filter_map(|v| secrets::reference(v)) {
                if values.contains_key(secret) || missing.contains(secret) {
                    lacks |= missing.contains(secret);
                    continue;
                }
                match input.secrets.get(secret) {
                    Ok(Some(value)) => {
                        values.insert(secret.to_owned(), value);
                    }
                    Ok(None) => {
                        missing.insert(secret.to_owned());
                        lacks = true;
                    }
                    Err(e) => {
                        self.notes.push(PlanNote::warning(e.to_string()));
                        missing.insert(secret.to_owned());
                        lacks = true;
                    }
                }
            }
            if lacks {
                waiting.push(name.clone());
            }
        }
        if !waiting.is_empty() {
            let names: Vec<&str> = missing.iter().map(String::as_str).collect();
            self.notes.push(PlanNote::warning(format!(
                "钥匙串里没有 {}，用到它的 {} 没有同步。先运行 fangcun secret set {}",
                names.join("、"),
                waiting.join("、"),
                names.first().copied().unwrap_or_default()
            )));
        }

        // 2. 项目层：全队的服务，密钥只写引用。
        let desired: BTreeMap<&String, &McpDecl> = team
            .mcp
            .iter()
            .filter(|(name, _)| !waiting.contains(name))
            .map(|(name, item)| (name, &item.decl))
            .collect();
        let mut files: Vec<ProjectMcpFile> = Vec::new();
        for tool in &input.project.manifest.project.targets {
            let Some(adapter) = input.adapters.get(tool) else {
                continue;
            };
            for location in adapter.mcp.iter().filter(|l| l.scope == Scope::Project) {
                let relative = !location.file.starts_with(['/', '~', '$']);
                let plain = location.key.iter().all(|k| !k.contains('{'));
                if relative && plain && !files.iter().any(|f| f.file == location.file) {
                    files.push(ProjectMcpFile {
                        file: location.file.clone(),
                        format: location.format,
                        key_path: location.key.clone(),
                        fields: adapter.mcp_fields.clone(),
                        tool_name: Some(adapter.tool.name.clone()),
                    });
                }
            }
        }
        for key in self.old.keys.iter().filter(|k| k.kind.is_mcp()) {
            if !files.iter().any(|f| f.file == key.file) {
                let format = if key.file.ends_with(".toml") {
                    FileFormat::Toml
                } else {
                    FileFormat::Json
                };
                let key_path: Vec<String> = key
                    .key
                    .rsplit_once('.')
                    .map(|(p, _)| p.split('.').map(str::to_owned).collect())
                    .unwrap_or_default();
                files.push(ProjectMcpFile {
                    file: key.file.clone(),
                    format,
                    key_path,
                    fields: None,
                    tool_name: None,
                });
            }
        }

        for target in files {
            let file = target.file.clone();
            let path = input.root.join(&file);
            let Some(mut pending) = self.take_doc(&path, target.format, &file) else {
                continue;
            };
            let key_path = &target.key_path;
            let forward = target.forward_field();
            let prefix = key_path.join(".");
            let mut rows = Vec::new();
            if let Some(tool_name) = &target.tool_name {
                let mut forwarded = Vec::new();
                for (name, decl) in &desired {
                    let (server, dropped) = target.committed(decl);
                    forwarded.extend(server.forward.iter().cloned());
                    if !dropped.is_empty() {
                        self.notes.push(PlanNote::warning(format!(
                            "{tool_name} 的配置没有引用环境变量的写法，{name} 的 {} 没有写进去",
                            dropped.join("、")
                        )));
                    }
                    let key = format!("{prefix}.{name}");
                    let owned = self.old.key(&file, &key).is_some();
                    let doc = &mut pending.doc;
                    match doc.get(key_path, name, forward) {
                        Some(existing) if existing == server => {}
                        Some(_) if !owned => {
                            self.replaced.push(format!("{file} 的 {key}"));
                            rows.push(format!("替换键 {key}"));
                        }
                        Some(_) => rows.push(format!("更新键 {key}")),
                        None => rows.push(format!("新增键 {key}")),
                    }
                    doc.set(key_path, name, &server, forward);
                    let refs = decl
                        .env
                        .iter()
                        .filter_map(|(var, v)| {
                            Some((var.clone(), secrets::reference(v)?.to_owned()))
                        })
                        .collect();
                    self.new.keys.push(OwnedKey {
                        file: file.clone(),
                        key,
                        asset: (*name).clone(),
                        kind: KeyKind::Mcp,
                        tool: None,
                        path: Vec::new(),
                        secrets: refs,
                    });
                }
                if !forwarded.is_empty() {
                    forwarded.sort();
                    forwarded.dedup();
                    self.notes.push(PlanNote::info(format!(
                        "{tool_name} 从启动它的环境里读 {}：方寸不替它写明文（项目里没有不提交的配置），启动 {tool_name} 前在环境里设好",
                        forwarded.join("、")
                    )));
                }
            }
            for owned in self
                .old
                .keys
                .iter()
                .filter(|k| k.kind.is_mcp() && k.file == file)
            {
                let name = owned.key.rsplit('.').next().unwrap_or_default();
                let still =
                    target.tool_name.is_some() && desired.keys().any(|n| n.as_str() == name);
                if still {
                    continue;
                }
                if waiting.iter().any(|w| w == name) {
                    self.new.keys.push(owned.clone());
                    continue;
                }
                if pending.doc.remove(key_path, name) {
                    rows.push(format!("删除键 {}", owned.key));
                }
            }
            pending.rows.extend(rows);
            self.put_doc(path, pending);
        }

        // 3. 个人层：只在本机。
        let personal: Vec<(&String, &McpDecl)> = local
            .mcp
            .iter()
            .filter(|(name, _)| !waiting.contains(name))
            .filter(|(name, item)| {
                let changed = team.mcp.get(*name).is_none_or(|t| t.decl != item.decl);
                changed
                    || item
                        .decl
                        .env
                        .values()
                        .any(|v| secrets::reference(v).is_some())
            })
            .map(|(name, item)| (name, &item.decl))
            .collect();
        let project = input.root.to_string_lossy().into_owned();
        let mut no_personal: Vec<(String, Vec<String>)> = Vec::new();
        let mut handled: BTreeSet<crate::model::ToolId> = BTreeSet::new();
        let old_tools: BTreeSet<crate::model::ToolId> = self
            .old
            .keys
            .iter()
            .filter(|k| k.kind == KeyKind::McpPersonal)
            .filter_map(|k| k.tool.clone())
            .collect();
        let targets = &input.project.manifest.project.targets;
        for tool in targets.iter().chain(old_tools.iter()) {
            if !handled.insert(tool.clone()) {
                continue;
            }
            let Some(adapter) = input.adapters.get(tool) else {
                continue;
            };
            let is_target = targets.contains(tool);
            let Some(location) = adapter.mcp.iter().find(|l| l.scope == Scope::Personal) else {
                // 只写个人覆盖里加的、改过的：用了密钥的全队服务在项目层已经写了引用。
                let only_personal: Vec<String> = personal
                    .iter()
                    .filter(|(name, decl)| team.mcp.get(*name).is_none_or(|t| t.decl != **decl))
                    .map(|(name, _)| (*name).clone())
                    .collect();
                if is_target && !only_personal.is_empty() {
                    no_personal.push((adapter.tool.name.clone(), only_personal));
                }
                continue;
            };
            let Some(path) =
                crate::adapter::expand(&location.file, input.user_home, Some(input.root))
            else {
                continue;
            };
            let Some(mut pending) = self.take_doc(&path, location.format, &location.file) else {
                continue;
            };
            let key_path = project_key(&location.key, &project);
            let shown: Vec<&str> = location
                .key
                .iter()
                .map(|k| {
                    if k.contains("{project}") {
                        "<本项目>"
                    } else {
                        k.as_str()
                    }
                })
                .collect();
            let shown = shown.join(".");
            let mut rows = Vec::new();
            let wanted: Vec<&(&String, &McpDecl)> = if is_target {
                personal.iter().collect()
            } else {
                Vec::new()
            };
            for (name, decl) in &wanted {
                let mut env = BTreeMap::new();
                let mut refs = BTreeMap::new();
                for (var, value) in &decl.env {
                    match secrets::reference(value) {
                        Some(secret) => {
                            let Some(plain) = values.get(secret) else {
                                continue;
                            };
                            env.insert(var.clone(), plain.expose().to_owned());
                            refs.insert(var.clone(), secret.to_owned());
                        }
                        None => {
                            env.insert(var.clone(), value.clone());
                        }
                    }
                }
                let server = Server {
                    command: decl.command.clone(),
                    args: decl.args.clone(),
                    env,
                    forward: Vec::new(),
                };
                let key = format!("{}.{name}", location.key.join("."));
                let label = format!("{shown}.{name}");
                let owned = self.old.keys.iter().any(|k| {
                    k.kind == KeyKind::McpPersonal && k.file == location.file && k.key == key
                });
                let tail = if refs.is_empty() {
                    "（本机个人层）"
                } else {
                    "（本机个人层，密钥取自钥匙串）"
                };
                let doc = &mut pending.doc;
                match doc.get(&key_path, name, None) {
                    Some(existing) if existing == server => {}
                    Some(_) if !owned => {
                        self.replaced.push(format!("{} 的 {label}", location.file));
                        rows.push(format!("替换键 {label}{tail}"));
                    }
                    Some(_) => rows.push(format!("更新键 {label}{tail}")),
                    None => rows.push(format!("新增键 {label}{tail}")),
                }
                doc.set(&key_path, name, &server, None);
                let mut full = location.key.clone();
                full.push((*name).clone());
                self.new.keys.push(OwnedKey {
                    file: location.file.clone(),
                    key,
                    asset: (*name).clone(),
                    kind: KeyKind::McpPersonal,
                    tool: Some(tool.clone()),
                    path: full,
                    secrets: refs,
                });
            }
            for owned in self.old.keys.iter().filter(|k| {
                k.kind == KeyKind::McpPersonal
                    && k.tool.as_ref() == Some(tool)
                    && k.file == location.file
            }) {
                let name = owned.asset.as_str();
                if wanted.iter().any(|(n, _)| n.as_str() == name) {
                    continue;
                }
                if waiting.iter().any(|w| w == name) {
                    self.new.keys.push(owned.clone());
                    continue;
                }
                if pending.doc.remove(&key_path, name) {
                    rows.push(format!("删除键 {shown}.{name}（本机个人层）"));
                }
            }
            pending.rows.extend(rows);
            self.put_doc(path, pending);
        }
        for (tool_name, names) in no_personal {
            self.notes.push(PlanNote::info(format!(
                "{tool_name} 项目里没有不提交的配置，个人覆盖里的 MCP 对它不生效：{}",
                names.join("、")
            )));
        }
        Ok(())
    }

    // ───────────── 配置文件：先改在内存里，最后每个文件写一次 ─────────────

    /// 取出一份要改的配置。同一个文件 MCP 和按项目停用都会改（例如 `.codex/config.toml`），
    /// 计划里只能写一次，所以改动都攒在这里。读不懂的文件记一条提示，不改。
    fn take_doc(&mut self, path: &Path, format: FileFormat, label: &str) -> Option<PendingDoc> {
        if let Some(pending) = self.docs.remove(path) {
            return Some(pending);
        }
        if self.bad_docs.contains(path) {
            return None;
        }
        let original = std::fs::read_to_string(path).ok();
        match ConfigDoc::parse(format, original.as_deref()) {
            Ok(doc) => Some(PendingDoc {
                doc,
                original,
                rows: Vec::new(),
            }),
            Err(e) => {
                self.notes
                    .push(PlanNote::warning(format!("{label} 读不懂（{e}），没有改")));
                self.bad_docs.insert(path.to_path_buf());
                None
            }
        }
    }

    fn put_doc(&mut self, path: PathBuf, pending: PendingDoc) {
        self.docs.insert(path, pending);
    }

    /// 把攒下的配置改动写进计划。
    fn flush_docs(&mut self) -> Result<()> {
        for (path, pending) in std::mem::take(&mut self.docs) {
            if pending.rows.is_empty() {
                continue;
            }
            let new_text = pending.doc.render();
            if pending.original.as_deref() == Some(new_text.as_str()) {
                continue;
            }
            let rows = &pending.rows;
            let detail = if rows.len() <= 2 {
                format!("{}，其余键不动", rows.join("、"))
            } else {
                format!("{} 处键的变更，其余键不动", rows.len())
            };
            self.builder
                .write_file(&path, new_text.into_bytes(), detail)?;
        }
        Ok(())
    }

    // ───────────── 指令 ─────────────

    fn sync_instructions(&mut self) -> Result<()> {
        let local = self.input.project.local_declarations();
        let Some(item) = &local.instructions else {
            return Ok(());
        };
        let canonical = item.decl.canonical.clone();
        if !self.input.root.join(&canonical).is_file() {
            self.notes.push(PlanNote::warning(format!(
                "清单里写的正本 {canonical} 不在项目里，镜像没有同步"
            )));
            return Ok(());
        }
        for mirror in &item.decl.mirror {
            let path = self.input.root.join(mirror);
            let want = format!("@{canonical}\n");
            let hash = hash_bytes(want.as_bytes());
            let current = State::read(&path)?;
            let points_at_canonical = matches!(&current, State::Symlink(t)
                if t.file_name().is_some_and(|n| n.to_string_lossy() == canonical));
            let same = std::fs::read(&path).ok().as_deref() == Some(want.as_bytes())
                && matches!(current, State::File(_));
            if !same && !points_at_canonical {
                let owned = self.old.path(mirror).is_some();
                let detail = match current {
                    State::Absent => format!("镜像正本 {canonical}"),
                    _ if owned => format!("改回对正本 {canonical} 的引用"),
                    _ => {
                        self.replaced.push(mirror.clone());
                        format!("改成对正本 {canonical} 的引用；原内容已备份")
                    }
                };
                self.builder.write_file(&path, want.into_bytes(), detail)?;
            }
            self.new.paths.push(OwnedPath {
                path: mirror.clone(),
                asset: canonical.clone(),
                kind: PathKind::Mirror,
                target: None,
                hash: Some(hash.0),
            });
        }
        Ok(())
    }

    // ───────────── 去掉的项 ─────────────

    /// 台账里有、这次没有的路径：还是方寸写的样子就删，被人改过就留着并提示。
    fn remove_dropped(&mut self) -> Result<()> {
        let keep: BTreeSet<String> = self.new.paths.iter().map(|p| p.path.clone()).collect();
        let dropped: Vec<OwnedPath> = self
            .old
            .paths
            .iter()
            .filter(|p| !keep.contains(&p.path))
            .cloned()
            .collect();
        // 先删工具目录里的链接，再删实体。
        let mut ordered = dropped;
        ordered.sort_by_key(|p| p.kind != PathKind::ToolLink);
        for owned in ordered {
            let path = self.input.root.join(&owned.path);
            let current = State::read(&path)?;
            if current == State::Absent {
                continue;
            }
            let untouched = match owned.kind {
                PathKind::SkillLink | PathKind::ToolLink => {
                    owned.target.as_ref().map(PathBuf::from).map(State::Symlink)
                        == Some(current.clone())
                }
                PathKind::SkillDir => {
                    current == State::Dir
                        && owned.hash.as_deref() == Some(hash_dir(&path).0.as_str())
                }
                PathKind::Mirror => std::fs::read(&path)
                    .ok()
                    .is_some_and(|b| owned.hash.as_deref() == Some(hash_bytes(&b).0.as_str())),
            };
            if !untouched {
                self.notes.push(PlanNote::warning(format!(
                    "{} 清单里已经去掉，但本机这份被改过，没有删（漂移处理在 M1-15）",
                    owned.path
                )));
                self.new.paths.push(owned);
                continue;
            }
            let detail = format!("清单里去掉了 {}", owned.asset);
            if current == State::Dir {
                let count = self.remove_children(&path)?;
                self.builder
                    .remove(&path, format!("{detail}，删除 {count} 个文件"))?;
            } else {
                self.builder.remove(&path, detail)?;
            }
        }
        Ok(())
    }

    // ───────────── .gitignore ─────────────

    fn sync_gitignore(&mut self) -> Result<()> {
        let root = self.input.root;
        if !root.join(".git").exists() {
            return Ok(());
        }
        let path = root.join(".gitignore");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut block = vec![IGNORE_BEGIN.to_owned()];
        block.extend(self.ignored.iter().cloned());
        block.push(IGNORE_END.to_owned());
        let block = block.join("\n");
        let new_text = match (text.find(IGNORE_BEGIN), text.find(IGNORE_END)) {
            (Some(start), Some(end)) if end > start => {
                let end = end + IGNORE_END.len();
                format!("{}{block}{}", &text[..start], &text[end..])
            }
            _ if text.is_empty() => format!("{block}\n"),
            _ if text.ends_with('\n') => format!("{text}\n{block}\n"),
            _ => format!("{text}\n\n{block}\n"),
        };
        if new_text == text {
            return Ok(());
        }
        self.builder.write_file(
            &path,
            new_text.into_bytes(),
            format!("方寸管理的一段：{} 条，其余行不动", self.ignored.len()),
        )
    }
}

/// 资产库里的正本：先校验内容哈希（7.5）；库里没有就按锁里记的提交重新取，对不上不收。
fn ensure_object(
    library: &Library,
    sources: &Sources,
    name: &str,
    locked: &LockedSkill,
) -> Result<PathBuf> {
    if let Some(object) = library.locate(&locked.hash)? {
        if hash_dir(&object) == locked.hash {
            return Ok(object);
        }
        return Err(Error::Source(format!(
            "{name}：资产库里的正本被改过，内容和锁里的哈希对不上"
        )));
    }
    let policy = locked
        .commit
        .as_ref()
        .map(|c| VersionPolicy::Pin(c.clone()));
    let resolved = sources.resolve_skill(
        library,
        name,
        &SourceRef(locked.source.clone()),
        policy.as_ref(),
        Some(&locked.path),
    )?;
    if resolved.version.hash != locked.hash {
        return Err(Error::Source(format!(
            "{name}：从 {} 取到的内容和锁里的哈希对不上（7.5），可能被改过",
            locked.source
        )));
    }
    library
        .locate(&locked.hash)?
        .ok_or_else(|| Error::NotFound(format!("资产库里的 {name}")))
}

fn sha_state(bytes: &[u8]) -> ContentHash {
    ContentHash(super::snapshot::sha256_hex(bytes))
}

fn file_mode(path: &Path) -> u32 {
    super::snapshot::file_mode(path)
}

// ───────────── 配置文件按键合并 ─────────────

/// 写进工具配置的一条 MCP 服务：方寸只管这几个字段。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Server {
    command: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    /// 按名字从环境里转发的变量（Codex 的 `env_vars`）。
    forward: Vec<String>,
}

/// 一份项目层的 MCP 配置，和它属于哪个目标工具。不再是目标的（只剩台账里的键要删）没有工具。
struct ProjectMcpFile {
    file: String,
    format: FileFormat,
    key_path: Vec<String>,
    fields: Option<crate::adapter::McpFields>,
    tool_name: Option<String>,
}

impl ProjectMcpFile {
    fn forward_field(&self) -> Option<&str> {
        self.fields.as_ref()?.env_forward.as_deref()
    }

    /// 提交进仓库的写法：`secret:` 引用换成环境变量引用（7.5、9.3）。
    /// 返回服务和没法引用、只好不写的变量。
    fn committed(&self, decl: &McpDecl) -> (Server, Vec<String>) {
        let reference = self.fields.as_ref().and_then(|f| f.env_reference.as_ref());
        let forward = self.forward_field();
        let mut env = BTreeMap::new();
        let mut forwarded = Vec::new();
        let mut dropped = Vec::new();
        for (var, value) in &decl.env {
            if secrets::reference(value).is_none() {
                env.insert(var.clone(), value.clone());
            } else if let Some(reference) = reference {
                env.insert(var.clone(), reference.render(var));
            } else if forward.is_some() {
                forwarded.push(var.clone());
            } else {
                dropped.push(var.clone());
            }
        }
        let server = Server {
            command: decl.command.clone(),
            args: decl.args.clone(),
            env,
            forward: forwarded,
        };
        (server, dropped)
    }
}

/// 键路径里的 `{project}` 换成项目根目录。
fn project_key(key: &[String], project: &str) -> Vec<String> {
    key.iter()
        .map(|segment| segment.replace("{project}", project))
        .collect()
}

/// 一份 MCP 配置。JSON 用保序的 `serde_json`，TOML 用 `toml_edit`，都保住别的键和顺序。
enum ConfigDoc {
    Json(Value),
    Toml(toml_edit::DocumentMut),
}

impl ConfigDoc {
    fn parse(format: FileFormat, text: Option<&str>) -> std::result::Result<Self, String> {
        let text = text.unwrap_or_default();
        match format {
            FileFormat::Json if text.trim().is_empty() => {
                Ok(Self::Json(Value::Object(serde_json::Map::new())))
            }
            FileFormat::Json => serde_json::from_str(text)
                .map(Self::Json)
                .map_err(|e| e.to_string()),
            FileFormat::Toml => text
                .parse::<toml_edit::DocumentMut>()
                .map(Self::Toml)
                .map_err(|e| e.to_string()),
        }
    }

    /// 服务现在的启动配置（只看 command、args、env 和转发列表 `forward` 四个字段）。
    fn get(&self, key_path: &[String], name: &str, forward: Option<&str>) -> Option<Server> {
        let strings = |items: Vec<Option<&str>>| -> Vec<String> {
            items.into_iter().flatten().map(str::to_owned).collect()
        };
        match self {
            Self::Json(root) => {
                let mut node = root;
                for key in key_path {
                    node = node.get(key)?;
                }
                let entry = node.get(name)?;
                let list = |field: &str| {
                    entry
                        .get(field)
                        .and_then(Value::as_array)
                        .map(|a| strings(a.iter().map(Value::as_str).collect()))
                        .unwrap_or_default()
                };
                Some(Server {
                    command: entry.get("command")?.as_str()?.to_owned(),
                    args: list("args"),
                    env: entry
                        .get("env")
                        .and_then(Value::as_object)
                        .map(|o| {
                            o.iter()
                                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                                .collect()
                        })
                        .unwrap_or_default(),
                    forward: forward.map(list).unwrap_or_default(),
                })
            }
            Self::Toml(doc) => {
                let mut item = doc.as_item();
                for key in key_path {
                    item = item.get(key)?;
                }
                let entry = item.get(name)?;
                let list = |field: &str| {
                    entry
                        .get(field)
                        .and_then(|a| a.as_array())
                        .map(|a| strings(a.iter().map(|v| v.as_str()).collect()))
                        .unwrap_or_default()
                };
                Some(Server {
                    command: entry.get("command")?.as_str()?.to_owned(),
                    args: list("args"),
                    env: entry
                        .get("env")
                        .and_then(|e| e.as_table_like())
                        .map(|t| {
                            t.iter()
                                .filter_map(|(k, v)| Some((k.to_owned(), v.as_str()?.to_owned())))
                                .collect()
                        })
                        .unwrap_or_default(),
                    forward: forward.map(list).unwrap_or_default(),
                })
            }
        }
    }

    /// 写入服务的 command、args、env（和转发列表），别的字段不动。
    fn set(&mut self, key_path: &[String], name: &str, server: &Server, forward: Option<&str>) {
        match self {
            Self::Json(root) => {
                let mut node = root;
                for key in key_path {
                    if !node.is_object() {
                        *node = Value::Object(serde_json::Map::new());
                    }
                    let Value::Object(map) = node else {
                        return;
                    };
                    node = map
                        .entry(key.clone())
                        .or_insert_with(|| Value::Object(serde_json::Map::new()));
                }
                if !node.is_object() {
                    *node = Value::Object(serde_json::Map::new());
                }
                let Some(servers) = node.as_object_mut() else {
                    return;
                };
                let entry = servers
                    .entry(name.to_owned())
                    .or_insert_with(|| Value::Object(serde_json::Map::new()));
                if !entry.is_object() {
                    *entry = Value::Object(serde_json::Map::new());
                }
                let Some(fields) = entry.as_object_mut() else {
                    return;
                };
                let list = |items: &[String]| {
                    Value::Array(items.iter().cloned().map(Value::String).collect())
                };
                fields.insert("command".to_owned(), Value::String(server.command.clone()));
                if server.args.is_empty() {
                    fields.remove("args");
                } else {
                    fields.insert("args".to_owned(), list(&server.args));
                }
                if server.env.is_empty() {
                    fields.remove("env");
                } else {
                    let env = server
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                        .collect();
                    fields.insert("env".to_owned(), Value::Object(env));
                }
                if let Some(field) = forward {
                    if server.forward.is_empty() {
                        fields.remove(field);
                    } else {
                        fields.insert(field.to_owned(), list(&server.forward));
                    }
                }
            }
            Self::Toml(doc) => {
                let mut table = doc.as_table_mut();
                for key in key_path {
                    let item = table.entry(key).or_insert_with(|| {
                        let mut t = toml_edit::Table::new();
                        t.set_implicit(true);
                        toml_edit::Item::Table(t)
                    });
                    if !item.is_table() {
                        let mut t = toml_edit::Table::new();
                        t.set_implicit(true);
                        *item = toml_edit::Item::Table(t);
                    }
                    let Some(next) = item.as_table_mut() else {
                        return;
                    };
                    table = next;
                }
                let item = table
                    .entry(name)
                    .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
                if !item.is_table_like() {
                    *item = toml_edit::Item::Table(toml_edit::Table::new());
                }
                let Some(fields) = item.as_table_like_mut() else {
                    return;
                };
                let list = |items: &[String]| -> toml_edit::Array {
                    items.iter().map(String::as_str).collect()
                };
                fields.insert("command", toml_edit::value(server.command.clone()));
                if server.args.is_empty() {
                    fields.remove("args");
                } else {
                    fields.insert("args", toml_edit::value(list(&server.args)));
                }
                if server.env.is_empty() {
                    fields.remove("env");
                } else {
                    let mut env = toml_edit::InlineTable::new();
                    for (k, v) in &server.env {
                        env.insert(k, v.as_str().into());
                    }
                    fields.insert("env", toml_edit::value(env));
                }
                if let Some(field) = forward {
                    if server.forward.is_empty() {
                        fields.remove(field);
                    } else {
                        fields.insert(field, toml_edit::value(list(&server.forward)));
                    }
                }
            }
        }
    }

    fn remove(&mut self, key_path: &[String], name: &str) -> bool {
        match self {
            Self::Json(root) => {
                let mut node = root;
                for key in key_path {
                    let Some(next) = node.get_mut(key) else {
                        return false;
                    };
                    node = next;
                }
                node.as_object_mut()
                    .is_some_and(|o| o.shift_remove(name).is_some())
            }
            Self::Toml(doc) => {
                let mut item = doc.as_item_mut();
                for key in key_path {
                    let Some(next) = item.get_mut(key) else {
                        return false;
                    };
                    item = next;
                }
                item.as_table_like_mut()
                    .is_some_and(|t| t.remove(name).is_some())
            }
        }
    }

    fn render(&self) -> String {
        match self {
            Self::Json(value) => {
                let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
                text.push('\n');
                text
            }
            Self::Toml(doc) => doc.to_string(),
        }
    }
}

mod disable;

pub use disable::Visible;

#[cfg(all(test, unix))]
mod tests;
