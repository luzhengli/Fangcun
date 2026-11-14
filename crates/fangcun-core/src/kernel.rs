//! 内核门面。桌面外壳和命令行只调这里，不直接碰下面的模块。
//!
//! 一个用例一个方法。分两组：
//!
//! - **只读用例**：M0 的全部内容。任何访问方式下都能调。
//! - **写入用例**：一律先返回 [`ChangePlan`]，由 [`Kernel::apply`] 落盘。
//!   内核以 [`Access::ReadOnly`] 打开时，这一组直接返回 [`Error::ReadOnly`]。
//!
//! 内核是同步的，不依赖异步运行时。桌面外壳在阻塞线程里调它。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::{Adapter, AdapterRegistry, ToolInfo};
use crate::global::GlobalOverview;
use crate::journal::{
    FileOp, Journal, Record, RecordFile, RecordFilter, RecordKind, RecordStatus, Trigger,
};
use crate::materializer::{Applied, ChangePlan};
use crate::model::{AssetState, ProjectId, SnapshotId, Timestamp, ToolId};
use crate::paths::FangcunHome;
use crate::resolver::{EffectiveContext, Explanation};
use crate::scanner::fsutil::display_path;
use crate::scanner::{CheckupReport, Collected, Reach, Scan, ScanOptions};
use crate::{Error, Result};

/// 内核的访问方式。M0 的桌面应用和命令行都用只读。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// 不写任何文件，包括 `~/.fangcun`。
    ReadOnly,
    ReadWrite,
}

/// 侧栏五个导航项旁的数字。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Overview {
    pub projects: u32,
    pub library: u32,
    pub bundles: u32,
    pub global: u32,
    /// 活动页的待处理数，与菜单栏角标一致。
    pub pending: u32,
}

/// 侧栏里的一个项目。
///
/// M0 没有登记：项目是近 30 天会话记录里出现过的工作目录，归到 git 根目录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectSummary {
    /// 项目根目录的绝对路径。
    pub id: ProjectId,
    pub name: String,
    /// 用 `~` 写的路径。
    pub path: String,
    /// 状态点。M0 没有受管的资产，为空时用灰点。
    pub state: Option<AssetState>,
    pub last_seen: Option<Timestamp>,
    pub sessions: u32,
    pub tools: Vec<ToolId>,
}

/// `fangcun status`：当前项目的同步状态与漂移。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectStatus {
    pub has_manifest: bool,
    pub drifted: Vec<String>,
    pub updatable: Vec<String>,
}

#[derive(Debug)]
pub struct Kernel {
    home: FangcunHome,
    adapters: AdapterRegistry,
    access: Access,
    user_home: PathBuf,
    origin: Trigger,
    extra_projects: Vec<PathBuf>,
    scan: Mutex<Option<Arc<Scan>>>,
    journal: Journal,
}

impl Kernel {
    /// 按默认位置打开。只算路径、读适配器，不创建任何目录，也不扫描。
    pub fn open(access: Access) -> Result<Self> {
        Self::with_home(FangcunHome::locate()?, access)
    }

    pub fn with_home(home: FangcunHome, access: Access) -> Result<Self> {
        let adapters = AdapterRegistry::load(&home)?;
        let user_home = std::env::home_dir().ok_or(Error::NoHomeDir)?;
        let journal = Journal::open(&home);
        Ok(Self {
            home,
            adapters,
            access,
            user_home,
            origin: Trigger::Desktop,
            extra_projects: Vec::new(),
            scan: Mutex::new(None),
            journal,
        })
    }

    /// 换一个用户主目录。测试和回归样例用它指向样例目录。
    #[must_use]
    pub fn with_user_home(mut self, user_home: impl Into<PathBuf>) -> Self {
        self.user_home = user_home.into();
        self
    }

    /// 记录里的触发方。命令行用 [`Trigger::Cli`]。
    #[must_use]
    pub fn with_origin(mut self, origin: Trigger) -> Self {
        self.origin = origin;
        self
    }

    /// 额外要扫的项目目录，例如命令行的当前目录。
    #[must_use]
    pub fn with_project(mut self, root: impl Into<PathBuf>) -> Self {
        self.extra_projects.push(root.into());
        self
    }

