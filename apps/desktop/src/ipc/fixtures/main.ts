/**
 * 项目页的示例数据。照抄 prototype/project/Main.dc.html 脚本段的 PROJ、R、STK、FILES、OTH。
 */
import type { Declaration } from "../bindings/Declaration";
import type { EffectiveContext } from "../bindings/EffectiveContext";
import type { EffectiveItem } from "../bindings/EffectiveItem";
import type { ProjectSummary } from "../bindings/ProjectSummary";
import type { Scope } from "../bindings/Scope";

/** 侧栏六个项目。Main.dc.html 的 PROJ；路径原型只给了 evals_studio 的，其余按同一目录写。 */
export const projects: ProjectSummary[] = [
  ["evals_studio", "drifted"],
  ["agent-lab", "synced"],
  ["fangcun", "updatable"],
  ["blog-site", "synced"],
  ["notes-kit", "synced"],
  ["client-acme", "synced"],
].map(([name, state]) => ({
  id: `/Users/me/code/${name}`,
  name: name as string,
  path: `~/code/${name}`,
  state: state as ProjectSummary["state"],
  lastSeen: null,
  sessions: 0,
  tools: ["claude-code", "codex"],
}));

const SCOPES: Scope[] = ["global", "bundle", "project", "personal"];

/** STK：四层各一句，「生效」那层标绿。 */
function stack(values: [string, string, string, string]): Declaration[] {
  return SCOPES.map((scope, i) => ({
    scope,
    location: values[i] || null,
    effective: values[i]?.startsWith("生效") ?? false,
  }));
}

function item(
  x: Partial<EffectiveItem> & Pick<EffectiveItem, "id" | "name" | "kind" | "scope" | "label">,
): EffectiveItem {
  return {
    version: null,
    bundle: null,
    state: null,
    resident: null,
    counted: true,
    location: "",
    linkTarget: null,
    source: null,
    hasScripts: false,
    members: [],
    command: null,
    declared: [],
    reason: "",
    triggers: null,
    files: [],
    overriddenBy: null,
    ...x,
  };
}

