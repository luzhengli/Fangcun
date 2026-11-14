//! 每个命令对应内核的一个用例。这里只做参数转换和打印。
//!
//! 输出是给人看的中文纯文本，路径、哈希原样打印。估算值一律标「估算」。

use std::path::{Path, PathBuf};

use fangcun_core::journal::{FileOp, Record, RecordFilter, RecordStatus, Trigger};
use fangcun_core::model::{AssetKind, ProjectId, Scope, SnapshotId, TokenEstimate};
use fangcun_core::resolver::{EffectiveContext, Explanation, scope_name};
use fangcun_core::scanner::{CheckupReport, FindingCategory, FindingStatus, format_k};
use fangcun_core::{Access, Error, Kernel, Result};

use crate::cli::{Cli, Command};

/// M0 只读。M1 起，会写入的命令改用 `Access::ReadWrite` 打开内核。
const ACCESS: Access = Access::ReadOnly;

pub fn run(cli: Cli) -> Result<()> {
    let cwd = current_project()?;
    let kernel = Kernel::open(ACCESS)?
        .with_origin(Trigger::Cli)
        .with_project(&cwd);

    match cli.command {
        Command::Doctor => print_doctor(&kernel.doctor()?),
        Command::Init { from_existing } => {
            let _plan = kernel.plan_init(&cwd, from_existing)?;
        }
        Command::Status => print_status(&kernel, &cwd)?,
        Command::Add { asset } => {
            let _plan = kernel.plan_add(&cwd, &asset)?;
        }
        Command::Adopt { asset } => {
            let _plan = kernel.plan_adopt(&asset)?;
        }
        Command::Lock => {
            let _plan = kernel.plan_lock(&cwd)?;
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
        "M0 不写任何文件，这里只有本次运行的体检。更早的记录保留在 ~/.fangcun/journal（M1 起写入）。"
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
