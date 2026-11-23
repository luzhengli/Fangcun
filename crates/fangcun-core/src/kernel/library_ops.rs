//! 资产库与收编一批的用例（批 6）：收编（含 MCP 入库，M1-20）、从全局移除（M1-34、M1-02）、
//! `fangcun add` 与搜索（M1-22）、体检的处理进度（M1-28）。

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Kernel;
use crate::global::{GlobalItemRef, GlobalOverview};
use crate::journal::{RecordFilter, RecordKind};
use crate::library::Library;
use crate::library::mcp::McpSpec;
use crate::library::source::Source;
use crate::manifest::{McpDecl, SkillDecl};
use crate::materializer::{ChangePlan, PlanBuilder, SnapshotScope};
use crate::model::{AssetKind, PlanNote, Scope, SourceRef, ToolId};
use crate::scanner::{CheckupReport, McpEntry, SkillCopy};
use crate::{Error, Result};

/// 搜索结果从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum HitOrigin {
    /// 本机资产库。
    Library,
    /// 公开市场 skills.sh（F2）。
    Market,
}

/// 命令面板 `⌘K`、`fangcun add` 的一条搜索结果（6.9、M1-22）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AssetHit {
    pub name: String,
    pub kind: AssetKind,
    /// 清单里的来源写法。资产库里来源未知的为空，这样的项加不进清单（F1）。
    pub source: Option<String>,
    pub origin: HitOrigin,
    /// 交给 `plan_add` 的写法，例如 `tdd` 或 `mattpocock/skills@tdd`。来源未知的为空。
    pub add: Option<String>,
    /// skills.sh 统计的安装次数。资产库里的为空。
    pub installs: Option<u32>,
}

/// `fangcun add` 解析出来要加的一项。
enum Addition {
    Skill(String, SkillDecl),
    Mcp(String, McpDecl),
}

impl Kernel {
    // ───────────── 收编（F1、M1-12、M1-20） ─────────────

    /// `fangcun adopt <资产>`：收编进资产库。
    ///
    /// skill：扫描到的、叫这个名字的副本（全局层和各项目）存进资产库，工具目录里的实体目录换成
    /// 指向正本的软链，规则见 `materializer::adopt`。没有叫这个名字的 skill 时找 MCP：每种写法
    /// 存一版，`env` 的明文换成 `secret:` 引用，明文在应用时存进钥匙串，工具配置不动。
    pub fn plan_adopt(&self, asset: &str) -> Result<ChangePlan> {
        let builder = self.adopt_builder(asset)?;
        Ok(self.issue(builder))
    }

    fn adopt_builder(&self, asset: &str) -> Result<PlanBuilder> {
        self.require_write("adopt")?;
        let scan = self.scan();
        let mut copies: Vec<&SkillCopy> = Vec::new();
        let mut entries: Vec<&McpEntry> = Vec::new();
        let all = scan
            .global
            .values()
            .chain(scan.project_items.values().flat_map(|p| p.values()));
        for collected in all {
            copies.extend(collected.skills.iter().filter(|s| s.name == asset));
            entries.extend(collected.mcp.iter().filter(|m| m.name == asset));
        }
        let library = Library::open(&self.home)?;
        let mut builder = PlanBuilder::new(String::new(), RecordKind::Adopt, SnapshotScope::Global)
            .command(format!("fangcun adopt {asset}"));
        if copies.is_empty() && !entries.is_empty() {
            crate::materializer::adopt::plan_mcp(
                &crate::materializer::adopt::McpAdoptInput {
                    name: asset,
                    entries,
                    user_home: &self.user_home,
                    library: &library,
                    secrets: self.secrets.as_ref(),
                },
                &mut builder,
            )?;
            return Ok(builder);
        }
        let scope = self.write_scope()?;
        crate::materializer::adopt::plan(
            &crate::materializer::adopt::AdoptInput {
                name: asset,
                copies,
                home: &self.home,
                user_home: &self.user_home,
                scope: &scope,
                library: &library,
            },
            &mut builder,
        )?;
        if !entries.is_empty() {
            builder.note(PlanNote::info(format!(
                "还有叫 {asset} 的 MCP 服务，这次只收录 skill"
            )));
        }
        Ok(builder)
    }

    // ───────────── 体检（6.2、M1-26、M1-28） ─────────────

