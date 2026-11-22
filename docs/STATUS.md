# 当前状态

只写判断：下一步、卡在哪、接手须知。分支、PR、待人处理、示例数据这些事实跑 `pnpm workflow --fetch` 看，不抄在这里。每次收工整份改写，不超过 40 行。

## 现在在哪

M1 进行中，目标是 v0.1。方寸 2 的界面批次都做完了，剩批 7 发布。工作流上刚孵化出 self-harness（通用 skill），本项目是它的第一个用户。

## 下一步

1. **人**：先合 #47（第三次复盘），再合 self-harness 孵化的 PR。看一眼 H-15（设计文档要不要做要求编号）。
2. **Agent**：批 7 发布。开工时问人：R-03 的 tap 仓库、R-01 的包标识（D-16）、M1-35 全局页「建议转套装」的文案。
3. **Agent**：harness 的下一轮，按 `pnpm harness audit` 的结论：W-31 AGENTS.md 瘦到 120 行、W-32 规则升档第一批（几条 grep 级 lint、`pnpm codex` 包装脚本、「改画布」skill）、W-33 BACKLOG 归档。可以穿插在发布之间做。
4. **Agent（终端会话）**：P-14，资产库「谁在用」写法和全局统一。
5. 发布前可以顺手做：M1-48 项目页与原型的界面差异。

## 卡在哪

- D-34 剩真机实测（VoiceOver、200% 缩放、菜单里的 ⌘K），Codex 操控桌面要人给权限（R-06）。
- 目标工具图标贴近官方标志，发布前要查商标规定（R-07）。
- self-harness 只在两个项目上试过，Habitat 那边还没落 `.harness/`，要和人商量（W-30）。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`。
- 交接物只有 PR 和 BACKLOG 条目。做了什么写在 PR 描述里，`docs/LOG.md` 已停写。
- 改了规则或质量关，同步改 `.harness/rules.json`、`oracles.json`；`self-harness/` 里不写本项目专有的词（`pnpm lint` 会拦）。
- 派子代理或 Codex 照 WORKFLOW「任务书」写；画布只由主会话发布。
- 跟人交流、写文档一律用中文。
