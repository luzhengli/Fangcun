# 当前状态

## 下一步

1. 在终端本地 `claude` 会话同步更新原型画布，补齐 `BACKLOG.md`「原型待改」`P-14`～`P-19`（含将「纳管」等旧术语统一改为「收录」），同步 `prototype/` 后跑 `pnpm screens --compare` 与 `pnpm workflow accept` 完成像素级验收。
2. 推进 M1 剩余内核缺口（`M1-44` 上游检查时间、`M1-45` 添加来源、`M1-47` 分叉标记、`M1-36`～`M1-38`）。

## 卡在哪 / 接手须知

- **无代码阻塞**：全量 UI/UX、产品文案（「收录」替代「纳管」）、设计系统（墨格）及第 2 档质量守卫已在应用与内核侧落地并通过验证。
- **原型画布滞后于设计文档**：`docs/product-design.md` 与 `apps/desktop` 均已升级至「收录」与最新设计系统，不得因 `prototype/` 尚未重绘而将已落地的 UI 交互或文案倒退回旧版。
- **术语兼容边界**：面向用户的文案由 `designGuard.test.ts`（第 2 档）守卫，严禁出现「物化 / 收编 / 纳管 / 已漂移 / 处漂移 / 回流到 / 瘦身托盘 / 加入托盘」；CLI 英文子命令（`adopt` 等）与 TOML/JSON 字段名保持向后兼容。
- **设计系统守卫**：`controls.module.css` 收口按钮与分段胶囊，`tokens.css` 与 `base.css` 维护 `--on-k`、叠层与 `tabular-nums` 规范，禁止绕过 token 另调色值。
