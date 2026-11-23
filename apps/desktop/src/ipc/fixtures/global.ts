/**
 * 全局页的示例数据。照抄 prototype/project/Global.dc.html 脚本段的 PROJS、ROWS（Claude Code 一侧）。
 */
import type { Distribution } from "../bindings/Distribution";
import type { GlobalOverview } from "../bindings/GlobalOverview";
import type { GlobalRow } from "../bindings/GlobalRow";
import type { GlobalStatus } from "../bindings/GlobalStatus";

const PROJS = ["evals_studio", "agent-lab", "fangcun", "blog-site", "notes-kit", "client-acme"];
const TOOLN = { cc: "Claude Code", cx: "Codex", ws: "Windsurf" } as const;
const SKDIR = {
  cc: "~/.claude/skills/",
  cx: "~/.codex/skills/",
  ws: "~/.codeium/windsurf/skills/",
} as const;
type T = keyof typeof TOOLN;

interface X {
  /** 命令行写法里的资产名，原型的 `cli`。 */
  cli?: string;
  src: string;
  ver: string;
  summary: string;
  advice: string;
  script?: boolean;
  shadowed?: string[];
  /** 项目层有同名项、但没能盖过全局这一份的项目，原型的 `conflict`。 */
  conflict?: string[];
  /** 指令文件全文加载，没有触发可统计，原型的 `fulltext`。 */
  fulltext?: boolean;
  act: "bundle" | "remove" | "keep" | "adopt";
  /** 检查器「最近活动」，原型的 `acts`：日期 MM-DD 与一句话。 */
  acts?: [string, string][];
}

/** 原型的 `acts` 写成活动记录的引用。类型按文字猜，只影响图标。 */
function recentOf(id: string, acts: [string, string][] = []): GlobalRow["recent"] {
  return acts.map(([day, text], i) => ({
    record: `${id}-${i}`,
    at: `2026-${day}T10:00:00+08:00`,
    kind: text.startsWith("体检") ? "checkup" : text.startsWith("上游") ? "upgrade" : "sync",
    text,
  }));
}

const ACTIONS: Record<X["act"], string[]> = {
  bundle: ["转成套装…", "从全局移除…"],
  remove: ["从全局移除…"],
  keep: [],
  adopt: ["去资产库收录"],
};

function R(
  id: string,
  name: string,
  kind: GlobalRow["kind"],
  count: number,
  location: string,
  tools: T[],
  cost: number,
  used: string[],
  status: GlobalStatus,
  x: X,
): GlobalRow {
  const shadowed = x.shadowed ?? [];
  const conflict = x.conflict ?? [];
  const distribution: Distribution[] = (["cc", "cx", "ws"] as T[]).map((t) => ({
    tool: t,
    toolName: TOOLN[t],
    path: tools.includes(t)
      ? kind === "skill"
        ? `${SKDIR[t]}${count > 1 ? "lark-*" : name}`
        : location
      : null,
    label: tools.includes(t)
      ? kind === "skill"
        ? "已软链"
        : kind === "mcp"
          ? "按键合并"
          : "全文加载"
      : kind === "skill"
        ? "未分发"
        : "未配置",
    present: tools.includes(t),
  }));
  return {
    id,
    name,
    kind,
    count,
    members: [x.cli ?? name],
    location,
    source: x.src,
    version: x.ver,
    resident: Math.round(cost * 1000),
    usedBy: x.fulltext ? null : used.length,
    status,
    overridden: shadowed.length,
    conflicts: conflict.length,
    hasScripts: x.script ?? false,
    inLibrary: status !== "unknown-source",
    command: null,
    usage: x.fulltext
      ? []
      : PROJS.map((p) => ({
          name: p,
          path: `~/code/${p}`,
          mark: shadowed.includes(p)
            ? "overridden"
            : conflict.includes(p)
              ? "conflict"
              : used.includes(p)
                ? "used"
                : "unused",
          triggers: used.includes(p) ? 1 : 0,
        })),
    distribution,
    summary: x.summary,
    advice: x.advice,
    actions: ACTIONS[x.act],
    recent: recentOf(id, x.acts),
    tools,
  } as GlobalRow & { tools: T[] };
}

