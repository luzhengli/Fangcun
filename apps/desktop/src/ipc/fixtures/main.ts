/**
 * 项目页的示例数据。照抄 prototype/project/Main.dc.html 脚本段的 PROJ、R、STK、FILES、OTH。
 */
import type { Declaration } from "../bindings/Declaration";
import type { EffectiveContext } from "../bindings/EffectiveContext";
import type { EffectiveItem } from "../bindings/EffectiveItem";
import type { ItemActivity } from "../bindings/ItemActivity";
import type { ProjectSummary } from "../bindings/ProjectSummary";
import type { RecordKind } from "../bindings/RecordKind";
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
  // 原型侧栏这一组叫「已登记项目」，六个都算已登记。
  registered: true,
}));

const SCOPES: Scope[] = ["global", "bundle", "project", "personal"];

/**
 * 检查器「最近活动」的一行。Main.dc.html 的 ACT：「11-10」写成那天的本地时间，「5 分钟前」按现在算。
 * 记录编号能对上活动页示例数据（Activity.dc.html 的 EV）的用那条，对不上的活动页选中第一条。
 */
function act(
  when: string,
  text: string,
  kind: RecordKind,
  record = `main-${when}-${text}`,
): ItemActivity {
  const at =
    when === "5 分钟前"
      ? new Date(Date.now() - 5 * 60_000).toISOString()
      : new Date(`2026-${when}T12:00:00`).toISOString();
  return { record, at, kind, text };
}

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
    secrets: [],
    recent: [],
    drift: null,
    update: null,
    overriddenBy: null,
    disable: null,
    ...x,
  };
}