    /// 体检报告的一键动作（6.2、M1-26）：按发现的编号分派到对应的写入用例。
    /// 「重复」收编进资产库；其余几类等各自的条目（M1-19、M1-17 等）接上。
    /// 计划记下发现的编号，应用后这一条算已处理（M1-28）。
    pub fn plan_finding(&self, finding_id: &str) -> Result<ChangePlan> {
        if !crate::scanner::finding_ready(finding_id) {
            return Err(Error::NotImplemented("kernel::plan_finding"));
        }
        let mut builder = match finding_id.split_once(':') {
            Some(("duplicate", asset)) => self.adopt_builder(asset)?,
            _ => return Err(Error::NotImplemented("kernel::plan_finding")),
        };
        builder.finding(finding_id);
        Ok(self.issue(builder))
    }

    /// 给报告标上处理进度：这次扫描之后体检一键动作留下、没回滚的记录（M1-28）。
    pub(super) fn with_progress(&self, mut report: CheckupReport) -> CheckupReport {
        // 读不了活动记录时照常给报告，只是没有进度。
        if let Ok(records) = self.journal.read(&RecordFilter::default()) {
            crate::scanner::mark_handled(&mut report, &records);
        }
        report
    }

    // ───────────── 全局（6.6、M1-34、M1-02） ─────────────

    /// 全局页每一行标上「已收编」：从全局移除只对这样的行开放（7.5）。
    pub(super) fn mark_in_library(&self, overview: &mut GlobalOverview) {
        let Ok(library) = Library::open(&self.home) else {
            return;
        };
        let scan = self.scan();
        let Some(global) = scan.global.get(&overview.tool) else {
            return;
        };
        for row in &mut overview.rows {
            row.in_library = match row.kind {
                AssetKind::Skill => {
                    let copies: Vec<&SkillCopy> = global
                        .skills
                        .iter()
                        .filter(|s| s.scope == Scope::Global && row.members.contains(&s.name))
                        .collect();
                    !copies.is_empty()
                        && copies
                            .iter()
                            .all(|c| library.locate(&c.hash).ok().flatten().is_some())
                }
                AssetKind::Mcp => global
                    .mcp
                    .iter()
                    .filter(|m| m.scope == Scope::Global && row.members.contains(&m.name))
                    .all(|m| mcp_in_library(&library, m)),
                AssetKind::Instruction => false,
            };
        }
    }

    /// 全局页、检查器被覆盖块与同名冲突的「从全局移除…」，`fangcun remove <资产> --global`
    /// （6.6、6.3、M1-34、M1-02）。
    ///
    /// `tool` 是全局页当前选中的目标工具；为空时是全部完整支持的工具。只对已收编进资产库的项
    /// 开放（7.5），没收编的返回 `Error::NotAdopted`。规则见 `materializer::global_remove`。
    pub fn plan_remove_global(
        &self,
        tool: Option<&ToolId>,
        items: &[GlobalItemRef],
    ) -> Result<ChangePlan> {
        self.require_write("remove")?;
        let scan = self.scan();
        let tools = match tool {
            Some(tool) => vec![self.adapter(tool)?],
            None => scan
                .tools
                .iter()
                .filter_map(|t| self.adapters.get(t))
                .collect(),
        };
        let library = Library::open(&self.home)?;
        let scope = self.write_scope()?;
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        let mut command = format!("fangcun remove {} --global", names.join(" "));
        if let Some(tool) = tool {
            command.push_str(&format!(" --tool {}", tool.as_str()));
        }
        let mut builder = PlanBuilder::new(String::new(), RecordKind::Sync, SnapshotScope::Global)
            .command(command);
        crate::materializer::global_remove::plan(
            &crate::materializer::global_remove::RemoveInput {
                tools,
                adapters: &self.adapters,
                items,
                scan: &scan,
                home: &self.home,
                user_home: &self.user_home,
                scope: &scope,
                library: &library,
                secrets: self.secrets.as_ref(),
            },
            &mut builder,
        )?;
        Ok(self.issue(builder))
    }

    // ───────────── 加入清单（6.9、M1-22） ─────────────

