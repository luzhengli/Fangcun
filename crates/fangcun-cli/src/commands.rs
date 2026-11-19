//! 每个命令对应内核的一个用例。这里只做参数转换和打印。
//!
//! 输出是给人看的中文纯文本，路径、哈希原样打印。估算值一律标「估算」。

use std::path::{Path, PathBuf};

use fangcun_core::journal::{FileOp, Record, RecordFilter, RecordStatus, Trigger};
use fangcun_core::materializer::{ChangeOp, ChangePlan};
use fangcun_core::model::{
    AssetKind, NoteTone, ProjectId, Scope, SnapshotId, SyncMode, TokenEstimate,
};
use fangcun_core::resolver::{EffectiveContext, Explanation, scope_name};
use fangcun_core::scanner::{CheckupReport, FindingCategory, FindingStatus, format_k};
use fangcun_core::{Error, Kernel, Result};

use crate::cli::{Cli, Command, ExportFormat};

pub fn run(cli: Cli) -> Result<()> {
    let cwd = current_project()?;
    // 默认可写；`--read-only` 或设置里的只读模式打开时只读（7.5）。
    let kernel = Kernel::open_configured(cli.read_only)?
        .with_origin(Trigger::Cli)
        .with_project(&cwd);

    match cli.command {
        Command::Doctor => print_doctor(&kernel.doctor()?),
        Command::Init { from_existing } => {
            let root = project_root_of(&cwd);
            let mode = ask_mode(&kernel, &root, cli.yes);
            let plan = kernel.plan_init(&root, from_existing, mode)?;
            confirm_and_apply(&kernel, &plan, cli.yes)?;
        }
        Command::Status => print_status(&kernel, &cwd)?,
        Command::Add { asset } => {
            let _plan = kernel.plan_add(&cwd, &asset)?;
        }
        Command::Adopt { asset } => {
            let _plan = kernel.plan_adopt(&asset)?;
        }
        Command::Lock => {
            let plan = kernel.plan_lock(&project_root_of(&cwd))?;
            confirm_and_apply(&kernel, &plan, cli.yes)?;
        }
        Command::Export {
            format: ExportFormat::SkillsLock,
        } => {
            let plan = kernel.plan_export_skills_lock(&project_root_of(&cwd))?;
            confirm_and_apply(&kernel, &plan, cli.yes)?;
        }
        Command::Sync { frozen: true } => {
            kernel.verify_frozen(&cwd)?;
        }
        Command::Sync { frozen: false } => {
            let _plan = kernel.plan_sync(&cwd)?;
        }
        Command::Update {
            asset,
            dry_run: _,
            projects,
        } => {
            let projects: Vec<ProjectId> = projects.into_iter().map(ProjectId).collect();
            let _plan = kernel.plan_update(asset.as_deref(), &projects)?;
        }
        Command::Rollback { snapshot } => {
            let _plan = kernel.plan_rollback(&cwd, SnapshotId(snapshot))?;
        }
        Command::Log => print_log(&kernel.log(&RecordFilter::default())?),
        Command::Why { asset, global } => {
            let root = project_root_of(&cwd);
            let project = if global { None } else { Some(root.as_path()) };
            print_why(&kernel.why(project, &asset)?);
        }
    }

    Ok(())
}

