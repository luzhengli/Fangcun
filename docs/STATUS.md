# 当前状态

更新于 2026-11-14。每次收工时整份改写，不要追加。

## 现在在哪

M0（透视版，只读）的功能全部实现。BACKLOG 的 M0-02 至 M0-11、M0-13 至 M0-21 完成，M0-12 按人给的截图改了窗口按钮位置，待人再看一眼。本机实测：`fangcun doctor` 约 7 秒，零配置，只写 `~/.fangcun/journal`。

人已定 D-15、D-18 至 D-22（都是推荐项，D-19 补问了两件），设计文档已改并随 PR #5 合进 main。代码跟上的三件（M0-19 至 M0-21）随 PR #6 合进 main。原型的三处（D-23）已由 Agent 在画布里改好并同步（版本 1794656299-24d3），示例数据还没跟上（M0-22）。

- **能用的**：
  - 命令行：`pnpm cli -- doctor`、`status`、`why <资产> [--global]`、`log`。写入命令仍返回「当前是只读模式」。
  - 桌面应用：启动先进体检报告；项目页、全局页、活动页可用；侧栏列近 30 天的项目，「重新体检」重扫。资产库、套装两页还是占位（M1）。
  - 适配器：Claude Code、Codex 完整支持；Cursor、Copilot CLI、Gemini CLI、opencode、Windsurf、Trae 只读扫描。体检扫 8 个工具。
  - 本机实测：8 个项目、39 项资产；18 个 lark-* skill 只有 Xuanji 用过，10 个全局 skill 近 30 天没人用，Claude Code 只读到 31 个全局 skill 里的 16 个，`~/.agents/.skill-lock.json` 07-05 后没更新。
- **检查**：`pnpm check` 全过（Rust 库 87 个测试，加两组适配器回归样例；前端 12 个）。四个页面用示例数据和原型截图对过，也用本机真实数据截图看过（`pnpm screens --snapshot`）。M0-19 至 M0-21 只改了内核和两处文案，没有重新截图。
- **原型**：`prototype/` 同步到画布版本 1794656299-24d3，`check.js` 七张画板全部通过。项目页 frontend-design 项目层被全局层遮蔽；全局页 frontend-design「同名冲突 2 处」，CLAUDE.md、AGENTS.md「全文加载」。Agent 自己定的细节记在 DECISIONS「原型跟上 D-19、D-21 时的取舍」，待人看。
- **git**：PR #5、#6、#7 都已合并到 main。原型这次的改动在分支 `claude/nifty-rubin-jc7pvh`，已推送，没开 PR。

## 下一步

1. **M0-12**：人用 `pnpm dev` 再看一眼左上角的红黄绿按钮。人给的截图（2026-11-14）里按钮中心比后退、侧栏按钮高约 4.5pt，`apps/desktop/src-tauri/tauri.conf.json` 的 `trafficLightPosition.y` 已从 13 改成 17.5（按 tao 的算法，按钮中心约在 y + 1.75pt，工具栏里其他按钮的中心在 19.25pt）。还偏就按同样算法微调。
2. **M0-22**：示例数据跟上新原型。改 `apps/desktop/src/ipc/fixtures/main.ts`、`global.ts`、`activity.ts`，再 `pnpm screens main global activity` 和 `prototype/screens/` 对比。
3. 人看一眼画布上 Agent 补的细节（DECISIONS「原型跟上 D-19、D-21 时的取舍」）。
4. 然后进 M1。先补 M1-pre-01 至 M1-pre-04。M1-01、M1-02 已定要做。

## 卡在哪

- **D-09、D-16、D-17** 照旧，不卡开发。

## 接手须知

- **先读 `docs/ARCHITECTURE.md`**，新增了「扫描缓存」「项目从哪来」两段和 `global` 模块。最要紧的规则没变：业务逻辑只在内核；M0 用 `Access::ReadOnly`；界面只经 `ipc/` 取数据；颜色只用 `styles/tokens.css`。
- **适配器**格式和现状在 `adapters/README.md`。改了适配器跑 `cargo test -p fangcun-core --test adapters`。没核实、没读的位置写在各适配器文件开头，不要凭记忆补。Trae 用的是二手来源，待核实。
- **同名优先级按工具真实规则**（D-19 已定，4.3）。工具规则与方寸相反时，物化不自动移动上层同名项，标「同名冲突」由人选。Claude Code 的 skill 全局优先，MCP 个人 > 项目 > 全局、整条取用；Codex 的同名 skill 都加载，MCP 项目层赢、按字段合并，未受信任项目的 `.codex/config.toml` 不读（后两条是实测）。界面上「谁覆盖谁」的小字在规则相反时会改写。
- **只读扫描的工具**只参加体检，不进项目页、全局页的工具切换。
- **MCP 常驻不估算**，预算条 MCP 段为 0，旁边写「N 个未估算」。
- **内核第一次被调用时全盘扫描一次并缓存**，读近 30 天会话记录（本机约 400MB，按修改时间先筛）。`Kernel::rescan` 重扫。
- **体检记录写进 `~/.fangcun/journal/records.jsonl`**（D-15）。桌面应用的每次扫描、`fangcun doctor` 记一条；命令行 `status`、`why` 顺带的扫描不记。写不进去时留在内存里。从来没有体检记录时才算「首次体检」。测试一律用临时目录，不碰真实的 `~/.fangcun`；`ipc_snapshot` 会在真实的 `~/.fangcun` 记一条，不想要就设 `FANGCUN_HOME`。
- **全局页的同名**：项目层真正生效（MCP）是「被覆盖」；全局那份照样生效（Claude Code 的 skill）或两份都加载（Codex 的 skill）是「同名冲突」，见 `global.rs` 的 `same_name_in`。
- **项目预算**读 `fangcun.toml` 的 `[project].budget`，M0 只读这一个键，别的键不报错；值写错了会报出来。
- **示例数据**在 `apps/desktop/src/ipc/fixtures/`，按画板分文件，照抄原型。原型和真实内核的差别（例如全局指令的「6 / 6」、MCP 有常驻数）保留在示例数据里，以便和原型截图对比。
- **看真实数据**：`cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json`，再 `pnpm screens --snapshot target/ipc-snapshot.json`，图在 `apps/desktop/screens/real-*.png`。
- **截图对比时的环境差异**，不算不一致：原型画了红黄绿窗口按钮，应用里是系统画的；原型给滚动条预留了位置；示例数据的「今天 / 昨天」按运行当天算，原型固定在 11-13。
- **各画板的示例数据有几处对不上**（全局层有什么、tdd 与 project-harness 的差异行数等），文档各小节照各自画板写，没有强行统一。
- **使用数据**：Claude Code 认 `Skill` 工具调用、用户输入的 `/命令`、`mcp__服务__工具`；Codex 认命令里读了 `…/skills/<名称>/SKILL.md`（插件缓存里的不算）和 `mcp__` 调用。Codex 的 MCP 调用本机没有样本。
- **快照按范围各自编号**，跨项目操作每个项目各留一个快照（M1）。
- 补改原型时 Agent 自己定的六件，人说「暂时按照你的方案」，是暂定，细节在 `docs/DECISIONS.md`。
