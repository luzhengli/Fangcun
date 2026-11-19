# 当前状态

更新于 2026-11-20。每次收工时整份改写，不要追加，不超过 40 行。长期有效的实现知识在 `docs/ARCHITECTURE.md`「实现要点」。

开工先跑 `pnpm workflow --fetch`。

## 现在在哪

M0（透视版，只读）完成，M1 进行中。第一段（地基）与第二段的写入主干（M1-10 同步、11 回滚、12 收编）的内核与命令行完成。M1 按「一批一个分支、一个 PR」推进（DECISIONS 2026-11-19）：批 1 已合（#27）→ 批 2 已合（#28）→ 批 3 第二段界面（W-14、M1-13、14、25、26）→ 批 4 日常维护（18+02、17、15+16、01、27）→ 批 5 套装与资产管理（19+21、20+22、24+23）。

- **能用的**：命令行 `doctor`、`status`、`why`、`log`、`init [--from-existing]`、`lock`、`export skills-lock`、`sync [--frozen]`、`rollback N`、`adopt`。「登记 → 写锁 → 同步 → 回滚」能跑通：article 副本上同步 21 个 skill 约 7 秒，回滚后和原项目逐字节一致。桌面应用还是 M0 的只读界面，写入按钮置灰（批 3 接）。
- **没做的写入**：用了 `secret:` 的 MCP（M1-17）、个人覆盖的 MCP（M1-27）、`[disable]`（M1-18）。同步时在计划里提示。
- **原型**：`prototype/` 在画布版本 1794656963-e22c，和第 6 节没有已知不一致。待改：P-01（应用图标）、P-02（登记入口）、P-03（设置窗口）。
- **示例数据**：四张 accept 到 1794656963-e22c。内核类型新加的字段（`registered`、`snapshots`、`reverts`）示例数据已补。
- **检查**：`pnpm check` 通过，内核 149 个测试。

## 下一步

1. **先把本地设计原型的配置搞定（人定，2026-11-20）**。目标：原型仍用 Claude Design 画布，但改画布、同步回 `prototype/`、实现都在本地做，不再两地操作。
   - 已查明（2026-11-20）：本地会话里没有 Artifact 工具，读不到画布 `WAK5PGWGuUmNNAYS8hFcXq`（VS Code 插件、`-p`、交互式终端都试过）。最可能的原因是 `~/.claude/settings.json` 的 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`，官方文档写明它关掉 artifact；单次运行用 `--settings` 覆盖成 `0` 不起作用。VS Code 插件本身也不在 artifact 支持的界面里（只支持终端 CLI 和 Claude 桌面应用）。
   - 要做：人决定是否从全局设置里去掉这个变量（`DISABLE_TELEMETRY` 可以留）→ 在终端里开 `claude`，运行 `/artifacts` 看列表里有没有这张画布，挂到会话后能否列出、读写它的文件 → 能的话改 AGENTS.md「原型怎么读」、WORKFLOW「同步原型」、`prototype/README.md`，把同步改成本地做；原型类会话在终端或桌面应用里开。
   - 走不通时退回原做法：在 claude.ai web 端说「同步原型」，按 BACKLOG「原型待改」改 P-01、P-02、P-03。
   - 顺带确认这张画布不在独立的 claude.ai/design（2026-12-14 关闭），在的话先迁移。
2. **原型 P-01、P-02、P-03**：用上一步定下的方式改，同步到 `prototype/`、逐条对照文档。
3. **批 3**：W-14（截图能截非默认状态）→ M1-13 同步面板（显示 `notes`、`blocked` 置灰）、M1-14 历史与回滚（`plan_rollback_record`）、M1-26 体检一键动作，再加等原型的 M1-25 登记界面、M1-24 设置窗口。界面照原型，开工前 `pnpm workflow find`。P-01 确认后 D-30 出图。

## 卡在哪

- 设计上没有卡在人身上的事。D-16（包标识）第一次对外分发前再问。
- 批 3 的 M1-25 等原型 P-02，M1-24 等 P-03。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`（怎么交接）。
- 交接物只有 PR 和 BACKLOG 条目。要人定、要人看的事进「待人处理」。
- 流程上卡住了，记进 WORKFLOW「摩擦记录」，攒到 5 条复盘（现在 3 条）。问人、写代码之前先 `pnpm workflow find`，结果别截断（交接规则第 5 条）。
- 试写入别在真实项目和主目录上试：`cp -a` 到临时目录，`FANGCUN_HOME` 指向临时目录。收编会写全局目录，只在测试里跑（`with_user_home`）。
