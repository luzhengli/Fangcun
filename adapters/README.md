# 适配器

每个 Agent 工具一份声明式描述。适配器是数据，不是代码（设计文档 7.3）：工具改了目录约定，改这里的 TOML，不改内核。

## 加载顺序

1. 内置：本目录的文件在编译时打进内核（`crates/fangcun-core/src/adapter/registry.rs`）。
2. 热更新：`~/.fangcun/adapters/<id>.toml`，同名时盖过内置的那份。还没实现。

新增一个工具：加一份 `<id>.toml`，在 `registry.rs` 的 `BUILTIN` 里登记，补回归样例。

## 格式

结构体在 `crates/fangcun-core/src/adapter/schema.rs`，开了 `deny_unknown_fields`：文件里出现结构体不认识的字段会直接报错，所以格式和数据必须一起改。

路径写法：`~/` 开头是主目录下的位置（全局层用）；`/` 开头是绝对路径；其余相对项目根目录（项目层、个人层用）。

```toml
schema = 1                     # 描述格式的版本

[tool]
id = "claude-code"             # 与 fangcun.toml 里 targets 的写法一致
name = "Claude Code"           # 界面上显示的名字
support = "full"               # full：完整支持。scan-only：只读扫描

[capabilities]                 # 每一项都可省略，省略表示还没核实，界面按「未知」处理
follows-symlinks = true        # 是否跟随软链
project-mcp = true             # 是否支持项目级 MCP
per-project-disable = true     # 是否支持按项目停用

[[skills]]                     # 一个 skill 目录，下面每个含 SKILL.md 的子目录是一个 skill
scope = "global"               # global | project | personal
path = "~/.claude/skills"
walk = "root"                  # 项目层从哪些目录找：root | ancestors | git-root-down
lock = { path = "~/.agents/.skill-lock.json", format = "skills-cli" }  # 可选，配套的锁文件

[[instructions]]               # 指令文件
scope = "project"
names = ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"]
walk = "ancestors"
pick = "all"                   # all：每层列出的都读。first：每层取第一个非空的
fallback = ["AGENTS.md"]       # 所有目录里都没有 names 时改读这些
personal = ["CLAUDE.local.md"] # 属于个人层的文件名
max-bytes = 32768              # 合并后的上限，超出的不加载

import-depth = 4               # 文件里的 @路径 一起带进来，最多几层。0 或不写表示不展开
extra-names = { file = "$CODEX_HOME/config.toml", format = "toml", key = ["project_doc_fallback_filenames"] }  # 从配置里追加文件名

[[rules]]                      # 规则目录：文件全部加载，frontmatter 带 paths 的按需加载
scope = "project"
dir = ".claude/rules"
extension = "md"               # 默认 md，Cursor 是 mdc

[[mcp]]                        # MCP 配置所在的文件与键路径
scope = "personal"
file = "~/.claude.json"
format = "json"                # json | toml
key = ["projects", "{project}", "mcpServers"]   # {project} 换成项目根目录的绝对路径
# 可选：工具只在满足条件时才读这份配置，不满足时界面写 unmet 的说明
requires = { file = "$CODEX_HOME/config.toml", format = "toml", key = ["projects", "{project}", "trust_level"], equals = "trusted", unmet = "项目没有标为受信任" }

[mcp-fields]                   # 服务条目里的字段名
command = "command"
args = "args"
env = "env"
url = "url"
enabled = "enabled"            # 可选，值为 false 时不加载

[precedence]                   # 同名时谁生效，排在前面的层赢；空表示都加载
skills = ["global", "project"]
mcp = ["personal", "project", "global"]
mcp-merge = "replace"          # replace：整条取用。fields：按字段合并，赢的那层写了的字段盖上去

[path-vars]                    # 路径开头的 $名称：先取同名环境变量，没有就用这里的默认值
CODEX_HOME = "~/.codex"

[[skill-switches]]             # 配置里按 path 或 name 停用 skill 的开关表，多项匹配时后面的赢
scope = "global"
file = "$CODEX_HOME/config.toml"
format = "toml"
key = ["skills", "config"]
path-field = "path"
name-field = "name"
enabled-field = "enabled"
# requires = { … }             # 可选，同 [[mcp]]

[[observed]]                   # 「其他加载项」，只看不管
kind = "hook"                  # plugin | hook | slash-command | subagent
scope = "project"
file = ".claude/settings.json" # 配置文件：键路径下的对象，每个键一项
format = "json"
key = ["hooks"]
# 或者 dir = ".claude/commands"：目录里每个 .md 一项
# 插件可以再给安装清单，界面上写「含 N 个 skill」：
# install-index = { file = "~/.claude/plugins/installed_plugins.json", format = "json", key = ["plugins"], path-field = "installPath", skills-dir = "skills" }

[sessions]                     # 会话记录，用来统计触发次数、发现项目
dir = "~/.claude/projects"
format = "claude-code-jsonl"   # claude-code-jsonl | codex-rollout-jsonl，解析写在 scanner/usage.rs
```

## 现状

2026-11-14 对照各工具的当期文档核实，出处和没读的位置写在各自文件开头。

| 工具 | 支持 | 来源 | 备注 |
| --- | --- | --- | --- |
| Claude Code | 完整 | 官方文档；会话记录、插件安装清单对照本机文件 | skill 同名时全局优先于项目；MCP 个人 > 项目 > 全局，整条取用。按项目停用没核实 |
| Codex | 完整 | 官方文档；MCP 合并与受信任规则用隔离的 `CODEX_HOME` 实测 | 同名 skill 都加载；MCP 项目层赢、按字段合并；项目配置只对受信任项目生效 |
| Cursor | 只读扫描 | 官方文档 | 规则是 `.mdc`；界面里设的 User rules 不在文件里 |
| Copilot CLI | 只读扫描 | 官方文档 | `COPILOT_HOME` 可换目录 |
| Gemini CLI | 只读扫描 | 官方文档 | 文档写明已在 2026-06-18 被 Antigravity CLI 取代；skill 目录文档没写 |
| opencode | 只读扫描 | 官方文档 | MCP 是 JSONC、`command` 为数组，还没读 |
| Windsurf | 只读扫描 | Devin 文档（Windsurf 文档已跳转） | MCP 没读 |
| Trae | 只读扫描 | **二手来源**（引用官方页的 rulesync），官方页没能读出 | 待核实 |

只读扫描的工具参加体检（重复、分发不齐、资产数），不出现在项目页和全局页的工具切换里，也不统计使用数据。

## 回归样例

`fixtures/<id>/home` 是仿真的主目录，`fixtures/<id>/project` 是仿真的项目目录。测试在 `crates/fangcun-core/tests/adapters.rs`：把样例复制到 `target/` 下，补上要写真实路径的文件（`~/.claude.json`、会话记录），再断言生效上下文和体检结果。工具升级或改了适配器后跑：

```bash
cargo test -p fangcun-core --test adapters
```
