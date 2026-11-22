/**
 * 变更计划的示例数据：照抄 prototype/project/Main.dc.html 的 plan、REG_OPTS、regPlan 与提示条文案。
 *
 * 原型里应用会改界面状态（同步后计划清零、登记后挪进已登记项目），这里用模块变量记着，
 * 刷新页面就回到初始状态。
 */

import type { Applied } from "../bindings/Applied";
import type { ChangePlan } from "../bindings/ChangePlan";
import type { HistoryEntry } from "../bindings/HistoryEntry";
import type { ManifestEdit } from "../bindings/ManifestEdit";
import type { SyncMode } from "../bindings/SyncMode";

const state = {
  synced: false,
  registered: new Set<string>(),
  next: 1,
  /** 回滚到的快照。为空时磁盘在最新一条。 */
  current: null as number | null,
};

/** Main.dc.html 的 plan：同步计划，只装排进来的改动（6.8）。 */
export function syncPlan(projectRoot: string, edits: ManifestEdit[] = []): ChangePlan {
  const name = projectRoot.split("/").pop() ?? projectRoot;
  // 方寸 2（D-35）：磁盘已经和清单一致，计划只装排进来的改动；没有暂存时是空计划。
  const changes: ChangePlan["changes"] = [];
  // 暂存的「在本项目停用」：清单加一节，Claude Code 的开关加 16 个键（原型 larkOff）。
  for (const edit of edits) {
    // 升级 tdd（Main.dc.html 的 tddQueued）：本项目改链接，勾选的项目各一行。
    if (edit.op === "upgrade") {
      changes.push(
        {
          op: "modify",
          path: ".agents/skills/tdd",
          detail: "链接改指 tdd@b7e9f0，含新脚本 run-tests.sh",
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

/** Main.dc.html 的 HIST：evals_studio 的四个快照；同步后多一条 #128。 */
export function history(): HistoryEntry[] {
  const rows: [number, string, string][] = [
    [127, "同步 3 处变更", "2026-11-13T10:42:00"],
    [126, "升级 frontend-design → v1.3.0", "2026-11-12T18:05:00"],
    [125, "收编 project-harness", "2026-11-11T09:30:00"],
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

/** Checkup.dc.html 的 dup：project-harness 的 5 份副本收编进资产库，分叉的另存一版。 */
export function findingPlan(): ChangePlan {
  const same = ["agent-lab/.claude", "fangcun/.claude", "blog-site/.agents", "notes-kit/.claude"];
  return {
    id: state.next++,
    title: "收编 project-harness：5 份副本，4 份相同、1 份已分叉",
    changes: [
      ...same.map((dir) => ({
        op: "modify" as const,
        path: `${dir}/skills/project-harness`,
        detail: "换成受管链接 → 资产库 4d9b1eb，事前备份",
      })),
      {
        op: "modify" as const,
        path: "evals_studio/.trae/project-harness",
        detail: "已分叉，另存为独立版本，换成受管链接",
      },
    ],
    notes: [],
    blocked: false,
  };
}

/** 漂移的三种处理（Main.dc.html 的 driftDoneMap）。原型点了直接生效，这里照内核的做法先出计划。 */
export function driftPlan(choice: "promote" | "discard" | "fork"): ChangePlan {
  const plans = {
    promote: {
      title: "回流 project-harness 到 local:luyao-skills",
      changes: [
        {
          op: "modify" as const,
          path: "~/code/luyao-skills/project-harness",
          detail: "回流 project-harness 的修改，生成新版本 9c1f2a",
        },
        { op: "modify" as const, path: "fangcun.toml", detail: "project-harness 改为固定 9c1f2a" },
        { op: "modify" as const, path: "fangcun.lock", detail: "project-harness 锁到 9c1f2a" },
      ],
      notes: [
        {
          tone: "info" as const,
          text: "另有 4 个项目也用 project-harness：agent-lab、fangcun、blog-site、notes-kit。回流后上游检查会把它们列进「可更新」，逐个决定是否跟进",
        },
      ],
    },
    discard: {
      title: "丢弃 project-harness 的修改",
      changes: [
        {
          op: "modify" as const,
          path: "~/.fangcun/library/objects/4d9b1eb…",
          detail: "恢复资产库里的正本 4d9b1eb，链到它的项目一起恢复；改过的内容进快照",
        },
      ],
      notes: [
        {
          tone: "info" as const,
          text: "想找回被丢弃的修改，对这条记录回滚即可（6.7 回滚规则第 4 条）",
        },
      ],
    },
    fork: {
      title: "分叉 project-harness 为本项目专属",
      changes: [
        {
          op: "modify" as const,
          path: "fangcun.toml",
          detail: "[skills] 去掉 project-harness，不再跟随来源",
        },
        { op: "modify" as const, path: "fangcun.lock", detail: "去掉 project-harness" },
        {
          op: "modify" as const,
          path: ".agents/skills/project-harness",
          detail: "改成本项目自己的实体目录，提交进仓库",
        },
      ],
      notes: [],
    },
  };
  return { id: state.next++, blocked: false, ...plans[choice] };
}
