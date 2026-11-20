/**
 * 变更计划的示例数据：照抄 prototype/project/Main.dc.html 的 plan、REG_OPTS、regPlan 与提示条文案。
 *
 * 原型里应用会改界面状态（同步后计划清零、登记后挪进已登记项目），这里用模块变量记着，
 * 刷新页面就回到初始状态。
 */
import type { Applied } from "../bindings/Applied";
import type { ChangePlan } from "../bindings/ChangePlan";
import type { SyncMode } from "../bindings/SyncMode";

const state = { synced: false, registered: new Set<string>(), next: 1 };

/** Main.dc.html 的 plan：evals_studio 的四处变更。 */
export function syncPlan(projectRoot: string): ChangePlan {
  const name = projectRoot.split("/").pop() ?? projectRoot;
  const changes: ChangePlan["changes"] = state.synced
    ? []
    : [
        { op: "add", path: ".agents/skills/tdd", detail: "链接到 tdd@a1b2c3" },
        { op: "add", path: ".claude/skills/tdd", detail: "相对链接到 .agents/skills/tdd" },
        { op: "modify", path: ".mcp.json", detail: "新增键 mcpServers.github" },
        { op: "remove", path: ".trae/project-harness", detail: "改由资产库管理，已备份" },
      ];
  return {
    id: state.next++,
    title: changes.length > 0 ? `即将对 ${name} 做 ${changes.length} 处变更` : `${name} 已是最新`,
    changes,
    notes: [],
    blocked: false,
  };
}

/** Main.dc.html 的 regPlan：登记写三个文件。 */
export function initPlan(projectRoot: string, mode: SyncMode): ChangePlan {
  const name = projectRoot.split("/").pop() ?? projectRoot;
  return {
    id: state.next++,
    title: `即将登记 ${name}，写 3 个文件`,
    changes: [
      { op: "add", path: "fangcun.toml", detail: `清单 · mode = "${mode}"，从目录现状反推` },
      { op: "add", path: "fangcun.lock", detail: "锁 · 记下每一项的版本与哈希" },
      { op: "modify", path: "~/.fangcun/projects.toml", detail: "登记表新增一节 · 本机，不提交" },
    ],
    notes: [{ tone: "info", text: "全局层的项不进项目清单。登记不移动、不改项目里已有的文件。" }],
    blocked: false,
  };
}

/** 撤销：把上一次写入回滚。原型里撤销直接生效，这里照内核的做法先出计划。 */
export function rollbackPlan(): ChangePlan {
  return {
    id: state.next++,
    title: "回滚到写入前",
    changes: [{ op: "modify", path: "（上一次写入的文件）", detail: "还原成写入前的样子" }],
    notes: [],
    blocked: false,
  };
}

/** 应用：原型同步后的快照是 #128，登记的也写 #128。 */
export function applyPlan(plan: ChangePlan): Applied {
  if (plan.title.startsWith("即将对")) state.synced = true;
  const registering = /^即将登记 (\S+)，/.exec(plan.title);
  if (registering?.[1]) state.registered.add(registering[1]);
  if (plan.title === "回滚到写入前") {
    state.synced = false;
    state.registered.clear();
  }
  return { snapshots: [128], record: `fixture-${plan.id}` };
}

export function isRegistered(name: string): boolean {
  return state.registered.has(name);
}