const SK = (
  id: string,
  name: string,
  tools: T[],
  cost: number,
  used: string[],
  status: GlobalStatus,
  x: X,
) => R(id, name, "skill", 1, `~/.agents/skills/${name}`, tools, cost, used, status, x);

const ANTH = "github:anthropics/skills";
const MAIN = "跟随 main · 0d81aa3";
const NEVER = "近 30 天没有任何项目触发它。可以直接从全局移除，资产库仍保留正本，随时可加回。";

type Row = GlobalRow & { tools: T[] };

const ROWS: Row[] = [
  R(
    "lark",
    "lark-* 飞书 skill",
    "skill",
    16,
    "~/.agents/skills/lark-*",
    ["cc", "ws"],
    6.4,
    ["client-acme"],
    "suggest-bundle",
    {
      src: "github:larksuite/skills",
      ver: "跟随 main · 3f20c9a",
      act: "bundle",
      cli: "lark-doc",
      summary:
        "16 个飞书办公 skill。名称与描述常驻每个项目的上下文，近 30 天只有 client-acme 触发过。",
      advice:
        "只有 client-acme 用到。转成套装、只给 client-acme 启用，其余 5 个项目每个省下 6.4k 常驻。",
      acts: [
        ["11-11", "体检发现：16 个飞书 skill 进了每个项目"],
        ["11-04", "上游更新到 3f20c9a"],
        ["09-02", "装入全局层"],
      ],
    },
  ) as Row,
  R(
    "playwright",
    "playwright",
    "mcp",
    1,
    "~/.codex/config.toml · mcp_servers.playwright",
    ["cx"],
    3.6,
    ["blog-site", "client-acme"],
    "suggest-bundle",
    {
      src: "npx @playwright/mcp",
      ver: "npx -y · 随 npm 最新",
      act: "bundle",
      script: true,
      summary: "浏览器自动化 MCP 服务。21 个工具定义常驻 Codex 的上下文，只有两个前端项目用到。",
      advice: "只有 blog-site、client-acme 用到。转成套装后，其余 4 个项目每个省下 3.6k 常驻。",
      acts: [
        ["10-30", "体检发现：工具定义占 3.6k"],
        ["08-21", "写入 ~/.codex/config.toml"],
      ],
    },
  ) as Row,
  R(
    "context7",
    "context7",
    "mcp",
    1,
    "~/.claude.json · mcpServers.context7",
    ["cc"],
    1.9,
    ["evals_studio", "agent-lab", "fangcun", "blog-site"],
    "most-projects",
    {
      src: "npx @upstash/context7-mcp",
      ver: "npx -y · 随 npm 最新",
      act: "keep",
      script: true,
      summary: "查询第三方库文档的 MCP 服务。6 个项目里有 4 个经常用到。",
      advice:
        "多数项目在用，留在全局合理。若想让 notes-kit 与 client-acme 更轻，可在这两个项目单独停用。",
      acts: [
        ["11-06", "在 4 个项目被调用"],
        ["07-14", "写入 ~/.claude.json"],
      ],
    },
  ) as Row,
  R(
    "claudemd",
    "CLAUDE.md",
    "instruction",
    1,
    "~/.claude/CLAUDE.md",
    ["cc"],
    0.9,
    [],
    "always-loaded",
    {
      src: "本机",
      ver: "内容哈希 7c01d2e",
      act: "keep",
      fulltext: true,
      summary:
        "用户级指令：语言偏好、提交规范与个人习惯。Claude Code 在每个项目的每次会话都全文加载，没有「触发」可统计。",
      advice: "只放每个项目都需要的约定。它的每一行都按项目缴税。",
      acts: [
        ["11-09", "内容变更 · 新增 3 行"],
        ["06-02", "首次发现"],
      ],
    },
  ) as Row,
  R(
    "agentsmd",
    "AGENTS.md",
    "instruction",
    1,
    "~/.codex/AGENTS.md",
    ["cx"],
    0.6,
    [],
    "always-loaded",
    {
      src: "本机",
      ver: "内容哈希 a9e4410",
      act: "keep",
      fulltext: true,
      summary:
        "Codex 的用户级指令，每个项目的每次会话都全文加载。与 ~/.claude/CLAUDE.md 有 70% 内容重复。",
      advice: "只放每个项目都需要的约定。它与 CLAUDE.md 大部分重复，后续版本可改为同一正本的镜像。",
      acts: [
        ["10-12", "内容变更"],
        ["06-02", "首次发现"],
      ],
    },
  ) as Row,
  SK("pdf", "pdf", ["cc", "ws"], 0.3, ["notes-kit"], "suggest-bundle", {
    src: ANTH,
    ver: MAIN,
    act: "bundle",
    script: true,
    summary: "读写 PDF 的 skill，带 4 个 Python 脚本。",
    advice: "只有 notes-kit 用到。可转成套装，或直接写进 notes-kit 的清单。",
    acts: [
      ["10-20", "在 notes-kit 被触发"],
      ["08-03", "装入全局层"],
    ],
  }) as Row,
  SK("pptx", "pptx", ["cc", "ws"], 0.3, [], "never-used", {
    src: ANTH,
    ver: MAIN,
    act: "remove",
    script: true,
    summary: "制作演示文稿的 skill，带脚本。",
    advice: NEVER,
    acts: [["08-03", "装入全局层"]],
  }) as Row,
  SK("docx", "docx", ["cc", "ws"], 0.3, [], "never-used", {
    src: ANTH,
    ver: MAIN,
    act: "remove",
    script: true,
    summary: "读写 Word 文档的 skill，带脚本。",
    advice: NEVER,
    acts: [["08-03", "装入全局层"]],
  }) as Row,
  SK("skillc", "skill-creator", ["cc"], 0.3, ["fangcun"], "suggest-bundle", {
    src: ANTH,
    ver: MAIN,
    act: "bundle",
    summary: "编写与评测 skill 的辅助流程。",
    advice: "只有 fangcun 用到。建议写进 fangcun 的项目清单，而不是放在全局。",
    acts: [
      ["11-12", "在 fangcun 被触发"],
      ["09-18", "装入全局层"],
    ],
  }) as Row,
  SK("prrev", "pr-review", ["cc", "cx", "ws"], 0.3, PROJS, "unknown-source", {
    src: "未收录 · 内容哈希 d4410e2",
    ver: "无版本记录",
    act: "adopt",
    summary: "每个项目都在用，但内容哈希与任何来源都不匹配，无法追踪版本与更新。",
    advice: "值得留在全局，建议先收录进资产库，记录来源与版本。",
    acts: [["11-11", "体检发现 · 未收录"]],
  }) as Row,
  SK("xlsx", "xlsx", ["cc", "ws"], 0.2, [], "never-used", {
    src: ANTH,
    ver: MAIN,
    act: "remove",
    script: true,
    summary: "读写表格的 skill，带脚本。",
    advice: NEVER,
    acts: [["08-03", "装入全局层"]],
  }) as Row,
  SK("mermaid", "mermaid", ["cc", "ws"], 0.2, [], "never-used", {
    src: "local:luyao-skills",
    ver: "开发模式 · 活链接",
    act: "remove",
    summary: "生成 Mermaid 图的约定。",
    advice: NEVER,
    acts: [["07-11", "装入全局层"]],
  }) as Row,
  SK("fed", "frontend-design", ["cc", "ws"], 0.2, [], "same-name-conflict", {
    src: "github:acme/skills",
    ver: "固定 v1.1.0 · 旧版",
    act: "remove",
    conflict: ["evals_studio", "blog-site"],
    summary:
      "全局层里的旧版 v1.1.0。evals_studio 与 blog-site 在项目层固定了同名的 v1.3.0，但 Claude Code 同名时全局优先，两个项目加载的仍是这一份。",
    advice:
      "方寸不会自动移走它：全局目录是所有项目共用的。从全局移除后，两个项目改用各自的 v1.3.0；其余 4 个项目从未触发，每个省下 0.2k 常驻。",
    acts: [
      ["11-12", "2 个项目在项目层升级到 v1.3.0，全局这份仍生效"],
      ["09-20", "v1.1.0 装入全局"],
    ],
  }) as Row,
  SK("wstyle", "writing-style", ["cc"], 0.2, ["fangcun", "blog-site", "notes-kit"], "common", {
    src: "local:luyao-skills",
    ver: "开发模式 · 活链接",
    act: "keep",
    summary: "中文写作风格约定。",
    advice: "一半项目在用。留在全局可以接受；若要更精准，可转成「写作」套装。",
    acts: [
      ["11-12", "在 blog-site 被触发"],
      ["06-20", "装入全局层"],
    ],
  }) as Row,
  SK(
    "web",
    "web-search",
    ["cc", "cx"],
    0.2,
    ["evals_studio", "agent-lab", "fangcun", "blog-site", "client-acme"],
    "common",
    {
      src: "github:obra/superpowers",
      ver: "跟随 main · 91c0b2d",
      act: "keep",
      summary: "联网检索的流程约定。",
      advice: "多数项目在用，留在全局合理。",
      acts: [
        ["11-13", "在 evals_studio 被触发"],
        ["06-20", "装入全局层"],
      ],
    },
  ) as Row,
  SK("gitc", "git-commit", ["cc", "cx", "ws"], 0.1, PROJS, "every-project", {
    src: "local:luyao-skills",
    ver: "开发模式 · 活链接",
    act: "keep",
    summary: "提交信息规范。三个工具都已分发，每个项目都用到。",
    advice: "这正是全局层该放的东西：每个项目都要，且很轻。",
    acts: [
      ["11-10", "在 6 个项目被触发"],
      ["05-30", "装入全局层"],
    ],
  }) as Row,
];

