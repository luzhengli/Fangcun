/**
 * 变更计划的示例数据：照抄 prototype/project/Main.dc.html 的 plan、REG_OPTS、regPlan 与提示条文案。
 *
 * 原型里应用会改界面状态（同步后计划清零、登记后挪进已登记项目），这里用模块变量记着，
 * 刷新页面就回到初始状态。
 */

import type { Applied } from "../bindings/Applied";
import type { ChangePlan } from "../bindings/ChangePlan";
import type { GlobalItemRef } from "../bindings/GlobalItemRef";
import type { HistoryEntry } from "../bindings/HistoryEntry";
import type { ManifestEdit } from "../bindings/ManifestEdit";
import type { SyncMode } from "../bindings/SyncMode";
import type { ToolId } from "../bindings/ToolId";
import { activity } from "./activity";
import { globalRowsNamed, removedGlobal } from "./global";
import { libraryState } from "./library";

const state = {
  synced: false,
  registered: new Set<string>(),
  next: 1,
  /** 回滚到的快照。为空时磁盘在最新一条。 */
  current: null as number | null,
  /** 正在预览的「从全局移除」涉及的全局行，应用后从列表里拿掉。 */
  removing: [] as string[],
  lastRemoved: [] as string[],
  /** 最近一次应用的变更列表，用于撤回计划展示真实文件路径。 */
  lastAppliedChanges: [] as ChangePlan["changes"],
};

/** Main.dc.html 的 plan：同步计划，只装排进来的改动（6.8）。 */
export function syncPlan(projectRoot: string, edits: ManifestEdit[] = []): ChangePlan {
  const name = projectRoot.split("/").pop() ?? projectRoot;
  // 方寸 2（D-35）：磁盘已经和清单一致，计划只装排进来的改动；没有暂存时是空计划。
  const changes: ChangePlan["changes"] = [];
  // 暂存的「在本项目停用」：清单加一节，Claude Code 的开关加 16 个键（原型 larkOff）。
  for (const edit of edits) {
    // 升级 tdd（Main.dc.html 的 tddQueued）：本项目改软链，勾选的项目各一行。
    if (edit.op === "upgrade") {
      changes.push(
        {
          op: "modify",
          path: ".agents/skills/tdd",
          detail: "本机软链改指 tdd@b7e9f0，含新脚本 run-tests.sh",
        },
        { op: "modify", path: "fangcun.lock", detail: "tdd 记下 b7e9f0 的内容哈希" },
      );
      for (const other of edit.projects) {
        const name = other.split("/").pop();
        changes.push({
          op: "modify",
          path: `${name}/.agents/skills/tdd`,
          detail: "改到 b7e9f0 · 另留该项目的快照",
        });
      }
      continue;
    }
    const n = edit.names.length;
    changes.unshift({
      op: "modify",
      path: "fangcun.toml",
      detail: `[disable] ${edit.op === "disable" ? "停用" : "恢复"} skill ${n > 3 ? `${edit.names.slice(0, 2).join("、")} 等 ${n} 项` : edit.names.join("、")}`,
    });
    changes.push({
      op: "modify",
      path: ".claude/settings.json",
      detail: `skillOverrides ${edit.op === "disable" ? "16 个设为 off" : "撤掉方寸写的 16 个"}，其余键不动`,
    });
  }
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
      {
        op: "add",
        path: "fangcun.toml",
        detail: `清单 · mode = "${mode}"（${mode === "link" ? "本机软链" : "拷入仓库"}），从目录现状反推`,
      },
      { op: "add", path: "fangcun.lock", detail: "锁 · 记下每一项的版本与哈希" },
      { op: "modify", path: "~/.fangcun/projects.toml", detail: "登记表新增一节 · 本机，不提交" },
    ],
    notes: [{ tone: "info", text: "全局层的项不进项目清单。登记不移动、不改项目里已有的文件。" }],
    blocked: false,
  };
}

/** 撤销或从活动详情发起回滚：按目标记录或上一次应用的文件列表还原快照状态。 */
export function rollbackPlan(recordId?: string): ChangePlan {
  const matched = recordId ? activity.find((r) => r.id === recordId) : undefined;
  const reverse =
    matched && matched.files.length > 0
      ? matched.files
          .filter((f) => f.op !== "read")
          .map((f) => ({
            op: (f.op === "add" ? "remove" : f.op === "remove" ? "add" : "modify") as
              | "add"
              | "modify"
              | "remove",
            path: f.path,
            detail: "按快照逐字节恢复写入前状态",
          }))
      : state.lastAppliedChanges.length > 0
        ? state.lastAppliedChanges.map((ch) => ({
            op: (ch.op === "add" ? "remove" : ch.op === "remove" ? "add" : "modify") as
              | "add"
              | "modify"
              | "remove",
            path: ch.path,
            detail: "还原成写入前快照状态",
          }))
        : [
            { op: "modify" as const, path: ".agents/skills/tdd", detail: "还原成写入前版本" },
            { op: "modify" as const, path: "fangcun.lock", detail: "恢复锁定哈希" },
          ];
  return {
    id: state.next++,
    title: "回滚到写入前",
    changes: reverse,
    notes: [],
    blocked: false,
  };
}