const R = {
  agents: item({
    id: "agents",
    recent: [
      act("11-13", "同步 · 内容未变", "sync", "e127"),
      act("11-01", "正本更新到 5e2a7c1", "sync"),
    ],
    name: "AGENTS.md",
    kind: "instruction",
    scope: "project",
    label: "正本",
    resident: 2400,
    location: "AGENTS.md",
    declared: stack(["", "", "生效 · 正本", ""]),
    files: ["AGENTS.md"],
    reason: "项目清单把 AGENTS.md 指定为正本，所以它对本项目生效。其他指令文件都从它生成。",
  }),
  claude: item({
    id: "claude",
    recent: [act("11-10", "由方寸生成镜像", "sync")],
    name: "CLAUDE.md",
    kind: "instruction",
    scope: "project",
    label: "镜像自 AGENTS.md",
    resident: 100,
    location: "CLAUDE.md",
    declared: stack(["", "", "生效 · 生成的镜像", ""]),
    files: ["CLAUDE.md · 内容为 @AGENTS.md"],
    reason: "这份文件由方寸自动生成，内容只有一行对 AGENTS.md 的引用。要改指令请编辑 AGENTS.md。",
  }),
  harness: item({
    id: "harness",
    // Main.dc.html 漂移块：SKILL.md 第 14 行的差异、回流时列出的 4 个项目。
    drift: {
      locked: "4d9b1eb",
      summary: "Agent 在本项目修改了 SKILL.md，相对 4d9b1eb 多 3 行、少 1 行。",
      files: [
        {
          path: "SKILL.md",
          op: "modify",
          added: 3,
          removed: 1,
          script: false,
          binary: false,
          truncated: 0,
          lines: [
            { kind: "hunk", text: "@@ SKILL.md · 第 14 行 @@" },
            { kind: "context", text: "## 收尾检查" },
            { kind: "remove", text: "运行全部测试后再提交。" },
            { kind: "add", text: "先运行受影响的测试，再运行全部测试。" },
            { kind: "add", text: "提交前检查 fangcun status 无本地未写回修改。" },
            { kind: "add", text: "新增依赖时同步更新 fangcun.toml。" },
          ],
        },
      ],
      shared: true,
      original: true,
      promoteBlocked: null,
      others: ["agent-lab", "fangcun", "blog-site", "notes-kit"],
    },
    recent: [
      act("5 分钟前", "检测到本地修改 · 多 3 行、少 1 行", "drift", "drift1"),
      act("11-11", "收录进资产库", "adopt"),
      act("07-02", "固定到 4d9b1eb", "sync"),
    ],
    name: "project-harness",
    kind: "skill",
    scope: "project",
    version: "7月版 · 4d9b1eb",
    state: "drifted",
    label: "本地已改",
    resident: 400,
    source: "local:luyao-skills",
    location: ".agents/skills/project-harness",
    declared: stack(["", "", "生效 · 固定 4d9b1eb", ""]),
    files: [
      ".agents/skills/project-harness → 资产库",
      ".claude/skills/project-harness → .agents/skills/project-harness",
    ],
    reason:
      "项目清单固定使用 4d9b1eb。项目里的这份副本被改过，和锁定的版本对不上，所以标为「本地已改」。",
  }),
  tdd: item({
    id: "tdd",
    // Main.dc.html 可更新块：SKILL.md 描述改写、新增脚本，另两个项目也在 a1b2c3。
    update: {
      from: "a1b2c3",
      to: "b7e9f0",
      scripts: true,
      files: [
        {
          path: "SKILL.md",
          op: "modify",
          added: 3,
          removed: 1,
          script: false,
          binary: false,
          truncated: 0,
          lines: [],
        },
        {
          path: "scripts/run-tests.sh",
          op: "add",
          added: 22,
          removed: 0,
          script: true,
          binary: false,
          truncated: 0,
          lines: [],
        },
      ],
      others: [
        { project: "/Users/me/code/agent-lab", name: "agent-lab", note: "跟随 main · 本机软链" },
        { project: "/Users/me/code/fangcun", name: "fangcun", note: "跟随 main · 拷入仓库" },
      ],
    },
    recent: [
      act("11-12", "上游新提交 b7e9f0", "updatable"),
      act("10-28", "升级到 a1b2c3", "upgrade"),
    ],
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
      "它来自本项目启用的套装「工程基线」，套装设定跟随上游 main。上游出了新版本 b7e9f0，你确认后才会更新。",
  }),
  fedp: item({
    id: "fedp",
    recent: [
      act("11-12", "升级 v1.1.0 → v1.3.0", "upgrade"),
      act("11-12", "全局层有同名 v1.1.0，Claude Code 仍加载全局那一份", "sync", "g1"),
    ],
    name: "frontend-design",
    kind: "skill",
    scope: "project",
    version: "v1.3.0",
    state: "overridden",
    label: "被全局层同名项覆盖",
    resident: 0,
    counted: false,
    source: "github:acme/skills",
    location: ".agents/skills/frontend-design",
    declared: stack(["生效 · 同名 v1.1.0", "", "v1.3.0 · 被遮蔽", ""]),
    files: [
      ".agents/skills/frontend-design → 资产库",
      ".claude/skills/frontend-design → .agents/skills/frontend-design（被遮蔽，未加载）",
    ],
    reason: "项目清单声明了它，但它没有生效：全局也装了同名 skill，Claude Code 同名时全局优先。",
    overriddenBy: "fedg",
  }),
  lark: item({
    id: "lark",
    recent: [act("07-05", "装入全局", "sync")],
    name: "lark-doc 等 16 项",
    kind: "skill",
    scope: "global",
    label: "本项目从未使用",
    resident: 6400,
    location: "~/.claude/skills",
    source: "~/.agents/skills",
    triggers: 0,
    members: ["lark-doc"],
    // 原型的「在本项目停用」：清单写 7.4 示例里的 lark-*。
    disable: { names: ["lark-*"], active: false, idle: true, unsupported: null, note: null },
    declared: stack(["生效 · 16 项", "", "", ""]),
    files: ["~/.claude/skills/lark-*（16 个链接，全局）"],
    reason:
      "它们装在全局目录里，全局的东西会加载进每一个项目。本项目近 30 天从未触发，却一直占着上下文。",
  }),
  prrev: item({
    id: "prrev",
    recent: [act("11-11", "体检发现 · 来源未知", "checkup", "chk1")],
    name: "pr-review",
    kind: "skill",
    scope: "global",
    state: "unknown-source",
    label: "未收录",
    resident: 300,
    location: "~/.agents/skills/pr-review",
    declared: stack(["生效 · 未收录", "", "", ""]),
    files: ["~/.claude/skills/pr-review → ~/.agents/skills/pr-review"],
    reason:
      "它装在全局目录里，所以对本项目也生效。方寸查不到它从哪来，无法判断版本，也没法提醒更新。",
  }),
  fedg: item({
    id: "fedg",
    recent: [
      act("11-12", "项目层同名项升级到 v1.3.0，这一份仍生效", "upgrade"),
      act("09-20", "装入全局", "sync"),
    ],
    name: "frontend-design",
    kind: "skill",
    scope: "global",
    version: "v1.1.0",
    label: "覆盖项目同名项",
    resident: 200,
    location: "~/.claude/skills",
    declared: stack(["生效 · v1.1.0", "", "同名 v1.3.0 · 被遮蔽", ""]),
    files: ["~/.claude/skills/frontend-design（全局，生效）"],
    reason:
      "它装在全局目录里，对本项目生效。本项目也声明了同名的 v1.3.0，但 Claude Code 同名时全局优先。",
  }),
  github: item({
    id: "github",
    secrets: ["GITHUB_TOKEN → 钥匙串"],
    recent: [
      act("11-10", "密钥迁入钥匙串", "secret", "k1"),
      act("11-10", "写入 .mcp.json", "sync"),
    ],
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
    reason: "项目清单声明了这个 MCP 服务。GitHub 令牌存在钥匙串里，配置文件只写引用，不写明文。",
  }),
};

