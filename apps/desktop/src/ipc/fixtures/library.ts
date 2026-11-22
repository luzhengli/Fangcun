/**
 * 资产库页的示例数据。照抄 prototype/project/Library.dc.html 脚本段的 PROJS、A、ACTS、tiles。
 *
 * 原型里「收编」会把 pr-review 改成已收编（`adopted`），这里用模块变量记着，撤销时放回。
 */
import type { LibraryOverview } from "../bindings/LibraryOverview";
import type { LibraryRow } from "../bindings/LibraryRow";
import type { LibraryStatus } from "../bindings/LibraryStatus";
import type { LibraryVersion } from "../bindings/LibraryVersion";
import type { SourceGroup } from "../bindings/SourceGroup";
import type { VersionTag } from "../bindings/VersionTag";

const PROJS = ["evals_studio", "agent-lab", "fangcun", "blog-site", "notes-kit", "client-acme"];

const TAG: Record<string, VersionTag> = {
  锁定中: "locked",
  最新: "latest",
  分叉: "fork",
  旧版: "old",
  未收编: "unadopted",
};

/** 原型 `V(id, note, tag, projs)`。 */
function V(
  label: string,
  note: string,
  tag: string,
  projs: [string, string, boolean?][] = [],
): LibraryVersion {
  return {
    label,
    hash: label,
    note,
    tag: TAG[tag] ?? "old",
    users: projs.map(([name, mode, drift]) => ({
      name,
      path: PROJS.includes(name) ? `~/code/${name}` : null,
      mode,
      drift: drift ?? false,
    })),
  };
}

function acts(id: string, list: [string, string][]): LibraryRow["recent"] {
  return list.map(([when, text], i) => ({
    record: `${id}-${i}`,
    at:
      when === "5 分钟前" || when === "刚刚"
        ? "2026-11-14T10:00:00+08:00"
        : `2026-${when}T10:00:00+08:00`,
    kind: text.includes("漂移") ? "drift" : text.includes("收编") ? "adopt" : "sync",
    text,
  }));
}

interface A {
  id: string;
  name: string;
  kind: LibraryRow["kind"];
  source: string;
  group: SourceGroup;
  status: LibraryStatus;
  drift?: number;
  cost: number;
  scripts: boolean;
  summary: string;
  store: string | null;
  versions: LibraryVersion[];
  acts: [string, string][];
}

function row(a: A): LibraryRow {
  const users = new Set(a.versions.flatMap((v) => v.users.map((u) => u.name)));
  const all = [...users].some((n) => n.startsWith("全部"));
  return {
    id: a.id,
    name: a.name,
    kind: a.kind,
    source: a.source,
    group: a.group,
    status: a.status,
    drift: a.drift ?? 0,
    usedBy: all ? PROJS.length : [...users].filter((n) => PROJS.includes(n)).length,
    versions: a.versions,
    resident: Math.round(a.cost * 1000),
    hasScripts: a.scripts,
    store: a.store,
    summary: a.summary,
    recent: acts(a.id, a.acts),
    adoptable: a.status === "unknown-source",
  };
}

export const libraryState = { adopted: false };

const LOCAL = "local:luyao-skills";