    /// 命令面板 `⌘K` 的搜索：资产库里名字包含这个词的，`market` 为真时再搜 skills.sh（联网，
    /// 只发搜索词，F2）。资产库的在前。
    pub fn search_assets(&self, query: &str, market: bool) -> Result<Vec<AssetHit>> {
        let query = query.trim();
        let needle = query.to_lowercase();
        let mut hits: Vec<AssetHit> = Library::open(&self.home)?
            .entries()?
            .into_iter()
            .filter(|e| e.kind != AssetKind::Instruction)
            .filter(|e| needle.is_empty() || e.name.to_lowercase().contains(&needle))
            .map(|e| {
                let add = (e.source.is_some() || e.kind == AssetKind::Mcp).then(|| e.name.clone());
                AssetHit {
                    name: e.name,
                    kind: e.kind,
                    source: e.source.map(|s| s.0),
                    origin: HitOrigin::Library,
                    add,
                    installs: None,
                }
            })
            .collect();
        if market && !query.is_empty() {
            for hit in crate::library::market::search(query)? {
                let short = hit.source.0.trim_start_matches("github:").to_owned();
                hits.push(AssetHit {
                    add: Some(format!("{short}@{}", hit.name)),
                    name: hit.name,
                    kind: AssetKind::Skill,
                    source: Some(hit.source.0),
                    origin: HitOrigin::Market,
                    installs: Some(u32::try_from(hit.installs).unwrap_or(u32::MAX)),
                });
            }
        }
        Ok(hits)
    }

    /// `fangcun add <资产>`、命令面板回车（6.9、M1-22）：加进项目清单，更新锁，按清单物化，一份计划。
    ///
    /// `<资产>` 的写法（DECISIONS 2026-11-21 批 6，D-38）：
    /// - `<来源>@<名字>`：来源是 `github:<owner>/<repo>`、`<owner>/<repo>` 或 `local:<名字>`；
    /// - 资产库里的名字：skill 用库里记的来源，MCP 用库里最新的一版（密钥是 `secret:` 引用）；
    /// - 都不是时搜 skills.sh，名字完全相同的只有一条才用它，多条就列出来让人写成 `<来源>@<名字>`。
    pub fn plan_add(&self, project_root: &Path, asset: &str) -> Result<ChangePlan> {
        self.require_write("add")?;
        let (mut project, lock) = self.manifest_and_lock(project_root)?;
        let library = Library::open(&self.home)?;
        let addition = self.resolve_addition(&library, asset.trim())?;
        let name = match &addition {
            Addition::Skill(name, _) | Addition::Mcp(name, _) => name.clone(),
        };
        let project_name = crate::scanner::project_name(project_root);
        let mut builder = PlanBuilder::new(
            format!("把 {name} 加进 {project_name}"),
            RecordKind::Sync,
            SnapshotScope::Project(project_root.to_path_buf()),
        )
        .command(format!("fangcun add {asset}"));
        builder.subject(name.clone(), None);
        let declared = match &addition {
            Addition::Skill(..) => project.manifest.skills.contains_key(&name),
            Addition::Mcp(..) => project.manifest.mcp.contains_key(&name),
        };
        if declared {
            builder.set_title(format!("{name} 已在 {project_name} 的清单里"));
            return Ok(self.issue(builder));
        }

        let file = project_root.join(crate::paths::MANIFEST_FILE);
        let text = std::fs::read_to_string(&file).map_err(|source| Error::Io {
            path: file.clone(),
            source,
        })?;
        let origin = crate::paths::MANIFEST_FILE;
        let (edited, detail) = match &addition {
            Addition::Skill(name, decl) => (
                crate::manifest::add_skill(origin, &text, name, decl)?,
                format!("[skills] 加入 {name}（{}）", decl.source),
            ),
            Addition::Mcp(name, decl) => (
                crate::manifest::add_mcp(origin, &text, name, decl)?,
                format!("加入 [mcp.{name}]"),
            ),
        };
        project.manifest = crate::manifest::parse_manifest(origin, &edited)?;
        builder.write_file(&file, edited.into_bytes(), detail)?;

        let sources = self.sources();
        let next = crate::resolver::resolve_lock(&project, Some(&lock), &sources, &library)?;
        let lock_text = next.to_toml()?;
        let lock_file = project_root.join(crate::paths::LOCK_FILE);
        if std::fs::read(&lock_file).ok().as_deref() != Some(lock_text.as_bytes()) {
            builder.write_file(
                &lock_file,
                lock_text.into_bytes(),
                super::lock_summary(Some(&lock), &next),
            )?;
        }
        if let Addition::Mcp(_, decl) = &addition {
            for (var, value) in &decl.env {
                let Some(secret) = crate::secrets::reference(value) else {
                    continue;
                };
                if !matches!(self.secrets.get(secret), Ok(Some(_))) {
                    builder.note(PlanNote::warning(format!(
                        "{var} 引用的密钥 {secret} 不在钥匙串里。先运行 fangcun secret set {secret}"
                    )));
                }
            }
        }
        self.sync_into(&mut builder, &project, &next, &library, &sources, &[])?;
        // 同步会把标题改成「同步 N 处变更」，这里改回来。
        builder.set_title(format!("把 {name} 加进 {project_name}"));
        Ok(self.issue(builder))
    }