    pub fn home(&self) -> &FangcunHome {
        &self.home
    }

    pub fn access(&self) -> Access {
        self.access
    }

    fn require_write(&self, use_case: &'static str) -> Result<()> {
        match self.access {
            Access::ReadWrite => Ok(()),
            Access::ReadOnly => Err(Error::ReadOnly(use_case)),
        }
    }

    fn adapter(&self, tool: &ToolId) -> Result<&Adapter> {
        self.adapters
            .get(tool)
            .ok_or_else(|| Error::UnknownTool(tool.0.clone()))
    }

    /// 最近一次扫描。还没扫过就先扫一次。
    fn scan(&self) -> Arc<Scan> {
        if let Ok(guard) = self.scan.lock()
            && let Some(scan) = guard.as_ref()
        {
            return Arc::clone(scan);
        }
        self.rescan_inner()
    }

    fn rescan_inner(&self) -> Arc<Scan> {
        let options = ScanOptions {
            project_roots: self.extra_projects.clone(),
        };
        let scan = Arc::new(crate::scanner::scan(
            &self.adapters,
            &self.user_home,
            &options,
        ));
        let report = crate::scanner::diagnose(&scan);
        self.journal.remember(self.checkup_record(&scan, &report));
        if let Ok(mut guard) = self.scan.lock() {
            *guard = Some(Arc::clone(&scan));
        }
        scan
    }

    fn checkup_record(&self, scan: &Scan, report: &CheckupReport) -> Record {
        let first = self.journal.is_empty();
        let trigger = if first && self.origin == Trigger::Desktop {
            Trigger::FirstLaunch
        } else {
            self.origin
        };
        let title = format!(
            "{}：扫描 {} 个工具、{} 个项目目录",
            if first { "首次体检" } else { "体检" },
            report.tools_scanned,
            report.project_dirs_scanned
        );
        let mut files = Vec::new();
        for (tool, collected) in scan.global.iter().filter(|(t, _)| scan.tools.contains(t)) {
            let dirs: std::collections::BTreeSet<String> = collected
                .skills
                .iter()
                .filter_map(|s| s.dir.parent().map(|p| display_path(p, &scan.home)))
                .chain(
                    collected
                        .mcp
                        .iter()
                        .map(|m| display_path(&m.file, &scan.home)),
                )
                .chain(
                    collected
                        .instructions
                        .iter()
                        .map(|i| display_path(&i.path, &scan.home)),
                )
                .collect();
            for dir in dirs {
                files.push(RecordFile {
                    op: FileOp::Read,
                    path: dir,
                    note: Some(scan.tool_name(tool).to_owned()),
                });
            }
        }
        Record {
            id: format!("checkup-{}", scan.scanned_at.0),
            at: scan.scanned_at.clone(),
            kind: RecordKind::Checkup,
            title,
            project: None,
            project_label: "全局层".to_owned(),
            trigger,
            snapshot: None,
            status: RecordStatus::ReadOnly,
            summary: format!(
                "只读扫描，未改动任何文件。发现 {} 项资产、{} 条待处理，读了近 30 天的 {} 份会话记录。",
                report.assets_found,
                report.findings.len(),
                report.sessions_read
            ),
            files,
            command: Some("fangcun doctor".to_owned()),
            output: Some(format!("{} 条发现 · 只读", report.findings.len())),
        }
    }

    fn project_collected(&self, scan: &Scan, root: &Path, tool: &ToolId) -> Result<Collected> {
        if let Some(found) = scan.project_items.get(root).and_then(|p| p.get(tool)) {
            return Ok(found.clone());
        }
        let adapter = self.adapter(tool)?;
        Ok(crate::scanner::collect(
            adapter,
            &scan.home,
            Reach::Project(root),
            &scan.locks,
        ))
    }

    // ───────────── 只读用例（M0） ─────────────

    /// 已知的工具，供目标工具分段控件使用。
    pub fn tools(&self) -> Vec<ToolInfo> {
        self.adapters
            .iter()
            .map(|adapter| adapter.tool.clone())
            .collect()
    }

