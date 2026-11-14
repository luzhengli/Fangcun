//! Tauri 命令。每个命令对应内核的一个用例，只做参数转换，不写业务逻辑。
//!
//! 新增命令要改三处：这里、`main.rs` 的 `generate_handler!`、
//! 前端的 `src/ipc/commands.ts`。参数和返回值的类型从内核导出，不在前端手写。
//!
//! 现在只有 M0 的只读命令。M1 的写入命令成对出现：`plan_*` 返回变更计划，`apply` 落盘。

use std::path::PathBuf;

use fangcun_core::adapter::ToolInfo;
use fangcun_core::error::ErrorPayload;
use fangcun_core::global::GlobalOverview;
use fangcun_core::journal::{Record, RecordFilter};
use fangcun_core::kernel::{Overview, ProjectSummary};
use fangcun_core::model::ToolId;
use fangcun_core::resolver::{EffectiveContext, Explanation};
use fangcun_core::scanner::CheckupReport;
use tauri::State;

use crate::state::AppState;

type CommandResult<T> = Result<T, ErrorPayload>;

/// 已知的工具，供目标工具分段控件使用。
#[tauri::command]
pub async fn tools(state: State<'_, AppState>) -> CommandResult<Vec<ToolInfo>> {
    state.run(|kernel| Ok(kernel.tools())).await
}

/// 侧栏导航项旁的数字。
#[tauri::command]
pub async fn overview(state: State<'_, AppState>) -> CommandResult<Overview> {
    state.run(|kernel| kernel.overview()).await
}

/// 已登记项目。
#[tauri::command]
pub async fn projects(state: State<'_, AppState>) -> CommandResult<Vec<ProjectSummary>> {
    state.run(|kernel| kernel.projects()).await
}

/// 体检报告（6.2）。
#[tauri::command]
pub async fn checkup(state: State<'_, AppState>) -> CommandResult<CheckupReport> {
    state.run(|kernel| kernel.doctor()).await
}

/// 「重新体检」：重新扫描全盘，返回新的体检报告。
#[tauri::command]
pub async fn rescan(state: State<'_, AppState>) -> CommandResult<CheckupReport> {
    state.run(|kernel| kernel.rescan()).await
}

/// 项目页的生效上下文（6.3）。
#[tauri::command]
pub async fn effective_context(
    state: State<'_, AppState>,
    project_root: PathBuf,
    tool: ToolId,
) -> CommandResult<EffectiveContext> {
    state
        .run(move |kernel| kernel.effective_context(&project_root, &tool))
        .await
}

/// 检查器里「为什么是这个状态」。`project_root` 为空时看全局层。
#[tauri::command]
pub async fn why(
    state: State<'_, AppState>,
    project_root: Option<PathBuf>,
    asset: String,
) -> CommandResult<Vec<Explanation>> {
    state
        .run(move |kernel| kernel.why(project_root.as_deref(), &asset))
        .await
}

/// 全局页（6.6）。
#[tauri::command]
pub async fn global_overview(
    state: State<'_, AppState>,
    tool: ToolId,
) -> CommandResult<GlobalOverview> {
    state.run(move |kernel| kernel.global_overview(&tool)).await
}

/// 活动页（6.7）。
#[tauri::command]
pub async fn activity(
    state: State<'_, AppState>,
    filter: RecordFilter,
) -> CommandResult<Vec<Record>> {
    state.run(move |kernel| kernel.log(&filter)).await
}