/// 登记时问一次物化方式（9.2），回车取建议的那个。已有清单、`--yes`、不在终端里时不问，
/// 由内核取建议。
fn ask_mode(kernel: &Kernel, root: &Path, yes: bool) -> Option<SyncMode> {
    use std::io::{BufRead, IsTerminal, Write};

    if yes || !std::io::stdin().is_terminal() || root.join("fangcun.toml").is_file() {
        return None;
    }
    let suggestion = kernel.suggest_mode(root);
    let (hint, default) = match suggestion.mode {
        SyncMode::Vendor => (
            format!("{} 位提交者，多人协作建议入库", suggestion.committers),
            "v",
        ),
        SyncMode::Link => ("个人项目建议链接".to_owned(), "l"),
    };
    print!("物化方式：链接（l，软链到资产库）或入库（v，实体文件提交进仓库）。{hint} [{default}] ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    let _ = std::io::stdin().lock().read_line(&mut answer);
    match answer.trim() {
        "l" | "L" | "link" => Some(SyncMode::Link),
        "v" | "V" | "vendor" => Some(SyncMode::Vendor),
        _ => Some(suggestion.mode),
    }
}

/// 先打印变更计划，再问「应用？[y/N]」（第 5 节）。`--yes` 不问；不在终端里又没加
/// `--yes` 时只打印不写，免得被脚本或 Agent 调用时卡在提问上。
fn confirm_and_apply(kernel: &Kernel, plan: &ChangePlan, yes: bool) -> Result<()> {
    use std::io::{BufRead, IsTerminal, Write};

    println!("{}", plan.title);
    for note in &plan.notes {
        let mark = match note.tone {
            NoteTone::Info => "·",
            NoteTone::Warning => "!",
            NoteTone::Danger => "‼",
        };
        println!("  {mark} {}", note.text);
    }
    if plan.changes.is_empty() {
        println!("没有要写的变更。");
        return Ok(());
    }
    for change in &plan.changes {
        let mark = match change.op {
            ChangeOp::Add => "+",
            ChangeOp::Modify => "~",
            ChangeOp::Remove => "−",
        };
        println!("  {mark} {}  {}", change.path.display(), change.detail);
    }
    if !yes {
        if !std::io::stdin().is_terminal() {
            println!("没有应用：不在终端里运行。确认后加 --yes 再运行一次。");
            return Ok(());
        }
        print!("应用？[y/N] ");
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().lock().read_line(&mut answer);
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("没有应用。");
            return Ok(());
        }
    }
    let applied = kernel.apply(plan)?;
    let snapshots: Vec<String> = applied
        .snapshots
        .iter()
        .map(|s| format!("#{}", s.0))
        .collect();
    println!("已应用，已留快照 {}。", snapshots.join("、"));
    Ok(())
}

/// 「当前项目」就是当前目录。
fn current_project() -> Result<PathBuf> {
    std::env::current_dir().map_err(|source| Error::Io {
        path: PathBuf::from("."),
        source,
    })
}

/// 往上找 git 根目录，找不到就用当前目录。和内核发现项目的规则一致。
fn project_root_of(cwd: &Path) -> PathBuf {
    cwd.ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(cwd)
        .to_path_buf()
}

fn category_name(category: FindingCategory) -> &'static str {
    match category {
        FindingCategory::Duplicate => "重复",
        FindingCategory::GlobalTax => "全局税",
        FindingCategory::Inconsistent => "不一致",
        FindingCategory::Detached => "脱节",
        FindingCategory::Secret => "密钥",
    }
}

fn print_doctor(report: &CheckupReport) {
    println!("体检报告 · 只读扫描完成，未改动任何文件");
    println!(
        "扫描了 {} 个工具、{} 个项目目录，发现 {} 项资产。重复副本 {}，来源未知 {}。",
        report.tools_scanned,
        report.project_dirs_scanned,
        report.assets_found,
        report.duplicate_copies,
        report.unknown_source
    );
    println!(
        "「用到」依据本机近 30 天的 {} 份会话记录，不上传。",
        report.sessions_read
    );
    println!();
    if report.findings.is_empty() {
        println!("没有发现。");
        return;
    }
    for finding in &report.findings {
        let state = match finding.status {
            FindingStatus::Pending => "待处理",
            FindingStatus::Resolved => "已处理",
            FindingStatus::NotScanned => "尚未扫描",
        };
        println!(
            "● {}  {}  [{state}]",
            category_name(finding.category),
            finding.title
        );
        println!("  {}", finding.intro);
        for location in &finding.locations {
            println!(
                "    {}  {}  {}",
                location.path, location.detail, location.note
            );
        }
        println!("  → {}（M1 开放）", finding.action);
        println!();
    }
}