    /// `fangcun doctor`、体检报告页。用最近一次扫描，还没扫过就先扫。
    pub fn doctor(&self) -> Result<CheckupReport> {
        Ok(crate::scanner::diagnose(&self.scan()))
    }

    /// 「重新体检」：重新扫描全盘，换掉缓存。
    pub fn rescan(&self) -> Result<CheckupReport> {
        Ok(crate::scanner::diagnose(&self.rescan_inner()))
    }

    pub fn overview(&self) -> Result<Overview> {
        let scan = self.scan();
        let mut global = std::collections::BTreeSet::new();
        let full = scan
            .global
            .iter()
            .filter(|(tool, _)| scan.tools.contains(tool));
        for (_, collected) in full {
            for skill in &collected.skills {
                global.insert(format!("skill:{}", skill.name));
            }
            for entry in &collected.mcp {
                global.insert(format!("mcp:{}", entry.name));
            }
            for file in &collected.instructions {
                global.insert(format!("instruction:{}", file.real_path.display()));
            }
        }
        Ok(Overview {
            projects: u32::try_from(scan.projects.len()).unwrap_or(u32::MAX),
            library: 0,
            bundles: 0,
            global: u32::try_from(global.len()).unwrap_or(u32::MAX),
            pending: self.journal.pending_count()?,
        })
    }

    pub fn projects(&self) -> Result<Vec<ProjectSummary>> {
        let scan = self.scan();
        Ok(scan
            .projects
            .iter()
            .map(|p| ProjectSummary {
                id: ProjectId(p.root.display().to_string()),
                name: p.name.clone(),
                path: display_path(&p.root, &scan.home),
                state: None,
                last_seen: p.last_seen.clone(),
                sessions: p.sessions,
                tools: p.tools.clone(),
            })
            .collect())
    }

    /// 项目页的生效上下文。
    pub fn effective_context(
        &self,
        project_root: &Path,
        tool: &ToolId,
    ) -> Result<EffectiveContext> {
        let scan = self.scan();
        let adapter = self.adapter(tool)?;
        let project = self.project_collected(&scan, project_root, tool)?;
        Ok(crate::resolver::effective_context(
            &scan,
            adapter,
            &project,
            project_root,
        ))
    }

    /// `fangcun status`。
    pub fn status(&self, project_root: &Path) -> Result<ProjectStatus> {
        Ok(ProjectStatus {
            has_manifest: project_root.join(crate::paths::MANIFEST_FILE).is_file(),
            drifted: Vec::new(),
            updatable: Vec::new(),
        })
    }

    /// `fangcun why <资产>`：每个读到它的工具各一条。`project_root` 为空时只看全局层。
    pub fn why(&self, project_root: Option<&Path>, asset: &str) -> Result<Vec<Explanation>> {
        let scan = self.scan();
        let mut out = Vec::new();
        for tool in &scan.tools {
            let adapter = self.adapter(tool)?;
            let (project, root) = match project_root {
                Some(root) => (self.project_collected(&scan, root, tool)?, root),
                None => (Collected::default(), scan.home.as_path()),
            };
            let context = crate::resolver::effective_context(&scan, adapter, &project, root);
            if let Some(explanation) = crate::resolver::explain(&context, &adapter.tool.name, asset)
            {
                out.push(explanation);
            }
        }
        if out.is_empty() {
            return Err(Error::NotFound(asset.to_owned()));
        }
        Ok(out)
    }

    /// 全局页。
    pub fn global_overview(&self, tool: &ToolId) -> Result<GlobalOverview> {
        let scan = self.scan();
        let adapter = self.adapter(tool)?;
        Ok(crate::global::overview(&scan, &self.adapters, adapter))
    }

    /// `fangcun log`、活动页。
    pub fn log(&self, filter: &RecordFilter) -> Result<Vec<Record>> {
        self.journal.read(filter)
    }

    /// `fangcun sync --frozen`：锁与清单不一致就失败。本身不改项目文件。
    pub fn verify_frozen(&self, _project_root: &Path) -> Result<()> {
        Err(Error::NotImplemented("Kernel::verify_frozen"))
    }

