# Codex

以本机版本为准，下面是 codex-cli 0.167 上看到的情况，用前先核对。

## 放在哪

- 用户级：`~/.agents/skills/self-harness/`。
- 项目级：`.agents/skills/self-harness/`（Codex 也认 `.codex/skills/`）。孵化期可以软链到仓库里的源目录。
- 入口文件：Codex 读 `AGENTS.md`。

## 各档的做法

| 档 | Codex 里用什么 |
| --- | --- |
| 1 写不出来 | 沙箱模式（`-s read-only`、`-s workspace-write`）；包装脚本把模型、沙箱、标准输入写死 |
| 2、3 拦截与提醒 | Codex 有 `hooks.json`，事件和输入格式与 Claude Code 不同，用前查文档。查不清时退到第 3 档：开工、收工手动跑 `harness status` |
| 4 按需加载 | skill |
| 5 常驻 | `AGENTS.md` |

## 非交互运行

- `codex exec` 后面接 `< /dev/null`，否则会卡在「Reading additional input from stdin」。
- 后台跑时盯着输出，几分钟没有进展就去看。
- 任务书写明能写哪些目录。没写时它会改项目文档，还会把任务书当成「人说」记下来。
