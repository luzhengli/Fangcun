/**
 * 示例数据：不连内核时给界面用的数据来源，只在 `fixtures` 模式下加载。
 *
 * 用途是照着原型复刻界面：`pnpm dev:fixtures` 在浏览器里看，`pnpm screens` 截图后
 * 和 `prototype/screens/` 里同名的原型截图对比。
 *
 * 规则：
 * - 数据照抄原型画板脚本段（`renderVals()`）里的示例数据，不另编。每一项注明取自哪张画板。
 * - 原型里没有的值不要补。缺了就先不提供那个命令，界面会收到 `not-implemented`。
 * - 类型由 `Commands` 约束。原型的数据放不进内核的类型时，改内核的类型，不在这里绕。
 * - 实现一个页面时，把那一页用到的命令补到这里。
 */
import type { ErrorPayload } from "../bindings/ErrorPayload";
import type { SettingsForm } from "../bindings/SettingsForm";
import type { Transport } from "../client";
import type { Commands } from "../commands";
import { activity } from "./activity";
import { checkup } from "./checkup";
import { globalClaude } from "./global";
import { effectiveClaude, effectiveCodex, effectiveRecent, projects, recentProject } from "./main";
import { manifestView } from "./manifest";
import {
  applyPlan,
  findingPlan,
  history,
  initPlan,
  isRegistered,
  rollbackPlan,
  rollbackToPlan,
  syncPlan,
} from "./plans";
import { snapshotHandlers } from "./snapshot";

const handlers: Partial<Commands> = {
  /** 目标工具分段控件。Main.dc.html 页头：Claude Code | Codex。 */
  tools: () => [
    { id: "claude-code", name: "Claude Code", support: "full" },
    { id: "codex", name: "Codex", support: "full" },
  ],

  /** 侧栏导航项旁的数字。Main.dc.html 侧栏：6、38、3、31，活动 2。 */
  overview: () => ({
    projects: isRegistered(recentProject.name) ? 7 : 6,
    library: 38,
    bundles: 3,
    global: 31,
    pending: 2,
  }),

  /** 侧栏项目。Main.dc.html 的 PROJ。 */
  projects: () => [...projects, { ...recentProject, registered: isRegistered(recentProject.name) }],

  /** Checkup.dc.html 的 F。 */
  checkup: () => checkup,
  rescan: () => checkup,

  /** Main.dc.html 的 R 与 layout。 */
  effective_context: ({ projectRoot, tool }) =>
    projectRoot === recentProject.id
      ? effectiveRecent(tool)
      : tool === "codex"
        ? effectiveCodex
        : effectiveClaude,

  /** Global.dc.html 的 ROWS。只有 Claude Code 一侧，Codex 一侧原型只说了「另有 2 项」。 */
  global_overview: () => globalClaude,

  /** Activity.dc.html 的 EV。 */
  activity: ({ filter }) =>
    activity.filter(
      (r) =>
        (filter.kind === null ||
          r.kind === filter.kind ||
          (filter.kind === "upgrade" && r.kind === "updatable")) &&
        (filter.project === null || r.project === filter.project),
    ),

  /** Main.dc.html 的 plan、regPlan，以及提示条的撤销。 */
  plan_sync: ({ projectRoot, edits }) => syncPlan(projectRoot, edits),
  plan_init: ({ projectRoot, mode }) => initPlan(projectRoot, mode),
  /** Main.dc.html 的 REG_OPTS 依据：git 历史里有 1 位提交者，建议链接。 */
  suggest_mode: () => ({ mode: "link", committers: 1 }),
  plan_rollback_record: () => rollbackPlan(),
  plan_rollback: ({ snapshot }) => rollbackToPlan(snapshot),
  /** Checkup.dc.html 的 dup。 */
  plan_finding: () => findingPlan(),
  /** Main.dc.html 的 HIST。 */
  history: () => history(),
  /** Main.dc.html 的 MF。未登记的目录没有清单。 */
  manifest_view: ({ projectRoot }) => (projectRoot === recentProject.id ? null : manifestView()),
  apply_plan: ({ plan }) => applyPlan(plan),
  /** 浏览器里没有系统的选文件夹面板，当作选了 luyao-skills。 */
  pick_directory: () => recentProject.id,

  /** Settings.dc.html：只读模式关，保留 30 天、每个项目至少 20 个。 */
  settings: () => settingsState,
  save_settings: ({ form }) => {
    settingsState = form;
    return form;
  },
};

let settingsState: SettingsForm = { readOnly: false, snapshotDays: 30, snapshotKeep: 20 };

export const fixtureTransport: Transport = async (command, args) => {
  const real = snapshotHandlers();
  const table = (real ?? handlers) as Record<string, ((args: never) => unknown) | undefined>;
  const handler = table[command];
  if (!handler) {
    throw {
      code: "not-implemented",
      message: `示例数据里还没有 ${command}`,
    } satisfies ErrorPayload;
  }
  return handler(args as never);
};