/** 从全局移除过的行（示例数据里应用瘦身计划后记着，撤销时放回）。 */
export const removedGlobal = new Set<string>();

/** 一个工具的全局页：只算这个工具加载的行，按常驻从高到低。 */
function overviewFor(t: T): GlobalOverview {
  const rows = ROWS.filter((r) => r.tools.includes(t) && !removedGlobal.has(r.id))
    .sort((a, b) => (b.resident ?? 0) - (a.resident ?? 0))
    .map(({ tools: _, ...row }) => row);
  const sum = (kind: GlobalRow["kind"]) =>
    rows.filter((r) => r.kind === kind).reduce((n, r) => n + (r.resident ?? 0), 0);
  const count = (kind: GlobalRow["kind"]) =>
    rows.filter((r) => r.kind === kind).reduce((n, r) => n + r.count, 0);
  const other: T = t === "cc" ? "cx" : "cc";
  const onlyOther = ROWS.filter(
    (r) => r.tools.includes(other) && !r.tools.includes(t) && !removedGlobal.has(r.id),
  ).length;
  return {
    tool: t === "cc" ? "claude-code" : "codex",
    budget: {
      limit: 20000,
      instructions: sum("instruction"),
      skills: sum("skill"),
      mcp: sum("mcp"),
      mcpUnestimated: 0,
    },
    projectTotal: 6,
    kinds: { skill: count("skill"), mcp: count("mcp"), instruction: count("instruction") },
    rarelyUsed: rows.filter((r) => r.status === "suggest-bundle").length,
    neverUsed: rows.filter((r) =>
      ["never-used", "overridden", "same-name-conflict"].includes(r.status),
    ).length,
    // 原型只在 Claude Code 一侧写「另有 2 项只被 Codex 加载」。
    otherTools:
      t === "cc" && onlyOther > 0 ? [{ tool: "codex", toolName: "Codex", count: onlyOther }] : [],
    rows,
  };
}

export const globalClaude = (): GlobalOverview => overviewFor("cc");
export const globalCodex = (): GlobalOverview => overviewFor("cx");

/** 全局行的 id 按成员名找回来：瘦身计划的示例数据用。 */
export function globalRowsNamed(names: string[]): GlobalRow[] {
  return ROWS.filter((r) => names.some((n) => r.members.includes(n) || r.name === n));
}