/** Main.dc.html 的 HIST：evals_studio 的四个快照；同步后多一条 #128。 */
export function history(): HistoryEntry[] {
  const rows: [number, string, string][] = [
    [127, "同步 3 处变更", "2026-11-13T10:42:00"],
    [126, "升级 frontend-design → v1.3.0", "2026-11-12T18:05:00"],
    [125, "收录 project-harness", "2026-11-11T09:30:00"],
    [124, "登记项目，生成清单与锁", "2026-11-10T16:12:00"],
  ];
  if (state.synced) rows.unshift([128, "同步 4 处变更", new Date().toISOString()]);
  const current = state.current ?? rows[0]?.[0];
  return rows.map(([snapshot, title, at]) => ({
    snapshot,
    record: `fixture-${snapshot}`,
    title,
    at: new Date(at).toISOString(),
    kind:
      snapshot === 124
        ? "sync"
        : snapshot === 125
          ? "adopt"
          : snapshot === 126
            ? "upgrade"
            : "sync",
    status: "applied",
    current: snapshot === current,
  }));
}

/** 历史页签「回滚到此」。 */
export function rollbackToPlan(snapshot: number): ChangePlan {
  return {
    id: state.next++,
    title: `回滚到快照 #${snapshot}`,
    changes: [
      { op: "modify", path: ".agents/skills/tdd", detail: `还原成快照 #${snapshot} 时的样子` },
    ],
    notes: [],
    blocked: false,
  };
}

/** 应用：原型同步后的快照是 #128，登记的也写 #128。 */
export function applyPlan(plan: ChangePlan): Applied {
  const rollingTo = /^回滚到快照 #(\d+)$/.exec(plan.title);
  if (rollingTo?.[1]) {
    state.current = Number(rollingTo[1]);
    return { snapshots: [129], record: `fixture-${plan.id}` };
  }
  if (plan.title !== "回滚到写入前") {
    state.lastAppliedChanges = plan.changes;
  }
  if (plan.title.startsWith("即将对")) state.synced = true;
  const registering = /^即将登记 (\S+)，/.exec(plan.title);
  if (registering?.[1]) state.registered.add(registering[1]);
  if (plan.title.startsWith("收录 pr-review")) libraryState.adopted = true;
  if (plan.title.startsWith("从全局层移除")) {
    for (const id of state.removing) removedGlobal.add(id);
    state.lastRemoved = [...state.removing];
  }
  if (plan.title === "回滚到写入前") {
    state.synced = false;
    state.registered.clear();
    libraryState.adopted = false;
    for (const id of state.lastRemoved) removedGlobal.delete(id);
    state.lastRemoved = [];
    state.lastAppliedChanges = [];
  }
  return { snapshots: [128], record: `fixture-${plan.id}` };
}

export function isRegistered(name: string): boolean {
  return state.registered.has(name);
}

/** Checkup.dc.html 的 dup：project-harness 的 5 份副本收录进资产库，已分化的另存一版。 */
export function findingPlan(): ChangePlan {
  const same = ["agent-lab/.claude", "fangcun/.claude", "blog-site/.agents", "notes-kit/.claude"];
  return {
    id: state.next++,
    title: "收录 project-harness：5 份副本，4 份相同、1 份内容已分化",
    changes: [
      ...same.map((dir) => ({
        op: "modify" as const,
        path: `${dir}/skills/project-harness`,
        detail: "换为受管软链 → 资产库 4d9b1eb，事前备份",
      })),
      {
        op: "modify" as const,
        path: "evals_studio/.trae/project-harness",
        detail: "内容已分化，另存为独立版本，换为受管软链",
      },
    ],
    notes: [],
    blocked: false,
  };
}

