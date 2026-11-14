/**
 * 活动页的示例数据。照抄 prototype/project/Activity.dc.html 脚本段的 EV（前十二条）。
 * 时间按原型的「d13 = 11-13」写成本地时间。快照有多个时，列表只显示第一个编号（6.7）。
 */
import type { FileOp } from "../bindings/FileOp";
import type { Record as ActivityRecord } from "../bindings/Record";
import type { RecordKind } from "../bindings/RecordKind";
import type { RecordStatus } from "../bindings/RecordStatus";
import type { Trigger } from "../bindings/Trigger";

const OP: Record<string, FileOp> = { "+": "add", "~": "modify", "−": "remove", "=": "read" };

function E(
  id: string,
  day: string,
  time: string,
  kind: RecordKind,
  projects: string[],
  title: string,
  x: {
    status: RecordStatus;
    trigger?: Trigger;
    snap?: number;
    summary: string;
    changes: [string, string, string][];
    cli: string;
  },
): ActivityRecord {
  const [command, ...output] = x.cli.replace(/^\$ /, "").split("\n");
  return {
    id,
    at: new Date(`2026-${day}T${time}:00`).toISOString(),
    kind,
    title,
    project:
      projects.length === 1 && projects[0] !== "global" ? `/Users/me/code/${projects[0]}` : null,
    projectLabel:
      projects[0] === "global"
        ? "全局层"
        : projects.length <= 2
          ? projects.join("、")
          : `${projects[0]} 等 ${projects.length} 个项目`,
    trigger: x.trigger ?? "desktop",
    snapshot: x.snap ?? null,
    status: x.status,
    summary: x.summary,
    files: x.changes.map(([op, path, note]) => ({ op: OP[op] ?? "read", path, note })),
    command: command ?? null,
    output: output.length > 0 ? output.join("\n") : null,
  };
}

