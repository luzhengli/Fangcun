/**
 * 体检报告的示例数据。照抄 prototype/project/Checkup.dc.html 脚本段的 F。
 */
import type { CheckupReport } from "../bindings/CheckupReport";
import type { FindingLocation } from "../bindings/FindingLocation";
import type { LocationTone } from "../bindings/LocationTone";

const L = (
  path: string,
  detail: string,
  note: string,
  tone: LocationTone | null = null,
): FindingLocation => ({
  path,
  detail,
  note,
  tone,
});

export const checkup: CheckupReport = {
  scannedAt: "2026-11-11T01:12:00Z",
  toolsScanned: 7,
  projectDirsScanned: 61,
  sessionsRead: 0,
  assetsFound: 38,
  duplicateCopies: 4,
  unknownSource: 2,
  findings: [
    {
      id: "dup",
      category: "duplicate",
      status: "pending",
      title: "project-harness 有 5 份副本，1 份已分叉",
      action: "收编到资产库",
      intro: "按内容哈希归并。4 份相同的副本收为同一版本，分叉的那份另存为独立版本，等你决定去留。",
      locations: [
        L("agent-lab/.claude/skills/project-harness", "4d9b1eb", "相同"),
        L("fangcun/.claude/skills/project-harness", "4d9b1eb", "相同"),
        L("blog-site/.agents/skills/project-harness", "4d9b1eb", "相同"),
        L("notes-kit/.claude/skills/project-harness", "4d9b1eb", "相同"),
        L("evals_studio/.trae/project-harness", "e81c55a", "已分叉", "warn"),
      ],
      foot: "原位置替换为受管链接，事前自动备份。",
    },
    {
      id: "repos",
      category: "duplicate",
      status: "pending",
      title: "4 个仓库存放着相同的 skill",
      action: "以 luyao-skills 为正本",
      intro: "同一批 skill 出现在 4 个仓库里。指定一个为正本，其余登记为镜像，不删除任何仓库。",
      locations: [
        L("~/code/luyao-skills", "最早提交", "建议正本", "info"),
        L("~/code/my-skills", "镜像", "内容相同"),
        L("~/code/agent-skill", "镜像", "内容相同"),
        L("~/code/harness", "镜像", "内容相同"),
      ],
      foot: "只写入方寸的资产库索引。",
    },
    {
      id: "tax",
      category: "global-tax",
      status: "pending",
      title: "16 个飞书 skill 加载进了每一个项目",
      action: "创建套装「飞书办公」",
      intro: "把这 16 项从全局层移到新套装，再勾选需要它的项目。其他项目的常驻上下文随之减少。",
      locations: [
        L("lark-doc · lark-sheet · lark-base …", "16 项", "常驻约 6.4k", "muted"),
        L("需要它的项目", "已勾选 1 个", "client-acme"),
      ],
      foot: "估算值。全局链接移除前自动快照。",
    },
    {
      id: "spread",
      category: "inconsistent",
      status: "pending",
      title: "31 个全局 skill 在各工具间分发不齐",
      action: "标记为已知",
      intro: "~/.agents/skills 中有 31 个 skill，各工具实际链接的数量不同。",
      locations: [
        L("Claude Code", "16 / 31", "缺 15"),
        L("Windsurf", "20 / 31", "缺 11"),
        L("Codex", "3 / 31", "缺 28"),
      ],
      foot: "推荐做法：把全局层瘦身后再统一分发。",
    },
    {
      id: "lock",
      category: "detached",
      status: "pending",
      title: "全局锁文件与实际目录不一致",
      action: "按目录重建锁",
      intro: "锁文件最后修改于 7 月 5 日，skills 目录 11 月 11 日还在变动。",
      locations: [
        L("全局 skills 锁文件", "07-05", "过期", "warn"),
        L("~/.agents/skills/", "11-11", "实际状态"),
      ],
      foot: "旧锁备份到活动，可回滚。",
    },
    {
      id: "secret",
      category: "secret",
      status: "not-scanned",
      title: "检查已提交文件中的明文密钥",
      action: "扫描 61 个仓库",
      intro: "在已提交的 MCP 配置与环境文件中查找明文 API Key。只读，不上传任何内容。",
      locations: [L(".mcp.json · .env · config.toml", "61 个仓库", "待扫描")],
      foot: "发现后可一键迁移到钥匙串。",
    },
  ],
};
