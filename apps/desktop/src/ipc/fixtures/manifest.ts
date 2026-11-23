/**
 * 项目页「清单」页签的示例数据。照抄 prototype/project/Main.dc.html 脚本段的 MAN、MF。
 * 原型的 RW(a, b, c, note, o)：a、b 默认等宽，o.am / o.bm = false 时不等宽，o.cm 时 c 等宽，
 * o.ct 是 c 的颜色（只用到橙色）。
 */
import type { ManifestRow } from "../bindings/ManifestRow";
import type { ManifestSection } from "../bindings/ManifestSection";
import type { ManifestView } from "../bindings/ManifestView";

function RW(
  a: string,
  b: string,
  c: string,
  note = "",
  o: { am?: boolean; bm?: boolean; cm?: boolean; warn?: boolean } = {},
): ManifestRow {
  return {
    cells: [a, b, c],
    mono: [o.am !== false, o.bm !== false, o.cm === true],
    note: note || null,
    warn: o.warn === true,
  };
}

function SEC(
  title: string,
  h1: string,
  h2: string,
  h3: string,
  rows: ManifestRow[],
): ManifestSection {
  return { title, headers: [h1, h2, h3], rows };
}

const MAN = [
  "# fangcun.toml —— 提交到 git",
  "[project]",
  'targets = ["claude-code", "codex"]',
  'mode    = "link"                 # link 或 vendor',
  "",
  'bundles = ["工程基线"]',
  "",
  "[skills]",
  'project-harness = { source = "local:luyao-skills", pin = "4d9b1eb" }',
  'frontend-design = { source = "github:acme/skills", pin = "v1.3.0" }',
  "",
  "[mcp.github]",
  'command = "npx"',
  'args    = ["-y", "@modelcontextprotocol/server-github"]',
  'env     = { GITHUB_TOKEN = "secret:github-token" }   # 值在钥匙串',
  "",
  "[instructions]",
  'canonical = "AGENTS.md"          # 正本',
  'mirror    = ["CLAUDE.md"]        # 生成为对正本的引用',
];

/** 原型默认状态：清单里还没有 [disable]。 */
export function manifestView(): ManifestView {
  const skills = [
    RW("project-harness", "local:luyao-skills", "固定 4d9b1eb"),
    RW("frontend-design", "github:acme/skills", "固定 v1.3.0"),
    RW("tdd", "github:obra/superpowers", "跟随 main 分支", "来自套装「工程基线」"),
  ];
  return {
    files: [
      {
        name: "fangcun.toml",
        role: "项目清单",
        tag: "提交到 git",
        committed: true,
        exists: true,
        description:
          "项目清单。同事克隆仓库后运行 fangcun sync，就能还原完全相同的环境。界面上的改动最终都写回这个文件。",
        sections: [
          SEC("项目设置", "设置", "值", "含义", [
            RW("目标工具", "claude-code, codex", "两个工具都按本清单同步", "", { am: false }),
            RW(
              "同步方式",
              "link",
              "本机软链，指向资产库正本",
              "团队项目可改为 vendor（拷入仓库），实体文件随 Git 提交",
              { am: false },
            ),
            RW("套装", "工程基线", "套装里的资产一并生效", "", { am: false, bm: false }),
          ]),
          SEC("Skills", "名称", "来源", "版本策略", skills),
          SEC("MCP 服务", "名称", "启动命令", "密钥", [
            RW(
              "github",
              "npx -y @modelcontextprotocol/server-github",
              "GITHUB_TOKEN 存在钥匙串",
              "文件里只写引用 secret:github-token",
            ),
          ]),
          SEC("指令文件", "角色", "文件", "说明", [
            RW("正本", "AGENTS.md", "手工编辑这一份", "", { am: false }),
            RW("镜像", "CLAUDE.md", "自动生成，内容是对 AGENTS.md 的引用", "", { am: false }),
          ]),
        ],
        lines: MAN,
      },
      {
        name: "fangcun.lock",
        role: "版本锁",
        tag: "提交 · 自动生成",
        committed: true,
        exists: true,
        description:
          "由方寸根据清单解析生成，记录每项资产确切的提交与内容哈希。每次同步都会校验哈希，来源被篡改会被拦下。不要手工编辑。",
        sections: [
          SEC("已解析的版本", "资产", "解析到", "内容哈希", [
            RW("project-harness", "4d9b1eb", "sha256:4d9b…7c21", "", { cm: true }),
            RW("frontend-design", "v1.3.0 → c03d1f2", "sha256:c03d…a9e0", "", { cm: true }),
            RW("tdd", "main → a1b2c3", "sha256:a1b2…3f08", "上游已有 b7e9f0，尚未升级", {
              cm: true,
            }),
            RW("github", "npx 启动命令", "sha256:7a1e…0b44", "启动命令一变，升级时会高亮提示", {
              cm: true,
              bm: false,
            }),
          ]),
        ],
        lines: [
          "# fangcun.lock —— 由方寸生成，请勿手工编辑",
          "version = 1",
          "",
          "[[asset]]",
          'name   = "project-harness"',
          'source = "local:luyao-skills"',
          'commit = "4d9b1eb"',
          'hash   = "sha256:4d9b…7c21"',
          "",
          "[[asset]]",
          'name   = "frontend-design"',
          'source = "github:acme/skills"',
          'tag    = "v1.3.0"',
          'commit = "c03d1f2"',
          'hash   = "sha256:c03d…a9e0"',
          "",
          "[[asset]]",
          'name   = "tdd"',
          'source = "github:obra/superpowers"',
          'track  = "main"',
          'commit = "a1b2c3"',
          'hash   = "sha256:a1b2…3f08"',
          "",
          "[[mcp]]",
          'name    = "github"',
          'command = "npx -y @modelcontextprotocol/server-github"',
          'hash    = "sha256:7a1e…0b44"',
        ],
      },
      {
        name: "fangcun.local.toml",
        role: "个人覆盖",
        tag: "不提交",
        committed: false,
        exists: true,
        description:
          "只属于你的覆盖，已自动加入 .gitignore。它优先于项目清单，但只在你的机器上生效，同事看不到。",
        sections: [
          SEC("个人覆盖", "项", "值", "效果", [
            RW(
              "追加 skill",
              "debug-notes · local:~/my-skills",
              "开发模式，改完立即可见",
              "只在你的机器上加载",
              { am: false },
            ),
            RW("停用", "pr-review", "你本人不加载这个全局 skill", "", { am: false }),
          ]),
        ],
        lines: [
          "# fangcun.local.toml —— 不提交，只属于你",
          "[skills]",
          'debug-notes = { source = "local:~/my-skills", dev = true }',
          "",
          "[disable]",
          'global = ["pr-review"]',
        ],
      },
    ],
  };
}
