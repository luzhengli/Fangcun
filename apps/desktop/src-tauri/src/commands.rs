//! Tauri 命令。每个命令对应内核的一个用例，只做参数转换，不写业务逻辑。
//!
//! 新增命令要改三处：这里、`main.rs` 的 `generate_handler!`、
//! 前端的 `src/ipc/commands.ts`。参数和返回值的类型从内核导出，不在前端手写。
//!
//! 写入命令成对出现：`plan_*` 返回变更计划（6.8），`apply_plan` 落盘。界面先把计划给人看，
//! 人点「应用」才调 `apply_plan`；内核只认自己出过的计划，按编号找回动作。

use std::path::PathBuf;

use fangcun_core::adapter::ToolInfo;
use fangcun_core::error::ErrorPayload;
use fangcun_core::global::GlobalOverview;
use fangcun_core::journal::{HistoryEntry, Record, RecordFilter};
use fangcun_core::kernel::{ManifestView, Overview, ProjectSummary, UpstreamReport};
use fangcun_core::materializer::drift::DriftChoice;
use fangcun_core::materializer::{Applied, ChangePlan};
use fangcun_core::model::{SnapshotId, SyncMode, ToolId};
use fangcun_core::resolver::{EffectiveContext, Explanation, ModeSuggestion};
use fangcun_core::scanner::CheckupReport;
use fangcun_core::settings::SettingsForm;
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

/// 「同步 · N」：按清单与锁物化这个项目（6.8、M1-13）。`edits` 是界面暂存的清单改动，例如在本项目停用。
#[tauri::command]
pub async fn plan_sync(
    state: State<'_, AppState>,
    project_root: PathBuf,
    edits: Option<Vec<fangcun_core::manifest::ManifestEdit>>,
) -> CommandResult<ChangePlan> {
    let edits = edits.unwrap_or_default();
    state
        .run(move |kernel| kernel.plan_sync_with(&project_root, &edits))
        .await
}

/// 登记时建议的物化方式（9.2、M1-25）。
#[tauri::command]
pub async fn suggest_mode(
    state: State<'_, AppState>,
    project_root: PathBuf,
) -> CommandResult<ModeSuggestion> {
    state
        .run(move |kernel| Ok(kernel.suggest_mode(&project_root)))
        .await
}

/// 登记项目：从目录现状反推清单与锁（F3、M1-25）。`mode` 是人在登记弹窗里选的。
#[tauri::command]
pub async fn plan_init(
    state: State<'_, AppState>,
    project_root: PathBuf,
    mode: SyncMode,
) -> CommandResult<ChangePlan> {
    state
        .run(move |kernel| kernel.plan_init(&project_root, true, Some(mode)))
        .await
}

/// 收编一项资产进资产库（F1、M1-26 体检的「重复」）。
#[tauri::command]
pub async fn plan_adopt(state: State<'_, AppState>, asset: String) -> CommandResult<ChangePlan> {
    state.run(move |kernel| kernel.plan_adopt(&asset)).await
}

/// 历史页签「回滚到此」：把项目回滚到某个快照（6.3、M1-14）。
#[tauri::command]
pub async fn plan_rollback(
    state: State<'_, AppState>,
    project_root: PathBuf,
    snapshot: SnapshotId,
) -> CommandResult<ChangePlan> {
    state
        .run(move |kernel| kernel.plan_rollback(&project_root, snapshot))
        .await
}

/// 活动页「回滚到此之前…」、提示条的「撤销」：撤销一条活动记录（6.7、M1-14）。
#[tauri::command]
pub async fn plan_rollback_record(
    state: State<'_, AppState>,
    record_id: String,
) -> CommandResult<ChangePlan> {
    state
        .run(move |kernel| kernel.plan_rollback_record(&record_id))
        .await
}

/// 应用一份计划。写入只发生在这里。
#[tauri::command]
pub async fn apply_plan(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    plan: ChangePlan,
) -> CommandResult<Applied> {
    let applied = state.run(move |kernel| kernel.apply(&plan)).await?;
    // 可能登记了新项目、换了正本：按现在的目录重设监听。
    AppState::watch(&app);
    Ok(applied)
}

/// 侧栏「添加项目…」：弹系统的选择文件夹面板，返回选中的目录（F3、M1-25）。
///
/// 对话框只从外壳这一侧调，界面拿到的只是路径，不给界面文件权限。
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> CommandResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let picked = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("选择要登记的项目目录")
            .blocking_pick_folder()
    })
    .await
    .map_err(|join_error| ErrorPayload::internal(join_error.to_string()))?;
    Ok(picked
        .and_then(|path| path.into_path().ok())
        .map(|path| path.to_string_lossy().into_owned()))
}

/// 设置页（6.1、M1-24）。
#[tauri::command]
pub async fn settings(state: State<'_, AppState>) -> CommandResult<SettingsForm> {
    state.run(|kernel| kernel.settings()).await
}

/// 设置页保存。只读开关变了就按新设置重开内核，之后的命令按新的读写权限走。
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    form: SettingsForm,
) -> CommandResult<SettingsForm> {
    let before = state.run(|kernel| kernel.settings()).await?;
    let saved = state.run(move |kernel| kernel.save_settings(&form)).await?;
    if saved.read_only != before.read_only {
        state.reopen()?;
    }
    Ok(saved)
}

/// 项目页「历史」页签（6.3、M1-14）。
#[tauri::command]
pub async fn history(
    state: State<'_, AppState>,
    project_root: PathBuf,
) -> CommandResult<Vec<HistoryEntry>> {
    state.run(move |kernel| kernel.history(&project_root)).await
}

/// 项目页「清单」页签（6.3、M1-31）。没有清单时为空。
#[tauri::command]
pub async fn manifest_view(
    state: State<'_, AppState>,
    project_root: PathBuf,
) -> CommandResult<Option<ManifestView>> {
    state
        .run(move |kernel| kernel.manifest_view(&project_root))
        .await
}

/// 体检报告的一键动作（6.2、M1-26）。
#[tauri::command]
pub async fn plan_finding(
    state: State<'_, AppState>,
    finding_id: String,
) -> CommandResult<ChangePlan> {
    state
        .run(move |kernel| kernel.plan_finding(&finding_id))
        .await
}

/// 项目页检查器的漂移块：回流、丢弃、分叉（6.3、M1-15）。
#[tauri::command]
pub async fn plan_drift(
    state: State<'_, AppState>,
    project_root: PathBuf,
    asset: String,
    choice: DriftChoice,
) -> CommandResult<ChangePlan> {
    state
        .run(move |kernel| kernel.plan_drift(&project_root, &asset, choice))
        .await
}

/// 手动的上游检查（M1-16，9.3）。会联网拉 GitHub 来源。
#[tauri::command]
pub async fn check_upstream(state: State<'_, AppState>) -> CommandResult<UpstreamReport> {
    state.run(|kernel| kernel.check_upstream()).await
}
