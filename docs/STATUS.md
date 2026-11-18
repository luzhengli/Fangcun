# 当前状态

更新于 2026-11-19。每次收工时整份改写，不要追加，不超过 40 行。长期有效的实现知识在 `docs/ARCHITECTURE.md`「实现要点」。

开工先跑 `pnpm workflow --fetch`。

## 现在在哪

M0（透视版，只读）的条目全部完成。M1 开工前的格式补了三件（M1-pre-01 至 03，写在 7.4），剩 M1-pre-04 适配器调研。

- **能用的**：命令行 `doctor`、`status`、`why`、`log`；桌面应用的体检报告、项目页、全局页、活动页，资产库、套装是占位（M1）。体检扫 8 个工具，Claude Code、Codex 完整支持。本机 `fangcun doctor` 约 7 秒，只写 `~/.fangcun/journal`。
- **原型**：`prototype/` 在画布版本 1794656963-e22c，七张画板通过 `check.js`，和第 6 节没有已知不一致。
- **示例数据**：M0-22 完成，Main、Checkup、Global、Activity 四张 accept 到 1794656963-e22c。frontend-design 按 D-19 改成 Claude Code 下全局生效、项目层被遮蔽；Codex 下项目层照常生效。取舍记在 DECISIONS 2026-11-19。
- **待人处理**：只剩 D-16（先不定）。旧云端分支已删。
- **检查**：`pnpm screens --compare` 四张差异 4.3%–6.7%（窗口按钮、滚动条、侧栏第二组是已知差异）。`pnpm window-shot --launch` 能真截图了（VS Code 已有屏幕录制权限），窗口按钮与工具栏差 0.25pt。

## 下一步

1. **M1-pre-04**：适配器还没读的位置，清单在 BACKLOG 和各适配器文件开头。
2. 拆 M1：按第 8 节的 M1 范围（收编、资产库、清单与锁、物化同步、漂移、更新回滚、密钥、简版套装、命令行）在 BACKLOG 拆条目。内核要先改哈希算法为 `skills-lock.json` v2（7.4）。
3. 应用图标：PR #15（D-29 规范）待合并；P-01 到云端会话里改原型，D-30 出图。

## 卡在哪

- 没有卡开发的事。D-16 包标识与显示名人选先不定，第一次对外分发前再问。
- 许可证已定：MIT 或 Apache-2.0 双许可（D-09）。

## 接手须知

- 先读 `docs/ARCHITECTURE.md`（不能破的规则、实现要点）和 `docs/WORKFLOW.md`（怎么交接）。
- 交接物只有 PR 和 BACKLOG 条目。要人定、要人看的事进「待人处理」。
- 流程上卡住了，记进 WORKFLOW「摩擦记录」，攒到 5 条复盘（现在 1 条没复盘）。
