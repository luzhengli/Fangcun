# 当前状态

更新于 2026-11-19。每次收工时整份改写，不要追加，不超过 40 行。长期有效的实现知识在 `docs/ARCHITECTURE.md`「实现要点」。

开工先跑 `pnpm workflow --fetch`。

## 现在在哪

M0（透视版，只读）功能全部实现。示例数据已跟上原型，剩 M0-12 窗口按钮再看一眼、M0-23 项目页检查器补被覆盖项的说明框。

- **能用的**：命令行 `doctor`、`status`、`why`、`log`；桌面应用的体检报告、项目页、全局页、活动页，资产库、套装是占位（M1）。体检扫 8 个工具，Claude Code、Codex 完整支持。本机 `fangcun doctor` 约 7 秒，只写 `~/.fangcun/journal`。
- **原型**：`prototype/` 在画布版本 1794656963-e22c，七张画板通过 `check.js`，和第 6 节没有已知不一致。
- **示例数据**：M0-22 完成，Main、Checkup、Global、Activity 四张 accept 到 1794656963-e22c。frontend-design 按 D-19 改成 Claude Code 下全局生效、项目层被遮蔽；Codex 下项目层照常生效。取舍记在 DECISIONS 2026-11-19。
- **检查**：`pnpm screens --compare` 四张差异 4.3%–6.7%（窗口按钮、滚动条、侧栏第二组是已知差异）。`pnpm window-shot` 要屏幕录制权限（H-01）。

## 下一步

1. 人过一遍 BACKLOG「待人处理」：H-01 至 H-04，以及 D-09、D-16、D-17。
2. **M0-23**：项目页检查器给被覆盖的项加说明框和「查看覆盖它的项」按钮，两个全局动作 M0 禁用。
3. **M0-12**：H-01 选 A 后，Agent 用 `pnpm window-shot` 量红黄绿按钮，偏了改 `trafficLightPosition.y`。
4. 然后进 M1。先补 M1-pre-01 至 M1-pre-04；M1-01、M1-02 已定要做。

## 卡在哪

- 没有卡开发的事。D-09、D-16、D-17 第一次对外分发前要定。
- H-03：11-13 的云端分支 `claude/zen-dirac-3ba2zc` 还没合并，等人定。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`（怎么交接）。
- 交接物只有 PR 和 BACKLOG 条目。要人定、要人看的事进「待人处理」。
- 流程上卡住了，记进 WORKFLOW「摩擦记录」，攒到 5 条复盘（现在 1 条没复盘）。
