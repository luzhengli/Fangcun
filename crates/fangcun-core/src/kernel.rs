//! 内核门面。桌面外壳和命令行只调这里，不直接碰下面的模块。
//!
//! 一个用例一个方法。分两组：
//!
//! - **只读用例**：M0 的全部内容。任何访问方式下都能调。
//! - **写入用例**：一律先返回 [`ChangePlan`]，由 [`Kernel::apply`] 落盘。
//!   内核以 [`Access::ReadOnly`] 打开时，这一组直接返回 [`Error::ReadOnly`]。
//!   计划的动作留在内核里，界面和命令行拿回来的只是编号、标题和每一行（M1-04）。
//!
//! 内核是同步的，不依赖异步运行时。桌面外壳在阻塞线程里调它。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adapter::{Adapter, AdapterRegistry, ToolInfo};
use crate::global::GlobalOverview;
use crate::journal::{
    FileOp, Journal, Record, RecordFile, RecordFilter, RecordKind, RecordStatus, Trigger,
};
use crate::library::Library;
use crate::library::source::Sources;
use crate::manifest::Lock;
use crate::materializer::{
    Applied, ApplyContext, ChangePlan, PlanBuilder, SnapshotScope, WriteScope,
};
use crate::model::{AssetState, ProjectId, SnapshotId, Timestamp, ToolId};
use crate::paths::FangcunHome;
use crate::resolver::{EffectiveContext, Explanation};
use crate::scanner::fsutil::display_path;
use crate::scanner::{CheckupReport, Collected, Reach, Scan, ScanOptions};
use crate::{Error, Result};

/// 内存里最多留多少份没应用的计划。
const PENDING_LIMIT: usize = 64;

/// 内核的访问方式。默认可写；设置里开了只读模式或命令行加了 `--read-only` 时只读（7.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// 不写方寸目录以外的任何文件。`~/.fangcun/journal` 照写，记体检（D-15）。
    ReadOnly,
    /// 写入范围见 [`WriteScope`]。
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
    /// 已登记（F3）。侧栏按它分「已登记项目」「近 30 天的项目」两组（D-28）。
    pub registered: bool,
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
    /// 最近一次扫描，以及它记没记过体检记录。
    scan: Mutex<Option<(Arc<Scan>, bool)>>,
    journal: Journal,
    /// 换掉 GitHub 的地址，测试用本地裸仓库。
    github_base: Option<String>,
    /// 出了还没应用的计划，按编号。
    pending: Mutex<BTreeMap<u32, ChangePlan>>,
    next_plan: AtomicU32,
}

impl Kernel {
    /// 按默认位置打开。只算路径、读适配器，不创建任何目录，也不扫描。
    pub fn open(access: Access) -> Result<Self> {
        Self::with_home(FangcunHome::locate()?, access)
    }

