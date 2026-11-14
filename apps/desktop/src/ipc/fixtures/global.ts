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
  act: "bundle" | "remove" | "keep" | "adopt";
}

const ACTIONS: Record<X["act"], string[]> = {
  bundle: ["转成套装…", "从全局移除…"],
  remove: ["从全局移除…"],
  keep: [],
  adopt: ["去资产库收编"],
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
        ? "已链接"
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
    usedBy: used.length,
    status,
    overridden: shadowed.length,
    conflicts: 0,
    hasScripts: x.script ?? false,
    command: null,
    usage: PROJS.map((p) => ({
      name: p,
      path: `~/code/${p}`,
      mark: shadowed.includes(p) ? "overridden" : used.includes(p) ? "used" : "unused",
      triggers: used.includes(p) ? 1 : 0,
    })),
    distribution,
    summary: x.summary,
    advice: x.advice,
    actions: ACTIONS[x.act],
  };
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

export const globalClaude: GlobalOverview = {
  tool: "claude-code",
  budget: { limit: 20000, instructions: 900, skills: 9000, mcp: 1900, mcpUnestimated: 0 },
  projectTotal: 6,
  kinds: { skill: 27, mcp: 1, instruction: 1 },
  rarelyUsed: 18,
  neverUsed: 5,
  otherTools: [{ tool: "codex", toolName: "Codex", count: 2 }],
  rows: [
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
        advice: "只有 client-acme 用到。转成套装后，其余 5 个项目每个省下 6.4k 常驻。",
      },
    ),
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
      },
    ),
    R(
      "claudemd",
      "CLAUDE.md",
      "instruction",
      1,
      "~/.claude/CLAUDE.md",
      ["cc"],
      0.9,
      PROJS,
      "every-project",
      {
        src: "本机",
        ver: "内容哈希 7c01d2e",
        act: "keep",
        summary: "用户级指令：语言偏好、提交规范与个人习惯。Claude Code 在每个项目都会全文加载。",
        advice: "每个项目都需要，适合留在全局。注意保持精简，它的每一行都按项目缴税。",
      },
    ),
    SK("pdf", "pdf", ["cc", "ws"], 0.3, ["notes-kit"], "suggest-bundle", {
      src: ANTH,
      ver: MAIN,
      act: "bundle",
      script: true,
      summary: "读写 PDF 的 skill，带 4 个 Python 脚本。",
      advice: "只有 notes-kit 用到。可转成套装，或直接写进 notes-kit 的清单。",
    }),
    SK("pptx", "pptx", ["cc", "ws"], 0.3, [], "never-used", {
      src: ANTH,
      ver: MAIN,
      act: "remove",
      script: true,
      summary: "制作演示文稿的 skill，带脚本。",
      advice: NEVER,
    }),
    SK("docx", "docx", ["cc", "ws"], 0.3, [], "never-used", {
      src: ANTH,
      ver: MAIN,
      act: "remove",
      script: true,
      summary: "读写 Word 文档的 skill，带脚本。",
      advice: NEVER,
    }),
    SK("skillc", "skill-creator", ["cc"], 0.3, ["fangcun"], "suggest-bundle", {
      src: ANTH,
      ver: MAIN,
      act: "bundle",
      summary: "编写与评测 skill 的辅助流程。",
      advice: "只有 fangcun 用到。建议写进 fangcun 的项目清单，而不是放在全局。",
    }),
    SK("prrev", "pr-review", ["cc", "cx", "ws"], 0.3, PROJS, "unknown-source", {
      src: "未知 · 内容哈希 d4410e2",
      ver: "无版本记录",
      act: "adopt",
      summary: "每个项目都在用，但内容哈希与任何来源都不匹配，无法追踪版本与更新。",
      advice: "值得留在全局，但应先收编进资产库，记录来源与版本。",
    }),
    SK("xlsx", "xlsx", ["cc", "ws"], 0.2, [], "never-used", {
      src: ANTH,
      ver: MAIN,
      act: "remove",
      script: true,
      summary: "读写表格的 skill，带脚本。",
      advice: NEVER,
    }),
    SK("mermaid", "mermaid", ["cc", "ws"], 0.2, [], "never-used", {
      src: "local:luyao-skills",
      ver: "开发模式 · 活链接",
      act: "remove",
      summary: "生成 Mermaid 图的约定。",
      advice: NEVER,
    }),
    SK("fed", "frontend-design", ["cc", "ws"], 0.2, [], "overridden", {
      src: "github:acme/skills",
      ver: "固定 v1.1.0 · 旧版",
      act: "remove",
      shadowed: ["evals_studio", "blog-site"],
      summary:
        "全局层里的旧版 v1.1.0。evals_studio 与 blog-site 在项目层固定了 v1.3.0，把它遮蔽了。",
      advice:
        "被遮蔽的两个项目用不到它，其余 4 个项目也从未触发。可以从全局移除，项目层的 v1.3.0 不受影响。",
    }),
    SK("wstyle", "writing-style", ["cc"], 0.2, ["fangcun", "blog-site", "notes-kit"], "common", {
      src: "local:luyao-skills",
      ver: "开发模式 · 活链接",
      act: "keep",
      summary: "中文写作风格约定。",
      advice: "一半项目在用。留在全局可以接受；若要更精准，可转成「写作」套装。",
    }),
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
      },
    ),
    SK("gitc", "git-commit", ["cc", "cx", "ws"], 0.1, PROJS, "every-project", {
      src: "local:luyao-skills",
      ver: "开发模式 · 活链接",
      act: "keep",
      summary: "提交信息规范。三个工具都已分发，每个项目都用到。",
      advice: "这正是全局层该放的东西：每个项目都要，且很轻。",
    }),
  ],
};