export const activity: ActivityRecord[] = [
  E("drift1", "11-13", "11:37", "drift", ["evals_studio"], "project-harness 的副本被修改", {
    status: "pending",
    trigger: "watcher",
    summary:
      "Agent 在工作时改进了项目里的 skill 副本。改动还留在副本里，等你决定回流、丢弃或分叉。",
    changes: [
      ["~", ".agents/skills/project-harness/SKILL.md", "+12 −3 行 · 与正本 4d9b1eb 不一致"],
    ],
    cli: "$ fangcun status\nevals_studio  1 处漂移  project-harness",
  }),
  E(
    "tdd1",
    "11-12",
    "16:20",
    "updatable",
    ["evals_studio", "agent-lab", "fangcun"],
    "tdd 上游有新提交 b7e9f0",
    {
      status: "pending",
      trigger: "upstream-check",
      summary: "跟随 main 的 tdd 有新版本。差异里新增了一个脚本，需要你确认后才会更新。",
      changes: [
        ["~", "SKILL.md", "+8 −2 行"],
        ["+", "scripts/run-tests.sh", "新增脚本 · 会执行命令"],
      ],
      cli: "$ fangcun update tdd --dry-run",
    },
  ),
  E("e127", "11-13", "10:42", "sync", ["evals_studio"], "同步 3 处变更", {
    status: "applied",
    snap: 127,
    summary: "加入工程基线套装里的 tdd，并为 Claude Code 写入 github MCP 配置。",
    changes: [
      ["+", ".agents/skills/tdd", "链接到 tdd@a1b2c3"],
      ["+", ".claude/skills/tdd", "相对链接到 .agents/skills/tdd"],
      ["~", ".mcp.json", "新增键 mcpServers.github"],
    ],
    cli: "$ fangcun sync\n3 处变更 · 快照 #127",
  }),
  E("k1", "11-13", "10:38", "secret", ["evals_studio"], "github-token 迁入钥匙串", {
    status: "applied",
    summary:
      "为 github MCP 存入访问令牌。这里只记名称和位置，从不记值。提交进仓库的 .mcp.json 只含 secret:github-token 引用。",
    changes: [
      ["+", "钥匙串 · fangcun / github-token", "存入 · 值不记录"],
      ["=", ".mcp.json", "env.GITHUB_TOKEN = secret:github-token"],
    ],
    cli: "$ fangcun secret set github-token\n已存入钥匙串",
  }),
  E("a41", "11-13", "09:15", "sync", ["client-acme"], "更新锁并同步，修复 CI", {
    status: "applied",
    snap: 41,
    trigger: "cli",
    summary: "重新解析清单，把新增的 playwright 写进锁，再物化到项目目录。下一次校验应能通过。",
    changes: [
      ["~", "fangcun.lock", "新增 playwright 条目"],
      ["~", ".mcp.json", "新增键 mcpServers.playwright"],
    ],
    cli: "$ fangcun lock && fangcun sync\n2 处变更 · 快照 #41",
  }),
  E("ci1", "11-12", "21:30", "verify", ["client-acme"], "校验失败：锁与清单不一致", {
    status: "failed",
    trigger: "cli",
    summary:
      "提交前钩子在本机运行校验。fangcun.toml 新增了 playwright，但 fangcun.lock 没有更新，--frozen 模式下拒绝物化。",
    changes: [
      ["~", "fangcun.toml", "[mcp.playwright] 已声明"],
      ["=", "fangcun.lock", "缺少 playwright 条目"],
    ],
    cli: "$ fangcun sync --frozen\n错误：锁与清单不一致（1 项）",
  }),
  E(
    "e126",
    "11-12",
    "18:05",
    "upgrade",
    ["evals_studio", "blog-site"],
    "升级 frontend-design v1.1.0 → v1.3.0",
    {
      status: "applied",
      snap: 126,
      summary:
        "从活动的待处理项进入，在项目页检查器里勾选两个项目一起升级。差异里没有脚本或命令变更。",
      changes: [
        ["~", "evals_studio/fangcun.lock", "frontend-design → v1.3.0"],
        ["~", "evals_studio/.agents/skills/frontend-design", "改链到 v1.3.0"],
        ["~", "blog-site/.agents/skills/frontend-design", "入库文件更新为 v1.3.0"],
      ],
      cli: "$ fangcun update frontend-design --projects evals_studio,blog-site",
    },
  ),
  E("b90", "11-12", "14:02", "rollback", ["agent-lab"], "回滚到快照 #88", {
    status: "applied",
    snap: 90,
    summary: "撤销 skill-b 的升级。磁盘状态与快照 #88 逐字节一致，v2.1.0 仍保留在资产库。",
    changes: [
      ["~", "fangcun.lock", "skill-b → v1.0.0"],
      ["~", ".agents/skills/skill-b", "改链回 v1.0.0"],
    ],
    cli: "$ fangcun rollback #88",
  }),
  E("b89", "11-12", "13:40", "upgrade", ["agent-lab"], "升级 skill-b v1.0.0 → v2.1.0", {
    status: "rolled-back",
    snap: 89,
    summary: "升级后 agent-lab 的测试流程失败，随后在 14:02 回滚。",
    changes: [
      ["~", "fangcun.lock", "skill-b → v2.1.0"],
      ["~", ".agents/skills/skill-b", "改链到 v2.1.0"],
    ],
    cli: "$ fangcun update skill-b",
  }),
  E("g1", "11-12", "10:10", "sync", ["global"], "全局层：frontend-design 被 2 个项目覆盖", {
    status: "recorded",
    trigger: "watcher",
    summary: "两个项目在项目层固定了 v1.3.0，全局的 v1.1.0 在那里被遮蔽。没有写入任何文件。",
    changes: [["=", "~/.agents/skills/frontend-design", "v1.1.0 · 被遮蔽"]],
    cli: "$ fangcun why frontend-design --global",
  }),
  E(
    "h125",
    "11-11",
    "09:30",
    "adopt",
    ["evals_studio", "agent-lab", "fangcun", "blog-site", "notes-kit"],
    "收编 project-harness：5 份副本归并为 1 份正本",
    {
      status: "applied",
      snap: 125,
      summary:
        "4 份相同副本替换为受管链接；evals_studio/.trae 里已分叉的那份另存为分叉版本 e81c55a，事前全部备份。",
      changes: [
        ["+", "~/.fangcun/lib/project-harness@4d9b1eb", "正本入库"],
        ["~", "4 个项目的 .agents/skills/project-harness", "替换为受管链接"],
        ["−", "evals_studio/.trae/project-harness", "改由资产库管理，已备份"],
      ],
      cli: "$ fangcun adopt project-harness",
    },
  ),
  E("chk1", "11-11", "09:12", "checkup", ["global"], "首次体检：扫描 7 个工具、61 个项目目录", {
    status: "read-only",
    trigger: "first-launch",
    summary: "只读扫描，未改动任何文件。",
    changes: [],
    cli: "$ fangcun doctor",
  }),
];