fn print_status(kernel: &Kernel, cwd: &Path) -> Result<()> {
    let root = project_root_of(cwd);
    let status = kernel.status(&root)?;
    println!(
        "{}  {}",
        fangcun_core::scanner::project_name(&root),
        root.display()
    );
    if status.has_manifest {
        println!("有 fangcun.toml。同步状态与漂移在 M1 读取。");
    } else {
        println!("未登记：没有 fangcun.toml。M0 只读，登记项目在 M1 开放。");
    }
    println!();
    for tool in kernel.tools() {
        let Ok(context) = kernel.effective_context(&root, &tool.id) else {
            continue;
        };
        print_context_summary(&tool.name, &context);
    }
    Ok(())
}

fn print_context_summary(tool_name: &str, context: &EffectiveContext) {
    let budget = &context.budget;
    let mcp_note = if budget.mcp_unestimated > 0 {
        format!("，MCP {} 个未估算", budget.mcp_unestimated)
    } else {
        String::new()
    };
    println!(
        "{tool_name}  常驻 {} / 项目预算 {} · 估算值（指令 {}，Skill {}{mcp_note}）",
        format_k(budget.total()),
        format_k(budget.limit),
        format_k(budget.instructions),
        format_k(budget.skills),
    );
    for kind in [AssetKind::Instruction, AssetKind::Skill, AssetKind::Mcp] {
        let items: Vec<_> = context.items.iter().filter(|i| i.kind == kind).collect();
        if items.is_empty() {
            continue;
        }
        let label = match kind {
            AssetKind::Instruction => "指令",
            AssetKind::Skill => "Skills",
            AssetKind::Mcp => "MCP",
        };
        println!("  {label}");
        for item in items {
            let resident = item
                .resident
                .map_or_else(|| "—".to_owned(), |r: TokenEstimate| format_k(r));
            println!(
                "    {:<36} {:<4} {:<20} {resident}",
                item.name,
                scope_name(item.scope),
                item.label
            );
        }
    }
    if !context.observed.is_empty() {
        println!("  其他加载项（只看不管）");
        for item in &context.observed {
            println!(
                "    {:<36} {:<4} {}",
                item.name,
                scope_name(item.scope),
                item.note.as_deref().unwrap_or("")
            );
        }
    }
    println!();
}

fn print_why(explanations: &[Explanation]) {
    for e in explanations {
        println!("{} · {}  {}", e.tool_name, e.name, e.label);
        println!("  {}", e.reason);
        println!("  在哪一层声明：");
        for d in &e.declared {
            let mark = if d.effective {
                "生效"
            } else if d.location.is_some() {
                "被遮蔽"
            } else {
                "—"
            };
            println!(
                "    {}  {:<6} {}",
                scope_name(d.scope),
                mark,
                d.location.as_deref().unwrap_or("")
            );
        }
        if e.effective_scope == Scope::Global {
            println!("  生效层：全局");
        }
        println!();
    }
}

fn print_log(records: &[Record]) {
    if records.is_empty() {
        println!("没有活动记录。");
    }
    for r in records {
        let status = match r.status {
            RecordStatus::Applied => "已应用",
            RecordStatus::Pending => "待处理",
            RecordStatus::Failed => "失败",
            RecordStatus::RolledBack => "已回滚",
            RecordStatus::Recorded => "已记录",
            RecordStatus::ReadOnly => "只读",
        };
        let snapshot = r
            .snapshot
            .map_or_else(|| "—".to_owned(), |s| format!("#{}", s.0));
        println!("{}  {}  {snapshot}  {status}", r.at.0, r.title);
        println!("    {} · {}", r.project_label, trigger_name(r.trigger));
        for file in &r.files {
            let op = match file.op {
                FileOp::Add => "+",
                FileOp::Modify => "~",
                FileOp::Remove => "−",
                FileOp::Read => "=",
            };
            println!("      {op} {}", file.path);
        }
    }
    println!();
    println!(
        "记录保存在 ~/.fangcun/journal。M0 只记体检：桌面应用启动、重新体检、fangcun doctor。"
    );
}

fn trigger_name(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Desktop => "桌面应用",
        Trigger::Cli => "命令行",
        Trigger::Watcher => "文件监听",
        Trigger::UpstreamCheck => "上游检查",
        Trigger::FirstLaunch => "首次启动",
    }
}