/** Codex 不读 Claude Code 的全局目录，没有同名项，项目层的 frontend-design 照常生效。 */
const fedpCodex = item({
  id: "fedp",
  recent: [act("11-12", "升级 v1.1.0 → v1.3.0", "upgrade")],
  name: "frontend-design",
  kind: "skill",
  scope: "project",
  version: "v1.3.0",
  label: "固定 v1.3.0",
  resident: 200,
  source: "github:acme/skills",
  location: ".agents/skills/frontend-design",
  declared: stack(["", "", "生效 · v1.3.0", ""]),
  files: [
    ".agents/skills/frontend-design → 资产库",
    ".claude/skills/frontend-design → .agents/skills/frontend-design",
  ],
  reason:
    "项目清单里声明了它，所以它对本项目生效。\nCodex 不读 Claude Code 的全局目录，这里没有同名项。",
});

/** Claude Code 下的生效上下文。预算 14.8k：指令 2.5k、Skill 7.5k、MCP 4.8k。 */
export const effectiveClaude: EffectiveContext = {
  tool: "claude-code",
  projectName: "evals_studio",
  projectPath: "~/code/evals_studio",
  hasManifest: true,
  mode: "link",
  targets: ["claude-code", "codex"],
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

/** Codex 下的生效上下文：不读 CLAUDE.md，也没有全局的飞书 skill 和 frontend-design。 */
export const effectiveCodex: EffectiveContext = {
  ...effectiveClaude,
  tool: "codex",
  budget: { limit: 20000, instructions: 2400, skills: 800, mcp: 4800, mcpUnestimated: 0 },
  items: [R.agents, R.harness, R.tdd, fedpCodex, R.github],
  observed: [],
  observedMissing: ["plugin", "hook"],
};

/** 近 30 天的项目里没登记的那个。Main.dc.html 的 RECENT。 */
export const recentProject: ProjectSummary = {
  id: "/Users/me/code/luyao-skills",
  name: "luyao-skills",
  path: "~/code/luyao-skills",
  state: null,
  lastSeen: null,
  sessions: 0,
  tools: ["claude-code"],
  registered: false,
};

/**
 * luyao-skills 的生效上下文：原型 isRecent 分支，只有全局层的三项，项目层没有发现资产。
 * frontend-design 在这里没有同名项，状态改成「全局生效」。
 */
export function effectiveRecent(tool: EffectiveContext["tool"]): EffectiveContext {
  const base = {
    tool,
    projectName: recentProject.name,
    projectPath: recentProject.path,
    hasManifest: false,
    mode: null,
    targets: [],
  };
  if (tool !== "claude-code") {
    return {
      ...base,
      budget: { limit: 20000, instructions: 0, skills: 0, mcp: 0, mcpUnestimated: 0 },
      items: [],
      observed: [],
      observedMissing: ["plugin", "hook"],
    };
  }
  const fedg: EffectiveItem = {
    ...R.fedg,
    label: "全局生效",
    declared: stack(["生效 · v1.1.0", "", "", ""]),
    reason: "它装在全局目录里，所以对本项目也生效。\n本项目没有同名项。",
  };
  return {
    ...base,
    budget: { limit: 20000, instructions: 0, skills: 6900, mcp: 0, mcpUnestimated: 0 },
    items: [R.lark, R.prrev, fedg],
    observed: effectiveClaude.observed.filter((o) => o.scope === "global"),
    observedMissing: ["subagent"],
  };
}
