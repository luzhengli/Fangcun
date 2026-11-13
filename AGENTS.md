# 方寸 Fangcun

方寸是 Agent 上下文的「包管理器 + 环境管理器」：管理 Skills、MCP、指令文件三类资产，让每个项目声明自己需要什么、锁定到哪个版本，并能随时看见 Agent 实际加载了什么。形态是 Mac 桌面应用加命令行 `fangcun`。完整设计见 [docs/product-design.md](docs/product-design.md)。

本文件是每次工作的入口，只放长期不变的东西。进展和待办在 `docs/` 里，按下面的顺序读，够用就停。

## 按需阅读的顺序

| 层 | 文件 | 什么时候读 |
| --- | --- | --- |
| 0 | `AGENTS.md`（本文件） | 每次，自动加载 |
| 1 | `docs/STATUS.md` | 每次开工先读：现在在哪、下一步、卡在哪 |
| 2 | `docs/BACKLOG.md` | 要挑活或查某件事的状态时 |
| 3 | `docs/product-design.md` 的对应章节 | 要设计或实现某个功能时，按下面的章节索引只读那一节。全文约 890 行，不要通读 |
| 4 | `docs/DECISIONS.md`、`docs/LOG.md`、`docs/archive/` | 要查某个决定的来由、某天做了什么、原型原件时 |

## 目录

| 路径 | 内容 |
| --- | --- |
| `AGENTS.md` | 本文件，指令文件的正本 |
| `CLAUDE.md` | 指向 `AGENTS.md` 的软链。不要单独编辑 |
| `docs/product-design.md` | 产品设计方案（中文） |
| `docs/STATUS.md` | 当前状态和下一步，每次收工整份改写 |
| `docs/BACKLOG.md` | 待办，按期分节，带状态标记 |
| `docs/DECISIONS.md` | 决定记录，只追加 |
| `docs/LOG.md` | 工作日志，只追加 |
| `prototype/` | 网页版原型画布的源文件镜像，实现界面时参考。只同步，不手改。同步记录在 `prototype/README.md` |
| `tools/prototype-preview/` | 原型的本地预览器，把 `prototype/` 的画板渲染成可交互的页面。用法见 `prototype/README.md`「本地看」 |
| `docs/archive/` | 旧的原型导出件，文件名带导出日期。2026-11-14 起改为同步源文件到 `prototype/`，这里不再新增 |

仓库里目前只有文档，还没有代码。开始写代码后，在这里补上代码目录和开发命令。

## 设计文档章节索引

用标题检索，不要靠行号。

| 要了解 | 读这一节 |
| --- | --- |
| 定位、三个核心主张、命名 | 1. 结论先行 |
| 痛点、本机扫描到的事实、五个根因 | 2. 痛点深挖 |
| 竞品 | 3. 现状与竞品 |
| 七个概念、四层作用域、版本、六条设计原则 | 4. 核心理念与概念模型 |
| MVP 六项能力与验收、命令行、明确不做 | 5. MVP 核心功能 |
| 窗口骨架、检查器统一结构 | 6.1 形态 |
| 某个页面 | 6.2 体检报告、6.3 项目页、6.4 资产库、6.5 套装、6.6 全局、6.7 活动 |
| 变更计划、关键流程 | 6.8、6.9 |
| 视觉规范要点 | 6.10 视觉与交互基调 |
| 技术选型、模块、适配器、清单示例、安全 | 7. 技术架构 |
| 分期与指标 | 8. 路线图 |
| 人已拍板的事、界面遗留项的结论 | 9.2、9.3 |

## 谁说了算

顺序是 **设计 → 原型 → 实现**，原型跟着产品设计走（2026-11-14 人定）。

- **需求**：产品设计文档是唯一来源，界面也一样。第 6 节写的就是界面的要求，原型和设计系统「墨格」按它画。
- **两者不一致**：以设计文档为准，改原型，不改文档。原型里有、文档里没有的东西，算一条设计提议：问人要不要写进文档，写进去之前不实现。
- **文档没写到的设计**：先问人，一件一件问，每件至少三个选项并标出推荐项。结论写进正文和 9.3，原话记进 `docs/DECISIONS.md`。
- **示例数据**：线框图与原型用同一套数据（6 个项目、全局 31 项、资产库 38 项等）。新画线框图时不要另编一套。