    /// 按设置打开：`~/.fangcun/settings.toml` 里开了只读模式，或 `force_read_only`
    /// （命令行的 `--read-only`）时只读，否则可写（7.5）。只读文件，不创建目录。
    pub fn open_configured(force_read_only: bool) -> Result<Self> {
        let home = FangcunHome::locate()?;
        let read_only = force_read_only || crate::settings::Settings::load(&home)?.read_only;
        let access = if read_only {
            Access::ReadOnly
        } else {
            Access::ReadWrite
        };
        Self::with_home(home, access)
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
            github_base: None,
            pending: Mutex::new(BTreeMap::new()),
            next_plan: AtomicU32::new(1),
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

    /// 换掉 GitHub 的地址。测试用 `file://` 指向本地的裸仓库。
    #[must_use]
    pub fn with_github_base(mut self, base: impl Into<String>) -> Self {
        self.github_base = Some(base.into());
        self
    }

    /// 一个用例一份：同一个仓库在这一轮里只拉一次。
    fn sources(&self) -> Sources {
        let sources = Sources::open(&self.home).fetching_once();
        match &self.github_base {
            Some(base) => sources.with_github_base(base.clone()),
            None => sources,
        }
    }

    /// 写入用例只对已登记的项目出计划，免得解析了半天，应用时才发现不在写入范围里。
    fn require_registered(&self, project_root: &Path) -> Result<()> {
        if crate::registry::contains(&crate::registry::load(&self.home)?, project_root) {
            Ok(())
        } else {
            Err(Error::NotFound(format!(
                "{} 还没登记。先运行 fangcun init",
                display_path(project_root, &self.user_home)
            )))
        }
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

    /// 出一份计划：编号、留在内核里等 [`Kernel::apply`]。
    pub(crate) fn issue(&self, builder: PlanBuilder) -> ChangePlan {
        let id = self.next_plan.fetch_add(1, Ordering::Relaxed);
        let plan = builder.build(id);
        if let Ok(mut pending) = self.pending.lock() {
            pending.insert(id, plan.clone());
            // 界面每次预览都出一份新计划，只留最近的，旧的应用时会报「计划不存在」，重新预览即可。
            while pending.len() > PENDING_LIMIT {
                pending.pop_first();
            }
        }
        plan
    }

    /// 现在的写入范围：方寸目录、已登记的项目、完整支持的工具在主目录下的全局位置。
    fn write_scope(&self) -> Result<WriteScope> {
        let registered = crate::registry::load(&self.home)?;
        Ok(WriteScope::new(
            self.home.root(),
            &self.user_home,
            &self.adapters,
            registered.into_iter().map(|p| p.path),
        ))
    }

    fn adapter(&self, tool: &ToolId) -> Result<&Adapter> {
        self.adapters
            .get(tool)
            .ok_or_else(|| Error::UnknownTool(tool.0.clone()))
    }

    /// 最近一次扫描。还没扫过就先扫一次。
    ///
    /// 桌面应用的第一次扫描就是启动时的体检，记一条；命令行的 `status`、`why` 顺带扫的不记，
    /// 只有 `doctor` 记。
    fn scan(&self) -> Arc<Scan> {
        self.scan_and_record(self.origin != Trigger::Cli)
    }

    fn scan_and_record(&self, record: bool) -> Arc<Scan> {
        // 扫描期间一直拿着锁：界面启动时几个查询同时到达，只扫一次，后到的等着用同一份结果。
        let mut guard = self
            .scan
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((scan, recorded)) = guard.as_mut() {
            // 命令行先跑了别的查询、再 `doctor` 时，补记这一次扫描。
            if record && !*recorded {
                self.record_checkup(scan);
                *recorded = true;
            }
            return Arc::clone(scan);
        }
        let scan = self.build_scan(record);
        *guard = Some((Arc::clone(&scan), record));
        scan
    }

    fn rescan_inner(&self) -> Arc<Scan> {
        let mut guard = self
            .scan
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let scan = self.build_scan(true);
        *guard = Some((Arc::clone(&scan), true));
        scan
    }

    /// 扫一次全盘，`record` 时记一条体检记录。调用方持有扫描锁。
    fn build_scan(&self, record: bool) -> Arc<Scan> {
        // 登记表读不了时照常扫描，只是少了近 30 天没用过的已登记项目。
        let registered = crate::registry::load(&self.home).unwrap_or_default();
        let mut project_roots = self.extra_projects.clone();
        project_roots.extend(registered.iter().map(|p| p.path.clone()));
        let options = ScanOptions { project_roots };
        let scan = Arc::new(crate::scanner::scan(
            &self.adapters,
            &self.user_home,
            &options,
        ));
        self.register_found(&scan, &registered);
        if record {
            self.record_checkup(&scan);
        }
        scan
    }

    /// 带 `fangcun.toml` 的仓库一经发现就自动登记（F3）。只读模式下不登记。
    fn register_found(&self, scan: &Scan, registered: &[crate::registry::RegisteredProject]) {
        if self.access != Access::ReadWrite {
            return;
        }
        let found: Vec<PathBuf> = scan
            .projects
            .iter()
            .filter(|p| p.root.join(crate::paths::MANIFEST_FILE).is_file())
            .filter(|p| !crate::registry::contains(registered, &p.root))
            .map(|p| p.root.clone())
            .collect();
        // 写不进 `~/.fangcun` 时不登记，下次扫描再试。
        let _ = crate::registry::register_found(&self.home, &found);
    }

    fn record_checkup(&self, scan: &Scan) {
        let report = crate::scanner::diagnose(scan);
        // 写不进 `~/.fangcun` 时记录留在内存里，体检照常。
        let _ = self.journal.record(self.checkup_record(scan, &report));
    }

    fn checkup_record(&self, scan: &Scan, report: &CheckupReport) -> Record {
        // 以前运行留下过体检记录，就不算首次。
        let first = !self.journal.has_kind(RecordKind::Checkup);
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
            // 同一秒里可能重扫，编号加上已有记录的条数，保证唯一。
            id: format!("checkup-{}-{}", scan.scanned_at.0, self.journal.count()),
            at: scan.scanned_at.clone(),
            kind: RecordKind::Checkup,
            title,
            project: None,
            project_label: "全局层".to_owned(),
            trigger,
            snapshot: None,
            snapshots: Vec::new(),
            reverts: None,
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

    /// 设置页（6.1、M1-24）：读 `~/.fangcun/settings.toml`，没有时是默认值。
    pub fn settings(&self) -> Result<crate::settings::SettingsForm> {
        Ok((&crate::settings::Settings::load(&self.home)?).into())
    }

    /// 设置页保存。只读模式下也能保存（不然关不掉）；只读开关要重开内核才生效，由外壳负责。
    pub fn save_settings(
        &self,
        form: &crate::settings::SettingsForm,
    ) -> Result<crate::settings::SettingsForm> {
        Ok((&crate::settings::Settings::save(&self.home, form)?).into())
    }

    /// 已知的工具，供目标工具分段控件使用。
    pub fn tools(&self) -> Vec<ToolInfo> {
        self.adapters
            .iter()
            .map(|adapter| adapter.tool.clone())
            .collect()
    }

    /// `fangcun doctor`、体检报告页。用最近一次扫描，还没扫过就先扫。
    pub fn doctor(&self) -> Result<CheckupReport> {
        Ok(crate::scanner::diagnose(&self.scan_and_record(true)))
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
        let registered = crate::registry::load(&self.home)?;
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
                registered: crate::registry::contains(&registered, &p.root),
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
        let mut context =
            crate::resolver::effective_context(&scan, adapter, &project, project_root);
        // 项目预算写在 `fangcun.toml`，没写时用默认值（D-22）。
        if let Some(budget) = crate::manifest::project_budget(project_root)? {
            context.budget.limit = crate::model::TokenEstimate(budget);
        }
        Ok(context)
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

    /// 体检报告的一键动作（6.2、M1-26）：按发现的编号分派到对应的写入用例。
    /// 「重复」收编进资产库；其余几类等各自的条目（M1-19、M1-17 等）接上。
    pub fn plan_finding(&self, finding_id: &str) -> Result<ChangePlan> {
        if !crate::scanner::finding_ready(finding_id) {
            return Err(Error::NotImplemented("kernel::plan_finding"));
        }
        match finding_id.split_once(':') {
            Some(("duplicate", asset)) => self.plan_adopt(asset),
            _ => Err(Error::NotImplemented("kernel::plan_finding")),
        }
    }

    /// 项目页「历史」页签（6.3、M1-14）：这个项目的快照，从新到旧，标出当前磁盘状态。
    /// 跨项目的记录也算（它给每个项目各留一个快照）。快照的范围按真实路径比。
    pub fn history(&self, project_root: &Path) -> Result<Vec<crate::journal::HistoryEntry>> {
        let records = self.journal.read(&RecordFilter::default())?;
        let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let root = real(project_root);
        Ok(crate::journal::history_of(&records, |p| {
            real(Path::new(&p.0)) == root
        }))
    }

    /// `fangcun sync --frozen`：锁与清单不一致就失败。本身不改项目文件。
    ///
    /// 不联网。结果在本机记一条「校验」（6.7）：一致是「已记录」，不一致是「失败」。
    pub fn verify_frozen(&self, project_root: &Path) -> Result<()> {
        let (project, lock) = self.manifest_and_lock(project_root)?;
        let mismatches = crate::resolver::lock_mismatches(&project, &lock);
        let name = crate::scanner::project_name(project_root);
        let ok = mismatches.is_empty();
        let at = Timestamp::now();
        let record = Record {
            id: format!("verify-{}-{}", at.0, self.journal.count()),
            at,
            kind: RecordKind::Verify,
            title: if ok {
                "校验通过：锁与清单一致".to_owned()
            } else {
                "校验失败：锁与清单不一致".to_owned()
            },
            project: Some(ProjectId(project_root.display().to_string())),
            project_label: name,
            trigger: self.origin,
            snapshot: None,
            snapshots: Vec::new(),
            reverts: None,
            status: if ok {
                RecordStatus::Recorded
            } else {
                RecordStatus::Failed
            },
            summary: if ok {
                "没有写入文件。".to_owned()
            } else {
                format!("没有写入文件。{}", mismatches.join("；"))
            },
            files: vec![
                RecordFile {
                    op: FileOp::Read,
                    path: crate::paths::MANIFEST_FILE.to_owned(),
                    note: None,
                },
                RecordFile {
                    op: FileOp::Read,
                    path: crate::paths::LOCK_FILE.to_owned(),
                    note: None,
                },
            ],
            command: Some("fangcun sync --frozen".to_owned()),
            output: Some(if ok {
                "一致".to_owned()
            } else {
                format!("{} 处不一致", mismatches.len())
            }),
        };
        let _ = self.journal.record(record);
        if ok {
            Ok(())
        } else {
            Err(Error::Frozen(mismatches.join("；")))
        }
    }

    /// 已登记项目的清单与锁，缺了就说下一步该运行什么。
    fn manifest_and_lock(&self, project_root: &Path) -> Result<(crate::manifest::Project, Lock)> {
        self.require_registered(project_root)?;
        let project =
            crate::manifest::load_project(project_root, &self.home)?.ok_or_else(|| {
                Error::NotFound(format!(
                    "{} 没有 fangcun.toml。先运行 fangcun init",
                    display_path(project_root, &self.user_home)
                ))
            })?;
        let lock = Lock::load(project_root)?.ok_or_else(|| {
            Error::NotFound(format!(
                "{} 没有 fangcun.lock。先运行 fangcun lock",
                display_path(project_root, &self.user_home)
            ))
        })?;
        Ok((project, lock))
    }

    // ───────────── 写入用例（M1）：先出计划，再 apply ─────────────

    /// 登记时建议的物化方式（9.2）：个人项目默认链接，git 历史里有两位及以上提交者时建议入库。
    pub fn suggest_mode(&self, project_root: &Path) -> crate::resolver::ModeSuggestion {
        crate::resolver::suggest_mode(project_root)
    }

    /// `fangcun init`、检查器「登记」、侧栏「添加项目…」（F3）：登记项目，生成清单与锁。
    ///
    /// - 已有 `fangcun.toml`：按现有清单登记，缺锁时补锁，不再反推。
    /// - `from_existing`：从目录现状反推清单（规则见 `resolver::infer`），GitHub 来源会联网找提交。
    /// - 否则只写 `[project]`。
    ///
    /// `mode` 是登记时问的物化方式（9.2），为空时取 [`Kernel::suggest_mode`] 的建议，写进新清单。
    /// 计划写三个文件：清单、锁、登记表。查不出来源的项不进清单，写在计划的提示里。
    pub fn plan_init(
        &self,
        project_root: &Path,
        from_existing: bool,
        mode: Option<crate::model::SyncMode>,
    ) -> Result<ChangePlan> {
        self.require_write("init")?;
        let root = project_root;
        if !root.is_dir() {
            return Err(Error::NotFound(format!(
                "目录 {}",
                display_path(root, &self.user_home)
            )));
        }
        let name = crate::scanner::project_name(root);
        let registered = crate::registry::load(&self.home)?;
        let already = crate::registry::contains(&registered, root);
        let manifest_file = root.join(crate::paths::MANIFEST_FILE);
        let lock_file = root.join(crate::paths::LOCK_FILE);
        let has_manifest = manifest_file.is_file();
        let command = if from_existing {
            "fangcun init --from-existing"
        } else {
            "fangcun init"
        };
        let scope = SnapshotScope::Project(root.to_path_buf());
        if already && has_manifest && lock_file.is_file() {
            let builder = PlanBuilder::new(format!("{name} 已经登记过"), RecordKind::Sync, scope)
                .command(command);
            return Ok(self.issue(builder));
        }

        let library = Library::open(&self.home)?;
        let sources = self.sources();
        let mut notes = Vec::new();
        let (project, seed, new_manifest) = if has_manifest {
            notes.push(crate::model::PlanNote::info(
                "已有 fangcun.toml，按现有清单登记，不再反推",
            ));
            let project = crate::manifest::load_project(root, &self.home)?
                .ok_or_else(|| Error::NotFound(crate::paths::MANIFEST_FILE.to_owned()))?;
            (project, Lock::load(root)?, None)
        } else {
            let scan = self.scan();
            let mut collected = BTreeMap::new();
            for tool in &scan.tools {
                collected.insert(tool.clone(), self.project_collected(&scan, root, tool)?);
            }
            let used = scan
                .project(root)
                .map(|p| p.tools.clone())
                .unwrap_or_default();
            let input = crate::resolver::InferInput {
                root,
                user_home: &self.user_home,
                fangcun_root: self.home.root(),
                adapters: &self.adapters,
                collected: &collected,
                used: &used,
            };
            let (mut manifest, seed) = if from_existing {
                let inferred = crate::resolver::infer_project(&input, &sources, &library)?;
                notes.extend(inferred.notes);
                (inferred.manifest, Some(inferred.seed))
            } else {
                let manifest = crate::manifest::Manifest {
                    project: crate::manifest::ProjectSection {
                        targets: crate::resolver::infer_targets(&input),
                        ..crate::manifest::ProjectSection::default()
                    },
                    ..crate::manifest::Manifest::default()
                };
                (manifest, None)
            };
            manifest.project.mode =
                Some(mode.unwrap_or_else(|| crate::resolver::suggest_mode(root).mode));
            let project = crate::manifest::Project {
                root: root.to_path_buf(),
                manifest: manifest.clone(),
                local: None,
                bundles: Vec::new(),
            };
            (project, seed, Some(manifest))
        };
        let lock = crate::resolver::resolve_lock(&project, seed.as_ref(), &sources, &library)?;

        let title = if already {
            format!("为 {name} 补上清单与锁")
        } else {
            format!("登记 {name}")
        };
        let mut builder = PlanBuilder::new(title, RecordKind::Sync, scope).command(command);
        builder.registers(root);
        if let Some(manifest) = &new_manifest {
            builder.write_file(
                &manifest_file,
                crate::manifest::render_manifest(manifest).into_bytes(),
                manifest_summary(manifest),
            )?;
        }
        let lock_text = lock.to_toml()?;
        if std::fs::read(&lock_file).ok().as_deref() != Some(lock_text.as_bytes()) {
            let previous = if new_manifest.is_some() {
                None
            } else {
                Lock::load(root)?
            };
            let detail = match &previous {
                Some(previous) => lock_summary(Some(previous), &lock),
                None => lock_counts(&lock),
            };
            builder.write_file(&lock_file, lock_text.into_bytes(), detail)?;
        }
        if !already {
            let text = crate::registry::with_registered(
                &self.home,
                &[root.to_path_buf()],
                &Timestamp::now(),
            )?;
            builder.write_file(
                &self.home.projects_file(),
                text.into_bytes(),
                format!("登记 {}", display_path(root, &self.user_home)),
            )?;
        }
        for note in notes {
            builder.note(note);
        }
        Ok(self.issue(builder))
    }

    /// `fangcun add <资产>`：加入清单。
    pub fn plan_add(&self, _project_root: &Path, _asset: &str) -> Result<ChangePlan> {
        self.require_write("add")?;
        Err(Error::NotImplemented("Kernel::plan_add"))
    }

    /// `fangcun adopt <资产>`：收编进资产库。
    ///
    /// 扫描到的、叫这个名字的副本（全局层和各项目）存进资产库，工具目录里的实体目录换成
    /// 指向正本的软链。规则见 `materializer::adopt`。
    pub fn plan_adopt(&self, asset: &str) -> Result<ChangePlan> {
        self.require_write("adopt")?;
        let scan = self.scan();
        let mut copies: Vec<&crate::scanner::SkillCopy> = Vec::new();
        for collected in scan.global.values() {
            copies.extend(collected.skills.iter().filter(|s| s.name == asset));
        }
        for per_tool in scan.project_items.values() {
            for collected in per_tool.values() {
                copies.extend(collected.skills.iter().filter(|s| s.name == asset));
            }
        }
        let library = Library::open(&self.home)?;
        let scope = self.write_scope()?;
        let mut builder = PlanBuilder::new(String::new(), RecordKind::Adopt, SnapshotScope::Global)
            .command(format!("fangcun adopt {asset}"));
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
        Ok(self.issue(builder))
    }

    /// `fangcun lock`：重新解析清单，更新锁。
    ///
    /// 约束没变的项沿用锁里的版本，新增的、换了约束的重新解析（会联网拉 git 来源，
    /// 拉到的版本存进资产库）。锁的内容没变时，计划里没有变更。
    pub fn plan_lock(&self, project_root: &Path) -> Result<ChangePlan> {
        self.require_write("lock")?;
        self.require_registered(project_root)?;
        let project =
            crate::manifest::load_project(project_root, &self.home)?.ok_or_else(|| {
                Error::NotFound(format!(
                    "{} 没有 fangcun.toml。先运行 fangcun init",
                    display_path(project_root, &self.user_home)
                ))
            })?;
        let previous = Lock::load(project_root)?;
        let library = Library::open(&self.home)?;
        let lock =
            crate::resolver::resolve_lock(&project, previous.as_ref(), &self.sources(), &library)?;
        let text = lock.to_toml()?;
        let file = project_root.join(crate::paths::LOCK_FILE);
        let name = project_root.file_name().map_or_else(
            || project_root.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let current = std::fs::read(&file).ok();
        let changed = current.as_deref() != Some(text.as_bytes());
        let title = if changed {
            format!("更新 {name} 的锁")
        } else {
            format!("{name} 的锁已是最新")
        };
        let mut builder = PlanBuilder::new(
            title,
            RecordKind::Sync,
            SnapshotScope::Project(project_root.to_path_buf()),
        )
        .command("fangcun lock");
        if changed {
            builder.write_file(
                &file,
                text.into_bytes(),
                lock_summary(previous.as_ref(), &lock),
            )?;
        }
        Ok(self.issue(builder))
    }

    /// `fangcun export skills-lock`（M1-08）：把锁里 GitHub 来源的 skill 导出成项目根目录的
    /// `skills-lock.json`，和 `skills` 命令行互通。原文件里方寸不管的项原样留下。
    pub fn plan_export_skills_lock(&self, project_root: &Path) -> Result<ChangePlan> {
        self.require_write("export")?;
        self.require_registered(project_root)?;
        let lock = Lock::load(project_root)?.ok_or_else(|| {
            Error::NotFound(format!(
                "{} 没有 fangcun.lock。先运行 fangcun lock",
                display_path(project_root, &self.user_home)
            ))
        })?;
        // 位置从适配器查：项目层 skill 目录配套的 skills 命令行锁。
        let (file, installed) = self
            .adapters
            .iter()
            .flat_map(|a| a.skills.iter())
            .filter(|l| l.scope == crate::model::Scope::Project)
            .find_map(|l| {
                let lock = l.lock.as_ref()?;
                Some((
                    crate::adapter::expand(&lock.path, &self.user_home, Some(project_root))?,
                    crate::adapter::expand(&l.path, &self.user_home, Some(project_root))?,
                ))
            })
            .ok_or_else(|| {
                Error::NotFound("适配器里没有项目层的 skills-lock.json 位置".to_owned())
            })?;
        let exported = crate::manifest::export_skills_lock(&lock, &file, &installed)?;
        let name = crate::scanner::project_name(project_root);
        let changed = std::fs::read(&file).ok().as_deref() != Some(exported.text.as_bytes());
        let title = if changed {
            format!("导出 {name} 的 skills-lock.json")
        } else {
            format!("{name} 的 skills-lock.json 已是最新")
        };
        let mut builder = PlanBuilder::new(
            title,
            RecordKind::Sync,
            SnapshotScope::Project(project_root.to_path_buf()),
        )
        .command("fangcun export skills-lock");
        if changed {
            let mut detail = format!("{} 个 skill", exported.exported.len());
            if !exported.kept.is_empty() {
                detail.push_str(&format!("，留下原有的 {} 项", exported.kept.len()));
            }
            builder.write_file(&file, exported.text.into_bytes(), detail)?;
        }
        if !exported.skipped.is_empty() {
            builder.note(crate::model::PlanNote::info(format!(
                "{} 项不是 GitHub 来源，skills-lock.json 记不了，没有导出：{}",
                exported.skipped.len(),
                exported.skipped.join("、")
            )));
        }
        Ok(self.issue(builder))
    }

    /// `fangcun sync`：按清单与锁物化。
    ///
    /// 锁和清单不一致时不出计划，要先 `fangcun lock`。资产库里没有的正本会按锁联网取回。
    pub fn plan_sync(&self, project_root: &Path) -> Result<ChangePlan> {
        self.require_write("sync")?;
        let (project, lock) = self.manifest_and_lock(project_root)?;
        let library = Library::open(&self.home)?;
        let sources = self.sources();
        let mut builder = PlanBuilder::new(
            String::new(),
            RecordKind::Sync,
            SnapshotScope::Project(project_root.to_path_buf()),
        )
        .command("fangcun sync");
        crate::materializer::sync::plan(
            &crate::materializer::sync::SyncInput {
                root: project_root,
                home: &self.home,
                user_home: &self.user_home,
                adapters: &self.adapters,
                project: &project,
                lock: &lock,
                library: &library,
                sources: &sources,
            },
            &mut builder,
        )?;
        Ok(self.issue(builder))
    }

    /// `fangcun update [资产]`。`asset` 为空表示全部；`projects` 为空表示当前项目。
    pub fn plan_update(&self, _asset: Option<&str>, _projects: &[ProjectId]) -> Result<ChangePlan> {
        self.require_write("update")?;
        Err(Error::NotImplemented("Kernel::plan_update"))
    }

    /// `fangcun rollback <快照>`：把项目回滚到快照 #N，也就是撤销留下快照 #N+1 的那条记录
    /// （6.7：「回滚到此之前…」的预览标题就是「回滚到快照 #N」）。
    pub fn plan_rollback(&self, project_root: &Path, snapshot: SnapshotId) -> Result<ChangePlan> {
        self.require_write("rollback")?;
        let wanted = crate::scanner::fsutil::canonical(project_root);
        let undo = SnapshotId(snapshot.0 + 1);
        let record = self
            .journal
            .read(&RecordFilter::default())?
            .into_iter()
            .find(|r| {
                crate::materializer::rollback::scopes_of(r)
                    .iter()
                    .any(|(scope, id)| {
                        *id == undo
                            && matches!(scope, SnapshotScope::Project(p)
                                if crate::scanner::fsutil::canonical(p) == wanted)
                    })
            })
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "快照 #{} 之后的那条记录（{} 现在就在快照 #{} 或更早）",
                    snapshot.0,
                    crate::scanner::project_name(project_root),
                    snapshot.0
                ))
            })?;
        self.plan_rollback_record(&record.id)
    }

    /// 活动页、历史页签的「回滚到此之前…」：撤销一条记录（6.7）。
    pub fn plan_rollback_record(&self, record_id: &str) -> Result<ChangePlan> {
        use crate::materializer::rollback;
        use crate::materializer::snapshot;

        self.require_write("rollback")?;
        let record = self
            .journal
            .find(record_id)?
            .ok_or_else(|| Error::NotFound(format!("记录 {record_id}")))?;
        let snapshots = rollback::check(&self.home, &record)?;
        let before = |id: u32| match (id.saturating_sub(1), record.kind) {
            (0, RecordKind::Adopt) => "收编前".to_owned(),
            (0, _) => "登记前".to_owned(),
            (n, _) => format!("快照 #{n}"),
        };
        let label = |scope: &SnapshotScope| match scope {
            SnapshotScope::Project(root) => crate::scanner::project_name(root),
            SnapshotScope::Global => "全局层".to_owned(),
        };
        let title = match snapshots.as_slice() {
            [(scope, snap)] => format!("把 {} 回滚到{}", label(scope), before(snap.id)),
            many => format!(
                "把 {} 个范围回滚到{}",
                many.len(),
                many.iter()
                    .map(|(_, snap)| before(snap.id))
                    .collect::<Vec<_>>()
                    .join(" · ")
            ),
        };
        let (primary, first) = &snapshots[0];
        let mut builder = PlanBuilder::new(title, RecordKind::Rollback, primary.clone());
        if let SnapshotScope::Project(_) = primary {
            builder = builder.command(format!("fangcun rollback {}", first.id.saturating_sub(1)));
        }
        // 后来哪个快照也动过这个路径：报给人先回滚那一条。
        let records = self.journal.read(&RecordFilter::default())?;
        let later = |scope: &SnapshotScope, path: &Path, after: u32| -> Option<(String, u32)> {
            let ids = snapshot::list(&self.home, scope);
            ids.iter().rev().filter(|id| id.0 > after).find_map(|id| {
                let snap = snapshot::load(&self.home, scope, *id).ok()?;
                snap.entries.iter().any(|e| e.path == path).then(|| {
                    let title = records
                        .iter()
                        .find(|r| {
                            rollback::scopes_of(r)
                                .iter()
                                .any(|(s, i)| i == id && s.normalized() == scope.normalized())
                        })
                        .map_or_else(|| snap.title.clone(), |r| r.title.clone());
                    (title, id.0)
                })
            })
        };
        rollback::plan(&self.home, &record, &snapshots, &later, &mut builder)?;
        Ok(self.issue(builder))
    }

    /// 应用一份计划。写入只发生在这里。
    ///
    /// 只认这个内核出过、还没应用的计划，按编号找回动作；传进来的标题和每一行不参与执行。
    /// 应用失败时计划作废，要重新预览。
    pub fn apply(&self, plan: &ChangePlan) -> Result<Applied> {
        self.require_write("apply")?;
        let issued = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&plan.id))
            .ok_or(Error::PlanNotFound(plan.id))?;
        let scope = self.write_scope()?;
        let applied = crate::materializer::apply(
            &issued,
            &ApplyContext {
                home: &self.home,
                scope: &scope,
                journal: &self.journal,
                trigger: self.origin,
            },
        )?;
        // 扫描缓存不在这里清：清了下一次查询会全盘重扫，桌面端还会多记一条体检。
        // 应用后要不要重扫由用例决定（M1-13 的同步面板）。
        // 快照保留期（6.7）：清理失败不影响这次应用。
        if let Ok(settings) = crate::settings::Settings::load(&self.home) {
            let keep = usize::try_from(settings.snapshot_keep).unwrap_or(usize::MAX);
            let _ = crate::materializer::snapshot::prune(&self.home, settings.snapshot_days, keep);
        }
        Ok(applied)
    }
}