    fn resolve_addition(&self, library: &Library, asset: &str) -> Result<Addition> {
        if asset.is_empty() {
            return Err(Error::NotFound("要加的资产".to_owned()));
        }
        if let Some((source, name)) = asset.rsplit_once('@') {
            let source = if source.contains(':') {
                source.to_owned()
            } else {
                format!("github:{source}")
            };
            let source = SourceRef(source);
            Source::parse(&source)?;
            return Ok(Addition::Skill(name.to_owned(), skill_decl(&source)));
        }
        let entries: Vec<_> = library
            .entries()?
            .into_iter()
            .filter(|e| e.name == asset && e.kind != AssetKind::Instruction)
            .collect();
        if let Some(skill) = entries.iter().find(|e| e.kind == AssetKind::Skill) {
            let Some(source) = &skill.source else {
                return Err(Error::Source(format!(
                    "{asset} 在资产库里来源未知，加不进清单。在资产库补上来源后再加（F1）"
                )));
            };
            return Ok(Addition::Skill(asset.to_owned(), skill_decl(source)));
        }
        if let Some(mcp) = entries.iter().find(|e| e.kind == AssetKind::Mcp) {
            let latest = mcp
                .versions
                .last()
                .ok_or_else(|| Error::NotFound(format!("资产库里 {asset} 的版本")))?;
            let spec: McpSpec = library
                .read_mcp(&latest.hash)?
                .ok_or_else(|| Error::NotFound(format!("资产库里 {asset} 的正本")))?;
            return Ok(Addition::Mcp(
                asset.to_owned(),
                McpDecl {
                    command: spec.command,
                    args: spec.args,
                    env: spec.env,
                },
            ));
        }
        let hits = crate::library::market::search(asset)?;
        let exact: Vec<_> = hits.iter().filter(|h| h.name == asset).collect();
        match exact.as_slice() {
            [one] => Ok(Addition::Skill(asset.to_owned(), skill_decl(&one.source))),
            [] => {
                let near: Vec<String> = hits
                    .iter()
                    .take(5)
                    .map(|h| format!("{}@{}", h.source.0.trim_start_matches("github:"), h.name))
                    .collect();
                Err(Error::NotFound(if near.is_empty() {
                    format!("资产库和 skills.sh 里叫 {asset} 的资产")
                } else {
                    format!(
                        "叫 {asset} 的资产。skills.sh 上相近的有：{}",
                        near.join("、")
                    )
                }))
            }
            many => {
                let list: Vec<String> = many
                    .iter()
                    .map(|h| format!("{}@{}", h.source.0.trim_start_matches("github:"), h.name))
                    .collect();
                Err(Error::Source(format!(
                    "skills.sh 上有 {} 个叫 {asset} 的 skill，写成 <来源>@<名字> 指定一个：{}",
                    many.len(),
                    list.join("、")
                )))
            }
        }
    }
}

fn skill_decl(source: &SourceRef) -> SkillDecl {
    SkillDecl {
        source: source.0.clone(),
        pin: None,
        track: None,
        path: None,
    }
}

/// 全局层的这条 MCP 换成 `secret:` 引用后的那一版在不在资产库里。
fn mcp_in_library(library: &Library, entry: &McpEntry) -> bool {
    let Some(command) = &entry.command else {
        return false;
    };
    let (spec, _) = McpSpec::from_config(&entry.name, command, &entry.args, &entry.env);
    spec.hash()
        .ok()
        .and_then(|h| library.locate(&h).ok().flatten())
        .is_some()
}
