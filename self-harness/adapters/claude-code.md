# Claude Code

## 放在哪

- 项目级：`.claude/skills/self-harness/`。孵化期可以软链到仓库里的源目录：`ln -s ../../<源目录> .claude/skills/self-harness`。
- 用户级：`~/.claude/skills/self-harness/`。
- 入口文件：Claude Code 读 `CLAUDE.md`。项目以 `AGENTS.md` 为正本时，`CLAUDE.md` 软链过去。

## 各档的做法

| 档 | Claude Code 里用什么 |
| --- | --- |
| 1 写不出来 | `.claude/settings.json` 的 `permissions.deny`；包装脚本 |
| 2 做错就拦 | `PreToolUse` hook，退出码 2 拦下并把原因回给 Agent |
| 3 适时提醒 | `SessionStart` hook 跑 `harness status`；`Stop` hook 检查收工（改了代码没改状态文件时提醒） |
| 4 按需加载 | skill |
| 5 常驻 | `CLAUDE.md` |

hook 的配置在 `.claude/settings.json` 的 `hooks` 下，改之前先读官方文档里对应事件的输入格式；hook 会拿到 `transcript_path`，需要查「这个会话里做过什么」时可以读它。

## 子代理

- 子代理常被拦着写报告文件、发外部请求。任务书里写明：结果写在最后一条消息里；对外发布由主会话做。
- 中断了（用量上限、被权限拦下）用 SendMessage 让同一个子代理接着做，不要重新派。