function rows(): LibraryRow[] {
  const adopted = libraryState.adopted;
  return [
    row({
      id: "harness",
      name: "project-harness",
      kind: "skill",
      source: LOCAL,
      group: "local",
      status: "drift",
      drift: 1,
      cost: 0.4,
      scripts: false,
      summary: "项目脚手架约定。开发模式：在来源仓库里改，登记为开发模式的项目立即可见。",
      store: "~/.fangcun/lib/sha256-4d9b…",
      versions: [
        V("4d9b1eb", "7 月版 · 07-02", "锁定中", [
          ["evals_studio", "链接", true],
          ["agent-lab", "链接"],
          ["fangcun", "链接"],
          ["blog-site", "入库"],
          ["notes-kit", "链接"],
        ]),
        V("e81c55a", "evals_studio/.trae 分叉", "分叉"),
        V("2b77c10", "06-11", "旧版"),
      ],
      acts: [
        ["5 分钟前", "evals_studio 中的副本发生漂移"],
        ["11-11", "收编进资产库"],
        ["07-02", "发布 4d9b1eb"],
      ],
    }),
    row({
      id: "tdd",
      name: "tdd",
      kind: "skill",
      source: "github:obra/superpowers",
      group: "github",
      status: "updatable",
      cost: 0.2,
      scripts: true,
      summary: "测试驱动开发流程。跟随 main 分支，有新提交时提醒，确认后才更新。",
      store: "~/.fangcun/lib/sha256-a1b2…",
      versions: [
        V("b7e9f0", "11-12 · 新增脚本", "最新"),
        V("a1b2c3", "10-28", "锁定中", [
          ["evals_studio", "套装"],
          ["agent-lab", "套装"],
          ["fangcun", "套装"],
        ]),
      ],
      acts: [
        ["11-12", "上游新提交 b7e9f0"],
        ["10-28", "3 个项目升级到 a1b2c3"],
      ],
    }),
    row({
      id: "skillb",
      name: "skill-b",
      kind: "skill",
      source: "github:acme/skills",
      group: "github",
      status: "synced",
      cost: 0.3,
      scripts: false,
      summary: "同一资产的两个大版本并存，各项目按需固定。",
      store: "~/.fangcun/lib/sha256-77e0…",
      versions: [
        V("v2.1.0", "11-02", "最新", [["client-acme", "入库"]]),
        V("v1.0.0", "08-15", "旧版", [["agent-lab", "链接"]]),
      ],
      acts: [
        ["11-02", "发布 v2.1.0，client-acme 升级"],
        ["08-15", "agent-lab 固定 v1.0.0"],
      ],
    }),
    row({
      id: "fed",
      name: "frontend-design",
      kind: "skill",
      source: "github:acme/skills",
      group: "github",
      status: "synced",
      cost: 0.2,
      scripts: false,
      summary: "前端视觉规范。全局层另有一份旧版 v1.1.0，被项目层遮蔽。",
      store: "~/.fangcun/lib/sha256-c03d…",
      versions: [
        V("v1.3.0", "11-12", "锁定中", [
          ["evals_studio", "链接"],
          ["blog-site", "入库"],
        ]),
        V("v1.1.0", "09-20 · 全局层仍在用", "旧版", [["全局层", "软链"]]),
      ],
      acts: [
        ["11-12", "发布 v1.3.0，2 个项目升级"],
        ["09-20", "v1.1.0 装入全局"],
      ],
    }),
    row({
      id: "github",
      name: "github",
      kind: "mcp",
      source: "npx @modelcontextprotocol/server-github",
      group: "npm",
      status: "synced",
      cost: 4.8,
      scripts: true,
      summary: "GitHub MCP 服务。提交进仓库的配置只含环境变量引用，令牌在钥匙串。",
      store: "~/.fangcun/mcp/github.toml",
      versions: [
        V("npx -y", "随 npm 最新", "锁定中", [
          ["evals_studio", ".mcp.json"],
          ["agent-lab", ".mcp.json"],
          ["fangcun", "Codex 配置"],
          ["client-acme", ".mcp.json"],
        ]),
      ],
      acts: [
        ["11-10", "密钥迁入钥匙串"],
        ["11-10", "写入 4 个项目的配置"],
      ],
    }),
    row({
      id: "agents",
      name: "AGENTS.md 工程基线",
      kind: "instruction",
      source: "local:luyao-skills/instructions",
      group: "local",
      status: "synced",
      cost: 1.1,
      scripts: false,
      summary: "指令片段。生成为各项目 AGENTS.md 的一部分，CLAUDE.md 以引用镜像。",
      store: "~/.fangcun/lib/sha256-91fa…",
      versions: [
        V("5e2a7c1", "11-01", "锁定中", [
          ["evals_studio", "正本"],
          ["agent-lab", "正本"],
          ["fangcun", "正本"],
        ]),
      ],
      acts: [["11-01", "正本更新到 5e2a7c1"]],
    }),
    adopted
      ? row({
          id: "prrev",
          name: "pr-review",
          kind: "skill",
          source: "~/.agents/skills/pr-review",
          group: "local",
          status: "adopted",
          cost: 0.3,
          scripts: false,
          summary: "已收编为本地来源，原位置替换为受管链接，事前已备份。",
          store: "~/.fangcun/lib/sha256-d441…",
          versions: [V("d4410e2", "收编于刚刚", "锁定中", [["全部 6 个项目", "全局层"]])],
          acts: [
            ["刚刚", "收编进资产库"],
            ["11-11", "体检发现 · 来源未知"],
          ],
        })
      : row({
          id: "prrev",
          name: "pr-review",
          kind: "skill",
          source: "~/.agents/skills/pr-review",
          group: "unknown",
          status: "unknown-source",
          cost: 0.3,
          scripts: false,
          summary: "全局层发现，内容哈希与任何来源都不匹配。收编后才能追踪版本与更新。",
          store: null,
          versions: [V("d4410e2", "本地内容哈希", "未收编", [["全部 6 个项目", "全局层"]])],
          acts: [["11-11", "体检发现 · 来源未知"]],
        }),
  ];
}

/** 资产库页。原型的概要写的是全库的数（38 项、5 个来源），列表只画了 7 行，这里照抄。 */
export function libraryOverview(): LibraryOverview {
  const adopted = libraryState.adopted;
  return {
    projectTotal: PROJS.length,
    kinds: { skill: 31, mcp: 4, instruction: 3 },
    sources: [
      { source: LOCAL, group: "local", count: 2 },
      { source: "github:obra/superpowers", group: "github", count: 1 },
      { source: "github:acme/skills", group: "github", count: 2 },
      { source: "npm", group: "npm", count: 1 },
      ...(adopted ? [] : [{ source: "来源未知", group: "unknown" as const, count: 1 }]),
    ],
    multiVersion: 4,
    drift: 1,
    updatable: 1,
    unknown: adopted ? 0 : 1,
    rows: rows(),
    projects: PROJS,
  };
}