const R = {
  agents: item({
    id: "agents",
    name: "AGENTS.md",
    kind: "instruction",
    scope: "project",
    label: "正本",
    resident: 2400,
    location: "AGENTS.md",
    declared: stack(["", "", "生效 · 正本", ""]),
    files: ["AGENTS.md"],
    reason: "项目清单把 AGENTS.md 指定为正本，所以它对本项目生效。\n其他指令文件都从它生成。",
  }),
  claude: item({
    id: "claude",
    name: "CLAUDE.md",
    kind: "instruction",
    scope: "project",
    label: "镜像自 AGENTS.md",
    resident: 100,
    location: "CLAUDE.md",
    declared: stack(["", "", "生效 · 生成的镜像", ""]),
    files: ["CLAUDE.md · 内容为 @AGENTS.md"],
    reason:
      "这份文件由方寸自动生成，内容只有一行对 AGENTS.md 的引用。\n要改指令请编辑 AGENTS.md，不要直接改它。",
  }),
  harness: item({
    id: "harness",
    name: "project-harness",
    kind: "skill",
    scope: "project",
    version: "7月版 · 4d9b1eb",
    state: "drifted",
    label: "已漂移",
    resident: 400,
    source: "local:luyao-skills",
    location: ".agents/skills/project-harness",
    declared: stack(["", "", "生效 · 固定 4d9b1eb", ""]),
    files: [
      ".agents/skills/project-harness → 资产库",
      ".claude/skills/project-harness → .agents/skills/project-harness",
    ],
    reason:
      "项目清单固定使用 4d9b1eb 版本。\n但项目里的这份副本被改动过，和锁定的版本对不上，所以标为漂移。",
  }),
  tdd: item({
    id: "tdd",
    name: "tdd",
    kind: "skill",
    scope: "bundle",
    bundle: "工程基线",
    version: "a1b2c3",
    state: "updatable",
    label: "可更新 ↑ b7e9f0",
    resident: 200,
    source: "github:obra/superpowers",
    hasScripts: true,
    location: ".agents/skills/tdd",
    declared: stack(["", "生效 · 来自「工程基线」", "", ""]),
    files: [".agents/skills/tdd → 资产库", ".claude/skills/tdd → .agents/skills/tdd"],
    reason:
      "它来自本项目启用的套装「工程基线」。\n套装设定跟随上游 main 分支。上游出了新版本 b7e9f0，你确认后才会更新。",
  }),
  fedp: item({
    id: "fedp",
    name: "frontend-design",
    kind: "skill",
    scope: "project",
    version: "v1.3.0",
    label: "覆盖全局同名项",
    resident: 200,
    source: "github:acme/skills",
    location: ".agents/skills/frontend-design",
    declared: stack(["同名 v1.1.0 · 被遮蔽", "", "生效 · v1.3.0", ""]),
    files: [
      ".agents/skills/frontend-design → 资产库",
      ".claude/skills/frontend-design → .agents/skills/frontend-design",
    ],
    reason:
      "项目清单里声明了它。\n全局也装了一个同名 skill，同名时以项目里的为准，所以生效的是这一份。",
  }),
  lark: item({
    id: "lark",
    name: "lark-doc …等 16 项",
    kind: "skill",
    scope: "global",
    label: "本项目从未使用",
    resident: 6400,
    location: "~/.claude/skills",
    source: "~/.agents/skills",
    triggers: 0,
    members: ["lark-doc"],
    declared: stack(["生效 · 16 项", "", "", ""]),
    files: ["~/.claude/skills/lark-*（16 个链接，全局）"],
    reason:
      "它们装在全局目录里，而全局的东西会加载进每一个项目。\n本项目没有用到它们，近 30 天也从未触发，却一直占着上下文。",
  }),
  prrev: item({
    id: "prrev",
    name: "pr-review",
    kind: "skill",
    scope: "global",
    state: "unknown-source",
    label: "来源未知",
    resident: 300,
    location: "~/.agents/skills/pr-review",
    declared: stack(["生效 · 来源未知", "", "", ""]),
    files: ["~/.claude/skills/pr-review → ~/.agents/skills/pr-review"],
    reason:
      "它装在全局目录里，所以对本项目也生效。\n方寸查不到它从哪来，无法判断版本，也没法提醒更新。",
  }),
  fedg: item({
    id: "fedg",
    name: "frontend-design",
    kind: "skill",
    scope: "global",
    version: "v1.1.0",
    state: "overridden",
    label: "被项目同名项覆盖",
    resident: 0,
    counted: false,
    location: "~/.claude/skills",
    declared: stack(["v1.1.0 · 被遮蔽", "", "生效 · 同名 v1.3.0", ""]),
    files: ["~/.claude/skills/frontend-design（全局，未加载）"],
    reason:
      "它没有生效。\n本项目里有一个同名 skill，同名时以项目里的为准，这份全局版本被挡住，不会加载。",
  }),
  github: item({
    id: "github",
    name: "github",
    kind: "mcp",
    scope: "project",
    version: "server-github",
    label: "密钥在钥匙串",
    resident: 4800,
    command: "npx -y @modelcontextprotocol/server-github",
    location: ".mcp.json · mcpServers.github",
    declared: stack(["", "", "生效 · 密钥在钥匙串", ""]),
    files: [".mcp.json → mcpServers.github（按键合并）", "Codex 配置 → github（按键合并）"],
    reason:
      "项目清单里声明了这个 MCP 服务，所以它对本项目生效。\nGitHub 令牌存在钥匙串里，配置文件只写引用，不写明文。",
  }),
};

/** Claude Code 下的生效上下文。预算 14.8k：指令 2.5k、Skill 7.5k、MCP 4.8k。 */
export const effectiveClaude: EffectiveContext = {
  tool: "claude-code",
  projectName: "evals_studio",
  projectPath: "~/code/evals_studio",
  hasManifest: true,
  budget: { limit: 20000, instructions: 2500, skills: 7500, mcp: 4800, mcpUnestimated: 0 },
  items: [R.agents, R.claude, R.harness, R.tdd, R.fedp, R.lark, R.prrev, R.fedg, R.github],
  observed: [
    {
      kind: "plugin",
      name: "code-review-kit",
      scope: "global",
      note: "含 2 个斜杠命令",
      location: "",
    },
    { kind: "plugin", name: "notify", scope: "global", note: "含 1 个 Hook", location: "" },
    {
      kind: "hook",
      name: "PostToolUse · 格式化代码",
      scope: "project",
      note: "会执行脚本",
      location: "",
    },
    { kind: "slash-command", name: "/release", scope: "project", note: "按需加载", location: "" },
  ],
  observedMissing: ["subagent"],
};

/** Codex 下的生效上下文：不读 CLAUDE.md，也没有全局的飞书 skill。 */
export const effectiveCodex: EffectiveContext = {
  ...effectiveClaude,
  tool: "codex",
  budget: { limit: 20000, instructions: 2400, skills: 800, mcp: 4800, mcpUnestimated: 0 },
  items: [R.agents, R.harness, R.tdd, R.fedp, R.github],
  observed: [],
  observedMissing: ["plugin", "hook"],
};