/// 新清单的说明，例如「清单：2 个目标工具、21 个 skill、1 个 MCP、正本 AGENTS.md」。
fn manifest_summary(manifest: &crate::manifest::Manifest) -> String {
    let mut parts = vec![format!("{} 个目标工具", manifest.project.targets.len())];
    if !manifest.skills.is_empty() {
        parts.push(format!("{} 个 skill", manifest.skills.len()));
    }
    if !manifest.mcp.is_empty() {
        parts.push(format!("{} 个 MCP", manifest.mcp.len()));
    }
    if let Some(instructions) = &manifest.instructions {
        parts.push(format!("正本 {}", instructions.canonical));
    }
    format!("清单：{}", parts.join("、"))
}

/// 新锁的说明，例如「锁：21 个 skill、1 个 MCP、1 份指令」。
fn lock_counts(lock: &Lock) -> String {
    let mut parts = Vec::new();
    for (count, label) in [
        (lock.skills.len(), "个 skill"),
        (lock.mcp.len(), "个 MCP"),
        (lock.instructions.len(), "份指令"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    if parts.is_empty() {
        "锁：还没有资产".to_owned()
    } else {
        format!("锁：{}", parts.join("、"))
    }
}

/// 锁的变更说明，例如「新增 tdd；更新 skill-b；去掉 old」。
fn lock_summary(previous: Option<&Lock>, next: &Lock) -> String {
    fn names<T: PartialEq>(
        old: Option<&std::collections::BTreeMap<String, T>>,
        new: &std::collections::BTreeMap<String, T>,
        added: &mut Vec<String>,
        updated: &mut Vec<String>,
        removed: &mut Vec<String>,
    ) {
        for (name, item) in new {
            match old.and_then(|o| o.get(name)) {
                None => added.push(name.clone()),
                Some(before) if before != item => updated.push(name.clone()),
                Some(_) => {}
            }
        }
        if let Some(old) = old {
            removed.extend(old.keys().filter(|k| !new.contains_key(*k)).cloned());
        }
    }
    let (mut added, mut updated, mut removed) = (Vec::new(), Vec::new(), Vec::new());
    names(
        previous.map(|p| &p.skills),
        &next.skills,
        &mut added,
        &mut updated,
        &mut removed,
    );
    names(
        previous.map(|p| &p.mcp),
        &next.mcp,
        &mut added,
        &mut updated,
        &mut removed,
    );
    names(
        previous.map(|p| &p.instructions),
        &next.instructions,
        &mut added,
        &mut updated,
        &mut removed,
    );
    let mut parts = Vec::new();
    for (label, list) in [("新增", added), ("更新", updated), ("去掉", removed)] {
        if !list.is_empty() {
            parts.push(format!("{label} {}", list.join("、")));
        }
    }
    if parts.is_empty() {
        "重写格式".to_owned()
    } else {
        parts.join("；")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kernel(access: Access) -> Kernel {
        kernel_at(FangcunHome::at("/nonexistent/fangcun-test"), access)
    }

    fn kernel_at(home: FangcunHome, access: Access) -> Kernel {
        Kernel::with_home(home, access)
            .expect("打开内核不应碰磁盘")
            .with_user_home("/nonexistent/fangcun-test-home")
    }

    /// 每个测试一个空的临时 `~/.fangcun`。
    fn temp_home(name: &str) -> FangcunHome {
        let root = std::env::temp_dir().join(format!("fangcun-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        FangcunHome::at(root)
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
            id: 0,
            title: String::new(),
            changes: Vec::new(),
            notes: Vec::new(),
            blocked: false,
            body: None,
        };

        let refusals = [
            kernel.plan_init(root, true, None).err(),
            kernel.plan_add(root, "tdd").err(),
            kernel.plan_adopt("tdd").err(),
            kernel.plan_lock(root).err(),
            kernel.plan_export_skills_lock(root).err(),
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
    fn concurrent_queries_scan_once() {
        let home = temp_home("concurrent");
        let kernel = kernel_at(home.clone(), Access::ReadOnly);
        std::thread::scope(|scope| {
            for _ in 0..6 {
                scope.spawn(|| kernel.overview().expect("概览"));
            }
        });
        let records = kernel.log(&RecordFilter::default()).expect("读记录");
        assert_eq!(records.len(), 1, "并发查询只应扫一次：{records:?}");
        kernel.rescan().expect("重扫");
        let ids: std::collections::BTreeSet<String> = kernel
            .log(&RecordFilter::default())
            .expect("读记录")
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids.len(), 2, "每条体检记录的编号都不同");
        let _ = std::fs::remove_dir_all(home.root());
    }

    #[test]
    fn checkups_are_kept_in_the_journal_across_runs() {
        let home = temp_home("journal");
        let first = kernel_at(home.clone(), Access::ReadOnly);
        first.overview().expect("概览");
        let records = first.log(&RecordFilter::default()).expect("读记录");
        assert_eq!(records[0].trigger, Trigger::FirstLaunch);
        assert!(records[0].title.starts_with("首次体检"));
        assert!(
            home.journal_dir()
                .join(crate::journal::RECORDS_FILE)
                .is_file()
        );

        // 再开一次：读得到上次的记录，这次不算首次。
        let second = kernel_at(home.clone(), Access::ReadOnly);
        second.rescan().expect("重扫");
        let records = second.log(&RecordFilter::default()).expect("读记录");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].trigger, Trigger::Desktop);
        assert!(records[0].title.starts_with("体检"));
        // 只写了 journal 一个子目录。
        let entries: Vec<_> = std::fs::read_dir(home.root()).expect("列目录").collect();
        assert_eq!(entries.len(), 1);
        let _ = std::fs::remove_dir_all(home.root());
    }

    #[test]
    fn cli_records_only_explicit_checkups() {
        let home = temp_home("cli");
        let cli = kernel_at(home.clone(), Access::ReadOnly).with_origin(Trigger::Cli);
        cli.overview().expect("概览");
        assert!(
            cli.log(&RecordFilter::default())
                .expect("读记录")
                .is_empty()
        );
        assert!(!home.root().exists(), "顺带的扫描不写记录");
        cli.doctor().expect("体检");
        let records = cli.log(&RecordFilter::default()).expect("读记录");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].trigger, Trigger::Cli);
        let _ = std::fs::remove_dir_all(home.root());
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
