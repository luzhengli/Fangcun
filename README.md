# 方寸 Fangcun

方寸是 Agent 上下文的「包管理器 + 环境管理器」：管理 Skills、MCP、指令文件三类资产，让每个项目声明自己需要什么、锁定到哪个版本，并能随时看见 Agent 实际加载了什么。形态是 Mac 桌面应用加命令行 `fangcun`，两者共用同一个内核。

它之于 Skills、MCP、指令文件，相当于 uv 或 mise 之于 Python 依赖与运行时。

- **Agent 读取的目录是构建产物，不是源。** `.claude/skills`、`.mcp.json`、`CLAUDE.md` 由清单和锁生成，不再手工复制。
- **以项目为中心。** 每个项目在 `fangcun.toml` 里声明、在 `fangcun.lock` 里锁定。全局层尽量薄。
- **先看见，再管理。** 任何时刻都能回答：这个项目里的 Agent 加载了什么、占了多少上下文（估算）、谁覆盖了谁。

v0.1 完整支持 Claude Code 与 Codex，其他工具只读展示。

## 安装

```bash
brew install --cask luzhengli/fangcun/fangcun   # 桌面应用
brew install luzhengli/fangcun/fangcun          # 命令行 fangcun（从源码编译，要几分钟）
```

也可以在 [Releases](https://github.com/luzhengli/Fangcun/releases) 下载 dmg 和命令行压缩包，用 `SHA256SUMS` 核对。

**第一次打开**：v0.1 没有签名公证，macOS 会拦下。在「系统设置 → 隐私与安全性」里点「仍要打开」，之后照常使用。

## 两分钟上手

1. **体检。** 打开方寸，先看到体检报告：重复的副本、未收录的资产、每个项目都在缴的「全局税」。命令行是 `fangcun doctor`，只读，不改任何文件。
2. **登记项目。** 在侧栏「近 30 天 · 未登记」里点一个项目，再点「登记…」，选择同步方式：
   - **本机软链**：项目内放软链指向本机资产库，不进 Git，零重复。适合个人项目。
   - **拷入仓库**：实体文件拷入项目目录随 Git 提交，同事与云端 Agent 不装方寸也能用。适合团队项目。

   命令行是在项目目录里运行 `fangcun init --from-existing`。
3. **同步。** 在清单里加资产（⌘K 搜索，或 `fangcun add <资产>`），点「同步」。写入前先给出变更计划，逐行列出 `+` `~` `−`，确认后才落盘。
4. **回滚。** 每次写入前都留快照。在「活动」页对任何一条记录点「回滚」，或 `fangcun rollback <快照>`。

Agent 改了受管的 skill，五秒内出现本地已改提示，可以写回源仓库、还原锁定版或转为项目独立文件（`fangcun drift`）。

## 命令行速查

```bash
fangcun doctor                    # 只读体检
fangcun init --from-existing      # 登记项目，从目录现状反推清单与锁
fangcun status                    # 当前项目的同步状态与本地改动
fangcun drift [资产] [promote|discard|fork]  # 看本地改动与差异；写回源仓库 / 还原锁定版 / 转为独立文件
fangcun add <资产>                 # 加入清单并同步
fangcun remove <资产>… --global    # 从全局层移除（只对已收录的）
fangcun adopt <资产>               # 收录进本机资产库
fangcun lock                      # 重新解析清单，更新锁
fangcun export skills-lock        # 导出 skills-lock.json，与 skills 命令行互通
fangcun sync                      # 按清单与锁同步
fangcun sync --frozen             # CI 用：锁与清单不一致就失败
fangcun update [资产] --dry-run    # 检查可升级的项，只看差异
fangcun rollback <快照>            # 回滚到某个快照
fangcun log                       # 活动记录
fangcun why <资产> [--global]      # 它为何生效、来自哪一层
fangcun secret set|rm|ls          # 钥匙串里的密钥，值不回显、不记录
```

会写入的命令先打印变更计划，再问「应用？[y/N]」。加 `--yes` 不问直接应用；`--read-only` 拒绝一切写入。被脚本或 Agent 调用、又没加 `--yes` 时，只打印计划，不写。

## 卸载

方寸只写各工具自己的原生位置，不发明运行时。卸载应用和命令行后，Agent 照常读到那些文件。

`~/.fangcun` 里是资产库正本、快照和活动记录。用「本机软链」方式的项目指向那里的正本，所以卸载（包括 `brew uninstall --zap`）不删这个目录。要连它一起删，先把项目的同步方式改成「拷入仓库」并同步。

## 隐私

本地优先，零遥测，不需要账号。「用到」的统计只读本机的会话记录，不上传。桌面界面的 CSP 是 `default-src 'self'`，发不出外部请求。

## 开发

设计文档在 [`docs/product-design.md`](docs/product-design.md)，工程架构在 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)，参与开发先读 [`AGENTS.md`](AGENTS.md)。

```bash
pnpm install
pnpm dev      # 桌面应用
pnpm check    # 提交前：格式、lint、类型、测试、前端构建
```

## 许可证

MIT 或 Apache-2.0 双许可，见 [`LICENSE-MIT`](LICENSE-MIT) 与 [`LICENSE-APACHE`](LICENSE-APACHE)。
