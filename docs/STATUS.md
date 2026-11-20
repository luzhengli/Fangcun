# 当前状态

更新于 2026-11-21。每次收工时整份改写，不要追加，不超过 40 行。长期有效的实现知识在 `docs/ARCHITECTURE.md`「实现要点」。

开工先跑 `pnpm workflow --fetch`。

## 现在在哪

M0（透视版，只读）完成，M1 进行中。M1 按「一批一个分支、一个 PR」推进（DECISIONS 2026-11-19）：批 1、2、3 已合（#27、#28、#34），项目卡片已合（#35）→ **批 4 日常维护进行中，分支 `batch4`，草稿 PR** → 批 5 套装与资产管理（19+21、20+22、24+23）。

- **批 4 做完的**：M1-18 按项目停用。清单 `[disable]` 同步时写进 Claude Code 的 `skillOverrides`、Codex 的 `[[skills.config]]` 和服务表 `enabled`，Claude Code 的全局 MCP 写本机 `~/.claude.json` 的 `disabledMcpServers`；检查器全局税块「在本项目停用」先加入同步计划。
- **批 4 没做的**：M1-02 同名冲突（H-11 实测完，处理依赖 M1-19、M1-20，挪到批 5）、M1-17 密钥、M1-15+16 漂移与更新、M1-01 MCP 试启动、M1-27 个人覆盖的 MCP，可以并进 M1-28。
- **能用的**：命令行 `doctor`、`status`、`why`、`log`、`init [--from-existing]`、`lock`、`export skills-lock`、`sync [--frozen]`、`rollback N`、`adopt`。桌面应用：同步与变更计划、历史回滚、登记、设置、体检「重复」一键收编、项目卡片、在本项目停用。
- **原型**：`prototype/` 在 1795193620-6b7f。原型待改 P-08（全局税块按钮、已停用的项给「在本项目恢复」），界面已按 6.3 做。
- **检查**：`pnpm check` 通过，内核 163 个测试，前端 12 个。

## 下一步

1. **人**：H-09（`pnpm dev` 里点一遍写入）。P-08 要改原型（终端会话里说「按原型待改改原型并同步」）。
2. **批 4 接着做**：M1-17 密钥（钥匙串、`secret:` 引用，也让用了密钥的 MCP 能同步）→ M1-15+16 漂移与更新 → M1-01 → M1-27。开工前 `pnpm workflow find`。

## 卡在哪

- D-16（包标识）第一次对外分发前再问。D-31（`.icon`）等 Tauri 支持。摩擦记录攒到 5 条，人说「复盘工作流」时复盘。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`（怎么交接）。
- 交接物只有 PR 和 BACKLOG 条目。要人定、要人看的事进「待人处理」。
- 问人、写代码之前先 `pnpm workflow find`，结果别截断（交接规则第 5 条）。
- 界面里的写入一律走 `state/plan.ts` 的 `preview`；检查器里的清单改动先进 `state/staged.ts`，由「同步 · N」一起出计划。
- 试写入别在真实项目和主目录上试：`cp -a` 到临时目录，`FANGCUN_HOME` 指向临时目录。收编会写全局目录，只在测试里跑（`with_user_home`）。