### 每轮迭代

1. **设计**：改 `docs/product-design.md`，人确认。没确认的不往下走。
2. **原型**：人在网页版画布里改，或者让 Agent 改画布。改完同步到 `prototype/`，逐条对照文档，列出不符合的地方，直到原型符合文档。
3. **实现**：只实现文档已确认、原型也已符合的部分。看样子、文案、示例数据时读 `prototype/`，判断该不该做时读文档。

## 原型怎么读

人在网页版画布里改原型，再把源文件同步到 `prototype/`。实现时读 `prototype/`，不读网页（2026-11-14 人定）。

- 画布：https://claude.ai/artifact/WAK5PGWGuUmNNAYS8hFcXq 。旧画布 CJ7tjMr5uEiXFFCcVcBEpp 是按 9.3 补改之前的版本，不再更新。
- 当前同步到哪个版本、哪个文件对应哪一节，看 `prototype/README.md`。
- `prototype/` 只同步，不手改。要改原型，先问人；人让 Agent 改时，在云端会话里改画布再同步回来。

### 文件结构

- `prototype/project/canvas.json`：画板索引。`boards` 里每张画板的位置、尺寸、标题，`notes` 是画布上的标题和便签。
- `prototype/project/<名称>.dc.html`：每张画板一个文件，是完整的 HTML，分三段：
  1. `<helmet><style>`：页面基础样式。`.fc{--bg:…}` 和 `.fc.dark{…}` 是「墨格」的颜色 token，浅色与深色各一套。实现时颜色从这里和 `DesignSystem.dc.html` 取，不要另调。
  2. `<x-dc>` 里的标记：界面本身。样式写在行内 `style` 里。`{{name}}` 是绑定到脚本的值；`<sc-for list="{{xs}}" as="x">` 是循环，`<sc-if value="{{cond}}">` 是条件；`onClick="{{fn}}"` 是事件；`<a href="Activity.dc.html">` 是跳到另一张画板。
  3. 末尾的 `<script type="text/x-dc" data-dc-script>`：`class Component extends DCLogic` 的 `renderVals()`。里面有状态的默认值、示例数据（如活动页的 `EV`、全局页的 `ROWS`）和交互逻辑。`data-props` 是画布上的调节项，例如深色模式、密度。
- 画布的运行时 `support.js` 不在仓库里，直接打开文件没有样式。要看样子，用 `tools/prototype-preview/` 的预览器，用法见 `prototype/README.md`「本地看」。
- 预览器是自己写的，只实现画板用到的模板语法。画布里新用了别的语法（例如 `<dc-import>`、`<x-import>`）时，同步后预览器可能显示不对，要跟着补。

### 怎么读

- 先按 `prototype/README.md` 的表找到画板，再用 `grep -n` 找要的文案或数据，只读那一段。单个文件 25–80KB，不要整份读。
- 找脚本段：`grep -n data-dc-script prototype/project/*.dc.html`。
- 看结构时去掉行内样式和图标：`sed -E 's/ style="[^"]*"//g; s/<svg.*<\/svg>//g' 文件`。
- 实现时：文案和示例数据照抄；结构和交互按原型改写成 React 组件，不要搬 `DCLogic` 或 `sc-*` 标签；视觉只用「墨格」token。

### 同步

人说「同步原型」时，在 claude.ai 云端会话里做（要用 Artifact 工具，本地命令行没有）：

1. 用 Artifact 工具 `list`（`scope: "files"`）列出画布文件。只要 `project/` 下的文件；`index.html`、`SKILL.md`、`artifact-type/` 是编辑器本身，不要。
2. 用 `read`，`paths` 填全部 `project/…` 路径，`out_dir` 填仓库的 `prototype`，一次读完。画布上已删除的画板，把本地对应文件也删掉。不要用网页抓取去读。
3. 改写 `prototype/README.md` 的同步记录（版本号取 `read` 结果里的 version）。新增或删掉画板时，改它的画板表。
4. `git diff --stat prototype/` 看哪些画板变了，逐条对照设计文档。不符合文档的，列给人，改原型；文档里没有的，问人要不要写进文档。不要为了迁就原型去改文档。
5. 按「收工前必须做的事」更新四份文档，提交。

