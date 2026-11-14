/**
 * 真实数据快照：`pnpm screens --snapshot <文件>` 把内核跑出来的结果注入页面，
 * 用来在浏览器里看本机真实数据下的界面，不用开桌面窗口。快照由
 * `cargo run -p fangcun-core --example ipc_snapshot -- <文件>` 生成。
 *
 * 只在 fixtures 模式下加载，和示例数据一样不会进发布版。
 */
import type { CheckupReport } from "../bindings/CheckupReport";
import type { EffectiveContext } from "../bindings/EffectiveContext";
import type { GlobalOverview } from "../bindings/GlobalOverview";
import type { Overview } from "../bindings/Overview";
import type { ProjectSummary } from "../bindings/ProjectSummary";
import type { Record as ActivityRecord } from "../bindings/Record";
import type { ToolInfo } from "../bindings/ToolInfo";
import type { Commands } from "../commands";

interface Snapshot {
  tools: ToolInfo[];
  overview: Overview;
  projects: ProjectSummary[];
  checkup: CheckupReport;
  /** 键是「项目根目录|工具」。 */
  effective: Record<string, EffectiveContext>;
  global: Record<string, GlobalOverview>;
  activity: ActivityRecord[];
}

export function snapshotHandlers(): Partial<Commands> | null {
  const snapshot = (window as { __FANGCUN_SNAPSHOT__?: Snapshot }).__FANGCUN_SNAPSHOT__;
  if (!snapshot) return null;
  const pick = <T>(value: T | undefined, what: string): T => {
    if (value === undefined) throw { code: "not-found", message: `快照里没有 ${what}` };
    return value;
  };
  return {
    tools: () => snapshot.tools,
    overview: () => snapshot.overview,
    projects: () => snapshot.projects,
    checkup: () => snapshot.checkup,
    rescan: () => snapshot.checkup,
    effective_context: ({ projectRoot, tool }) =>
      pick(snapshot.effective[`${projectRoot}|${tool}`], `${projectRoot} 的 ${tool}`),
    global_overview: ({ tool }) => pick(snapshot.global[tool], `${tool} 的全局层`),
    activity: ({ filter }) =>
      snapshot.activity.filter(
        (r) =>
          (filter.kind === null || r.kind === filter.kind) &&
          (filter.project === null || r.project === filter.project),
      ),
  };
}