    // ───────────── 写入用例（M1）：先出计划，再 apply ─────────────

    /// `fangcun init`：登记项目，生成清单与锁。
    pub fn plan_init(&self, _project_root: &Path, _from_existing: bool) -> Result<ChangePlan> {
        self.require_write("init")?;
        Err(Error::NotImplemented("Kernel::plan_init"))
    }

    /// `fangcun add <资产>`：加入清单。
    pub fn plan_add(&self, _project_root: &Path, _asset: &str) -> Result<ChangePlan> {
        self.require_write("add")?;
        Err(Error::NotImplemented("Kernel::plan_add"))
    }

    /// `fangcun adopt <资产>`：收编进资产库。
    pub fn plan_adopt(&self, _asset: &str) -> Result<ChangePlan> {
        self.require_write("adopt")?;
        Err(Error::NotImplemented("Kernel::plan_adopt"))
    }

    /// `fangcun lock`：重新解析清单，更新锁。
    pub fn plan_lock(&self, _project_root: &Path) -> Result<ChangePlan> {
        self.require_write("lock")?;
        Err(Error::NotImplemented("Kernel::plan_lock"))
    }

    /// `fangcun sync`：按清单与锁物化。
    pub fn plan_sync(&self, _project_root: &Path) -> Result<ChangePlan> {
        self.require_write("sync")?;
        Err(Error::NotImplemented("Kernel::plan_sync"))
    }

    /// `fangcun update [资产]`。`asset` 为空表示全部；`projects` 为空表示当前项目。
    pub fn plan_update(&self, _asset: Option<&str>, _projects: &[ProjectId]) -> Result<ChangePlan> {
        self.require_write("update")?;
        Err(Error::NotImplemented("Kernel::plan_update"))
    }

    /// `fangcun rollback <快照>`。
    pub fn plan_rollback(&self, _project_root: &Path, _snapshot: SnapshotId) -> Result<ChangePlan> {
        self.require_write("rollback")?;
        Err(Error::NotImplemented("Kernel::plan_rollback"))
    }

    /// 应用一份计划。写入只发生在这里。
    pub fn apply(&self, _plan: &ChangePlan) -> Result<Applied> {
        self.require_write("apply")?;
        Err(Error::NotImplemented("Kernel::apply"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kernel(access: Access) -> Kernel {
        Kernel::with_home(FangcunHome::at("/nonexistent/fangcun-test"), access)
            .expect("打开内核不应碰磁盘")
            .with_user_home("/nonexistent/fangcun-test-home")
    }

    #[test]
    fn opening_does_not_create_the_home_directory() {
        let kernel = kernel(Access::ReadOnly);
        assert!(!kernel.home().root().exists());
    }

    #[test]
    fn read_only_kernel_refuses_every_write_use_case() {
        let kernel = kernel(Access::ReadOnly);
        let root = Path::new("/nonexistent/project");
        let plan = ChangePlan {
            title: String::new(),
            changes: Vec::new(),
        };

        let refusals = [
            kernel.plan_init(root, true).err(),
            kernel.plan_add(root, "tdd").err(),
            kernel.plan_adopt("tdd").err(),
            kernel.plan_lock(root).err(),
            kernel.plan_sync(root).err(),
            kernel.plan_update(None, &[]).err(),
            kernel.plan_rollback(root, SnapshotId(1)).err(),
            kernel.apply(&plan).err(),
        ];
        for refusal in refusals {
            assert!(
                matches!(refusal, Some(Error::ReadOnly(_))),
                "得到的是 {refusal:?}"
            );
        }
    }

    #[test]
    fn first_two_tools_are_fully_supported_and_the_rest_scan_only() {
        let tools = kernel(Access::ReadOnly).tools();
        let full: Vec<&str> = tools
            .iter()
            .filter(|t| t.support == crate::adapter::SupportLevel::Full)
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(full, [ToolId::CLAUDE_CODE, ToolId::CODEX]);
        // 9.2：其余工具在 M0 只读扫描。
        assert_eq!(tools.len(), 8);
    }
}