本地会话只需 `git pull` 拿到最新的 `prototype/`。

### 旧导出件

`docs/archive/` 里是 11-13、11-14 的网页另存件，是打包文件。要读时先解包：`<script type="__bundler/manifest">` 是 JSON，每项的 `data` 是 base64，`compressed` 为真时再 gzip 解压；`<script type="__bundler/template">` 是外壳或单画板页面；页面内容在最后一个 `</helmet>` 之后。现在一般不需要读它们。

## 交流

跟人交流和写文档用中文，技术词汇可保持英语。

## 设计的前提

来自设计文档第 1 节和 4.6 节。要改其中任何一条，先问人。

- Agent 读取的目录是构建产物，不是源。以项目为中心，全局层尽量薄。先看见，再管理。
- 不发明运行时，只写各工具的原生位置。卸载方寸后一切照常工作。
- 文件即真相：清单 `fangcun.toml` 与锁 `fangcun.lock` 是纯文本，界面只是它们的编辑器。
- 预览先于写入，一切可撤销，只动自己的东西。
- 本地优先，零遥测，不需要账号。
- MVP 覆盖 Skills、MCP、指令文件，首批完整支持 Claude Code 与 Codex。先做只读的 M0。
- 技术栈是 Rust 内核 + Tauri 2 外壳 + React 界面。桌面应用与命令行共用内核。
- 全部开源。清单格式是自有的 `fangcun.toml`，可导入导出 `skills-lock.json`。产品名是方寸 Fangcun。

后两条是人在 2026-11-14 确认的，完整结论见设计文档 9.2。

## 写文档的约定

- **固定术语**：资产、物化（界面动词用「同步」）、漂移、回流 / 丢弃 / 分叉（顺序固定）、固定 / 跟随、常驻（永远标「估算」）。
- **语气**：陈述事实，给出数字，按钮写动作。不写安抚性的话。
- **状态**：一律「圆点加短标签」，颜色只用「墨格」的六种语义，不新增。
- **章节编号**：增删小节后检查全文的交叉引用。

## 收工前必须做的事

每次工作结束前按这个顺序更新，缺一项都算没做完。

1. `docs/STATUS.md`：整份改写成现在的状态。它是给下一位接手者的交接说明。
2. `docs/BACKLOG.md`：改对应条目的状态标记。新发现的活加进去，不要只留在对话里。
3. `docs/LOG.md`：在最上面加一条，三到五行。
4. `docs/DECISIONS.md`：人新定了什么，记原话和日期。自己做的取舍也记，标明是谁定的。

## 多 Agent 协作约定

### Claude Code

| 工作包 | 模型 | `effort` | 升级 |
|---|---|---|---|
| 规则明确的定位、清单/文档检查、结构化提取 | Opus 5.5 | `low` | 需上下文/证据冲突升 `medium` |
| 主要开发实现、代码理解、局部修复 | Opus 5.5 | `medium` | 多组件/复杂状态、视觉或安全审查升 `high` |
| 关键架构、核心决策、复杂设计、事务安全、跨页面一致性 | Opus 5.5 | `high`，必要时 `xhigh` | 先缩小问题、补证据 |

显式传 `model: "opus"` 与 `effort`；首次派发让子代理报告模型 ID（应为 `claude-opus-5-5`）。`fork` 忽略 `model` 且沿用主会话强度，需不同强度时改用 `Explore`/`Plan`/`general-purpose`。

### Computer use（通用）

- 操控桌面/GUI（浏览器点击/输入/截图、系统设置、Finder 等）**必须调用本机 Codex CLI 且固定 `gpt-6.1-terra`**；Claude Code 不得用自身 computer-use/浏览器工具代替。
- 思考级别固定 `max`（用户指定）；只观察用 `-s read-only`，需落盘证据用 `-s workspace-write` 并限定证据目录：
  ```sh
  codex exec -m gpt-6.1-terra -c model_reasoning_effort="max" -s workspace-write -C "$PWD" \
    -o <证据目录>/codex-last-message.md "<任务书>"
  ```
- 同时仅一个负责人操作桌面；无头脚本（check/Playwright）不算 computer use。Codex 或该模型不可用时停止并报告。
