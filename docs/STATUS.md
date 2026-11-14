# 当前状态

更新于 2026-11-14。每次收工时整份改写，不要追加。

## 现在在哪

M0（透视版，只读）的功能全部实现。BACKLOG 的 M0-02 至 M0-11、M0-13 至 M0-18 完成；只剩 M0-12 等人批准 Codex 操作桌面。出口标准本机实测：`fangcun doctor` 约 7 秒，零配置，`~/.fangcun` 没有建，没有写任何文件。

- **能用的**：
  - 命令行：`pnpm cli -- doctor`、`status`、`why <资产> [--global]`、`log`。写入命令仍返回「当前是只读模式」。
  - 桌面应用：启动先进体检报告；项目页、全局页、活动页可用；侧栏列近 30 天的项目，「重新体检」重扫。资产库、套装两页还是占位（M1）。
  - 适配器：Claude Code、Codex 完整支持；Cursor、Copilot CLI、Gemini CLI、opencode、Windsurf、Trae 只读扫描。体检扫 8 个工具。
  - 本机实测：8 个项目、39 项资产；18 个 lark-* skill 只有 Xuanji 用过，10 个全局 skill 近 30 天没人用，Claude Code 只读到 31 个全局 skill 里的 16 个，`~/.agents/.skill-lock.json` 07-05 后没更新。
- **检查**：`pnpm check` 全过（Rust 86 个测试，含两组适配器回归样例；前端 12 个）。按 ci.yml 在只含要提交文件的干净副本里跑过四步，全过。四个页面用示例数据和原型截图对过，也用本机真实数据截图看过（`pnpm screens --snapshot`）。
- **git**：全部改动在分支 `m0-readonly`，已推送，PR #5 等人合并。CI 第一次运行两项都过。

## 下一步

1. 人看 PR #5，合并到 main。
2. **M0-12**：在 Codex 里批准它用 computer use 操作 Fangcun（和系统设置），再跑一遍检查；或人用 `pnpm dev` 自己看窗口按钮对齐、工具栏拖动、深色模式。debug 包在 `target/debug/bundle/macos/Fangcun.app`。
3. 人看「要人定」的 D-18 至 D-22。**D-19 最要紧**：Claude Code 的 skill 同名时全局盖过项目，和 4.3「下层覆盖上层」相反，影响 M1 物化怎么做。
4. 然后进 M1。先补 M1-pre-01 至 M1-pre-04。

## 卡在哪

- **M0-12**：Codex CLI 0.167.0 的 computer use 能起来、也找到了 Fangcun 窗口，但非交互运行时自动审批拒绝了对 Fangcun 和系统设置的访问。按 `AGENTS.md`，Claude Code 不能自己操作桌面。
- **要人定的设计**：D-15、D-18 至 D-22，实现已按推荐项或最保守的做法先做，见 DECISIONS。设计文档没有改。
- **D-09、D-16、D-17** 照旧，不卡开发。

## 接手须知

- **先读 `docs/ARCHITECTURE.md`**，新增了「扫描缓存」「项目从哪来」两段和 `global` 模块。最要紧的规则没变：业务逻辑只在内核；M0 用 `Access::ReadOnly`；界面只经 `ipc/` 取数据；颜色只用 `styles/tokens.css`。
- **适配器**格式和现状在 `adapters/README.md`。改了适配器跑 `cargo test -p fangcun-core --test adapters`。没核实、没读的位置写在各适配器文件开头，不要凭记忆补。Trae 用的是二手来源，待核实。
- **同名优先级按工具真实规则**：Claude Code 的 skill 全局优先，MCP 个人 > 项目 > 全局、整条取用；Codex 的同名 skill 都加载，MCP 项目层赢、按字段合并，未受信任项目的 `.codex/config.toml` 不读（后两条是实测）。界面上「谁覆盖谁」的小字在规则相反时会改写。
- **只读扫描的工具**只参加体检，不进项目页、全局页的工具切换。
- **MCP 常驻不估算**，预算条 MCP 段为 0，旁边写「N 个未估算」。
- **内核第一次被调用时全盘扫描一次并缓存**，读近 30 天会话记录（本机约 400MB，按修改时间先筛）。`Kernel::rescan` 重扫。
- **体检记录只在内存里**：`fangcun log` 每次是新进程，所以永远是空的；桌面应用里能看到本次运行的体检。
- **示例数据**在 `apps/desktop/src/ipc/fixtures/`，按画板分文件，照抄原型。原型和真实内核的差别（例如全局指令的「6 / 6」、MCP 有常驻数）保留在示例数据里，以便和原型截图对比。
- **看真实数据**：`cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json`，再 `pnpm screens --snapshot target/ipc-snapshot.json`，图在 `apps/desktop/screens/real-*.png`。
- **截图对比时的环境差异**，不算不一致：原型画了红黄绿窗口按钮，应用里是系统画的；原型给滚动条预留了位置；示例数据的「今天 / 昨天」按运行当天算，原型固定在 11-13。
- **各画板的示例数据有几处对不上**（全局层有什么、tdd 与 project-harness 的差异行数等），文档各小节照各自画板写，没有强行统一。
- **使用数据**：Claude Code 认 `Skill` 工具调用、用户输入的 `/命令`、`mcp__服务__工具`；Codex 认命令里读了 `…/skills/<名称>/SKILL.md`（插件缓存里的不算）和 `mcp__` 调用。Codex 的 MCP 调用本机没有样本。
- **快照按范围各自编号**，跨项目操作每个项目各留一个快照（M1）。
- 补改原型时 Agent 自己定的六件，人说「暂时按照你的方案」，是暂定，细节在 `docs/DECISIONS.md`。