/** 本地已改的三种处理：写回源仓库 / 还原锁定版 / 转为项目独立文件。 */
export function driftPlan(choice: "promote" | "discard" | "fork"): ChangePlan {
  const plans = {
    promote: {
      title: "将 project-harness 的本地修改写回源仓库",
      changes: [
        {
          op: "modify" as const,
          path: "~/code/luyao-skills/project-harness",
          detail: "写入本地修改到 local:luyao-skills，生成新版本 9c1f2a",
        },
        { op: "modify" as const, path: "fangcun.toml", detail: "project-harness 改为固定 9c1f2a" },
        { op: "modify" as const, path: "fangcun.lock", detail: "project-harness 锁到 9c1f2a" },
      ],
      notes: [
        {
          tone: "info" as const,
          text: "另有 4 个项目也用 project-harness：agent-lab、fangcun、blog-site、notes-kit。写回后上游检查会把它们列进「可更新」，逐个决定是否跟进",
        },
      ],
    },
    discard: {
      title: "还原 project-harness 到锁定版",
      changes: [
        {
          op: "modify" as const,
          path: "~/.fangcun/library/objects/4d9b1eb…",
          detail: "还原资产库里的锁定版 4d9b1eb，链到它的项目一起恢复；本地改过的内容留进快照",
        },
      ],
      notes: [
        {
          tone: "info" as const,
          text: "想找回刚才改过的内容，对这条记录回滚即可（6.7 回滚规则第 4 条）",
        },
      ],
    },
    fork: {
      title: "将 project-harness 转为项目独立文件",
      changes: [
        {
          op: "modify" as const,
          path: "fangcun.toml",
          detail: "[skills] 去掉 project-harness，不再跟随原来源",
        },
        { op: "modify" as const, path: "fangcun.lock", detail: "去掉 project-harness" },
        {
          op: "modify" as const,
          path: ".agents/skills/project-harness",
          detail: "转为本项目自己的独立实体文件，随项目仓库提交",
        },
      ],
      notes: [],
    },
  };
  return { id: state.next++, blocked: false, ...plans[choice] };
}

/** Library.dc.html 的 adopt：pr-review 收录到资产库，原位换为受管软链。 */
export function adoptPlan(asset: string): ChangePlan {
  const dirs = ["~/.claude/skills", "~/.agents/skills", "~/.codeium/windsurf/skills"];
  return {
    id: state.next++,
    title: `收录 ${asset}：存入本机资产库，原位换为软链`,
    changes: [
      {
        op: "add",
        path: "~/.fangcun/lib/sha256-d441…",
        detail: `${asset} 存入资产库 · 记为本地来源`,
      },
      ...dirs.map((dir) => ({
        op: "modify" as const,
        path: `${dir}/${asset}`,
        detail: "换为受管软链 → 资产库 d4410e2，事前备份",
      })),
    ],
    notes: [{ tone: "info", text: "6 个项目照常加载。只换软链、不改清单" }],
    blocked: false,
  };
}

const SKDIR: Record<string, string> = {
  "claude-code": "~/.claude/skills/",
  codex: "~/.codex/skills/",
};

/** Global.dc.html 的待清理计划：当前工具全局目录里的软链或配置项，逐个移出。 */
export function removeGlobalPlan(tool: ToolId, items: GlobalItemRef[]): ChangePlan {
  const rows = globalRowsNamed(items.map((i) => i.name));
  state.removing = rows.map((r) => r.id);
  const changes = rows.map((r) => ({
    op: "remove" as const,
    path:
      r.kind === "skill" ? `${SKDIR[tool] ?? ""}${r.count > 1 ? "lark-*" : r.name}` : r.location,
    detail:
      r.kind === "skill"
        ? `移出全局层（${r.members[0]}@${r.count > 1 ? `3f20c9a，${r.count} 个软链` : "0d81aa3"}），资产库保留正本`
        : `移出全局层（${r.name}），资产库保留正本`,
  }));
  return {
    id: state.next++,
    title: rows.length === 1 ? `从全局层移除 ${rows[0]?.name}` : `从全局层移除 ${rows.length} 项`,
    changes,
    notes: [],
    blocked: false,
  };
}

/** 命令面板回车加入（6.9）：按 `fangcun add` 的三处写入照写。 */
export function addPlan(projectRoot: string, asset: string): ChangePlan {
  const name = asset.split("@").pop() ?? asset;
  return {
    id: state.next++,
    title: `即将对 ${projectRoot.split("/").pop()} 做 3 处变更`,
    changes: [
      { op: "modify", path: "fangcun.toml", detail: `[skills] 加入 ${name}` },
      { op: "modify", path: "fangcun.lock", detail: `记下 ${name} 的版本与哈希` },
      { op: "add", path: `.claude/skills/${name}`, detail: "本机软链到资产库正本" },
    ],
    notes: [],
    blocked: false,
  };
}
