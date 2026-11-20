/**
 * 内核命令的类型化封装。与 `src-tauri/src/commands.rs` 一一对应，顺序也一致。
 *
 * 类型来自 `bindings/`，由内核导出（`pnpm bindings`），不要在这里手写数据结构。
 * 参数名在 Rust 一侧是 snake_case，Tauri 会转成这里的 camelCase。
 */
import type { Applied } from "./bindings/Applied";
import type { ChangePlan } from "./bindings/ChangePlan";
import type { CheckupReport } from "./bindings/CheckupReport";
import type { EffectiveContext } from "./bindings/EffectiveContext";
import type { Explanation } from "./bindings/Explanation";
import type { GlobalOverview } from "./bindings/GlobalOverview";
import type { ModeSuggestion } from "./bindings/ModeSuggestion";
import type { Overview } from "./bindings/Overview";
import type { ProjectSummary } from "./bindings/ProjectSummary";
import type { Record as ActivityRecord } from "./bindings/Record";
import type { RecordFilter } from "./bindings/RecordFilter";
import type { SettingsForm } from "./bindings/SettingsForm";
import type { SnapshotId } from "./bindings/SnapshotId";
import type { SyncMode } from "./bindings/SyncMode";
import type { ToolId } from "./bindings/ToolId";
import type { ToolInfo } from "./bindings/ToolInfo";
import { call } from "./client";

/**
 * 命令表：命令名 → 参数与返回值。内核调用和示例数据都按它检查类型，
 * 所以内核的类型一改，对不上的示例数据会在 `pnpm typecheck` 里报出来。
 */
export interface Commands {
  tools: () => ToolInfo[];
  overview: () => Overview;
  projects: () => ProjectSummary[];
  checkup: () => CheckupReport;
  rescan: () => CheckupReport;
  effective_context: (args: { projectRoot: string; tool: ToolId }) => EffectiveContext;
  why: (args: { projectRoot: string | null; asset: string }) => Explanation[];
  global_overview: (args: { tool: ToolId }) => GlobalOverview;
  activity: (args: { filter: RecordFilter }) => ActivityRecord[];
  plan_sync: (args: { projectRoot: string }) => ChangePlan;
  suggest_mode: (args: { projectRoot: string }) => ModeSuggestion;
  plan_init: (args: { projectRoot: string; mode: SyncMode }) => ChangePlan;
  plan_adopt: (args: { asset: string }) => ChangePlan;
  plan_rollback: (args: { projectRoot: string; snapshot: SnapshotId }) => ChangePlan;
  plan_rollback_record: (args: { recordId: string }) => ChangePlan;
  apply_plan: (args: { plan: ChangePlan }) => Applied;
  pick_directory: () => string | null;
  settings: () => SettingsForm;
  save_settings: (args: { form: SettingsForm }) => SettingsForm;
}

export const kernel = {
  tools: () => call("tools"),
  overview: () => call("overview"),
  projects: () => call("projects"),
  checkup: () => call("checkup"),
  rescan: () => call("rescan"),
  effectiveContext: (projectRoot: string, tool: ToolId) =>
    call("effective_context", { projectRoot, tool }),
  why: (projectRoot: string | null, asset: string) => call("why", { projectRoot, asset }),
  globalOverview: (tool: ToolId) => call("global_overview", { tool }),
  activity: (filter: RecordFilter) => call("activity", { filter }),
  planSync: (projectRoot: string) => call("plan_sync", { projectRoot }),
  suggestMode: (projectRoot: string) => call("suggest_mode", { projectRoot }),
  planInit: (projectRoot: string, mode: SyncMode) => call("plan_init", { projectRoot, mode }),
  planAdopt: (asset: string) => call("plan_adopt", { asset }),
  planRollback: (projectRoot: string, snapshot: SnapshotId) =>
    call("plan_rollback", { projectRoot, snapshot }),
  planRollbackRecord: (recordId: string) => call("plan_rollback_record", { recordId }),
  applyPlan: (plan: ChangePlan) => call("apply_plan", { plan }),
  pickDirectory: () => call("pick_directory"),
  settings: () => call("settings"),
  saveSettings: (form: SettingsForm) => call("save_settings", { form }),
};
