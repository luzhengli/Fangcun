# 当前状态

更新于 2026-11-14。每次收工时整份改写，不要追加，不超过 40 行。长期有效的实现知识在 `docs/ARCHITECTURE.md`「实现要点」。

开工先跑 `pnpm workflow --fetch`。

## 现在在哪

M0（透视版，只读）功能全部实现，只剩 M0-12 窗口按钮再看一眼、M0-22 示例数据跟上原型。

- **能用的**：命令行 `doctor`、`status`、`why`、`log`；桌面应用的体检报告、项目页、全局页、活动页，资产库、套装是占位（M1）。体检扫 8 个工具，Claude Code、Codex 完整支持。本机 `fangcun doctor` 约 7 秒，只写 `~/.fangcun/journal`。
- **原型**：`prototype/` 在画布版本 1794656963-e22c，`check.js` 七张画板通过，和第 6 节没有已知不一致。PR #8 已合并。
- **工作流**：新建 `docs/WORKFLOW.md`（一轮迭代、口令、交接规则、质量关、复盘）。新工具 `pnpm workflow`、`pnpm screens --compare`、`pnpm window-shot`；CI 在示例数据落后时留警告。BACKLOG 加了「待人处理」「原型待改」「工作流」三节。都在分支 `workflow-loop` 上，还没提交。
- **检查**：`pnpm screens --compare` 本机跑通，六张画板差异 1.6%–6.7%，Global 的拼图里能看到 frontend-design 还是「被覆盖 2 处」（M0-22）。`pnpm window-shot` 的找窗口、报错路径跑过，真正截图要等屏幕录制权限（H-01）。

## 下一步

1. 人过一遍 BACKLOG「待人处理」：H-01 至 H-04，以及 D-09、D-16、D-17。
2. **M0-22**：示例数据跟上原型。`pnpm workflow` 列出 Main、Checkup、Global、Activity 四张落后；改完 `pnpm screens --compare` 看拼图，再 `pnpm workflow accept <画板>`。
3. **M0-12**：H-01 选 A 后，Agent 用 `pnpm window-shot` 量红黄绿按钮，偏了改 `trafficLightPosition.y`。
4. 然后进 M1。先补 M1-pre-01 至 M1-pre-04；M1-01、M1-02 已定要做。

## 卡在哪

- 没有卡开发的事。D-09、D-16、D-17 第一次对外分发前要定。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`（怎么交接）。
- 交接物只有 PR 和 BACKLOG 条目。要人定、要人看的事进「待人处理」。
- 流程上卡住了，记进 WORKFLOW「摩擦记录」，攒到 5 条复盘。
