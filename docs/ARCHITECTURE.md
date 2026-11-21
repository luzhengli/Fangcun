# 工程架构

代码怎么组织、哪些规则不能破。产品要做什么看 `product-design.md`，这里只讲工程。改了代码结构或规则，同步改这份文档。

2026-11-14 建立（M0-01），同日实现 M0 的只读用例：扫描、体检、生效上下文、全局页、活动、命令行。2026-11-19 M1-04 放开写入：写入的地基有了，各写入用例仍返回「尚未实现」，随 M1 各条补上。

## 总览

```
apps/desktop/src            React 界面
        │  ipc/（唯一通道）
apps/desktop/src-tauri      Tauri 外壳：把内核用例暴露成命令      crates/fangcun-cli   命令行 fangcun
        │                                                              │
        └──────────────────────────┬───────────────────────────────────┘
                          crates/fangcun-core   内核，门面是 Kernel
                                   │
                          adapters/*.toml       各工具的声明式描述，编译时打进内核
```

| 路径 | 内容 |
| --- | --- |
| `crates/fangcun-core/` | 内核库。全部业务逻辑在这里 |
| `crates/fangcun-cli/` | 命令行。解析参数、调内核、打印结果 |
| `apps/desktop/src-tauri/` | 桌面外壳。打开内核、暴露命令、转发事件 |
| `apps/desktop/src/` | React 界面 |
| `apps/desktop/scripts/screens.mjs` | 给应用各页面截图，和原型截图对比 |
| `apps/desktop/scripts/window-shot.mjs` | 给真实的桌面窗口截图，量窗口按钮 |
| `tools/workflow/` | `pnpm workflow`，工作流的状态，见 `docs/WORKFLOW.md` |
| `adapters/` | 适配器，每个工具一份 TOML。格式见 `adapters/README.md` |
| `tools/prototype-preview/` | 原型预览器与同步检查，不属于产品代码 |
| `tools/icons/` | `pnpm icons`，从 `apps/desktop/src-tauri/icons/source/` 的 SVG 出应用图标 |

## 不能破的规则

每条后面是它保护的设计原则。要破例，先问人。

1. **业务逻辑只在内核。** 外壳和命令行只做参数转换和输出。界面能做的事命令行都能做，行为一致（7.1）。
2. **外壳只调 `Kernel`。** 不直接用内核下面的模块。一个用例一个方法。
3. **只写写入范围，写前留快照**（7.5，M1-pre-05）。外壳用 `Kernel::open_configured` 打开内核：默认 `Access::ReadWrite`；`~/.fangcun/settings.toml` 里 `read-only = true` 或命令行 `--read-only` 时 `Access::ReadOnly`，写入用例直接返回 `Error::ReadOnly`，有测试守着，只写 `~/.fangcun/journal`（D-15）。可写时，`materializer::WriteScope` 只认三处：`~/.fangcun`、`~/.fangcun/projects.toml` 里登记的项目（不含 `.git` 下）、完整支持的工具在适配器里声明的、主目录下的全局与个人位置；企业托管目录和 `/etc` 不写。全局层的移除、降级只对已收编的项开放，由各用例在出计划时查。打开内核不创建任何目录。
4. **写入先出计划。** 写入用例一律返回 `ChangePlan`，只有 `Kernel::apply` 落盘。方寸目录以外的文件只有 `materializer` 能改（4.6 第 3、4、5 条）。计划的动作留在内核里，界面拿到的只有编号、标题和每一行；`apply` 只认内核自己出过、还没应用的计划。
5. **内核不写死任何工具的路径或格式。** 一律从适配器查（7.3）。
6. **界面只经 `ipc/` 取数据。** 不给界面加文件、子进程、网络权限。CSP 是 `default-src 'self'`，界面发不出外部请求，零遥测由此保证。原型里的 Google Fonts 不搬进来。
7. **类型从内核单向流到界面。** `pnpm bindings` 把 Rust 类型导出到 `apps/desktop/src/ipc/bindings/`，不手改、不在前端另写一份。
8. **颜色只用「墨格」token。** `styles/tokens.css` 的取值由测试逐项比对原型，组件里不写色值。
9. **密钥的值不过 IPC、不进日志、不进快照。** 用 `SecretValue` 包着，`Debug` 不输出内容。
10. **不用会 panic 的占位。** 没实现的地方返回 `Error::NotImplemented("模块::函数")`，clippy 禁了 `todo!` 和 `unimplemented!`。

## 内核

模块对应设计文档 7.2。上面的可以用下面的，反过来不行。

```
kernel                                           门面
scanner  resolver  global  materializer  watcher   用例层
library  manifest  journal  estimator  secrets
adapter                                    适配器
model  paths  error                        纯类型，不做 IO
```

| 模块 | 职责 | 哪一期实现 |
| --- | --- | --- |
| `model` | 领域类型：资产类型、四层作用域、版本、六种状态语义 | 已有，随用随补 |
| `paths` | `~/.fangcun` 的布局和清单文件名。`FANGCUN_HOME` 可覆盖 | 已有 |
| `adapter` | 适配器的结构与加载，路径展开 | 已有。格式见 `adapters/README.md` |
| `manifest` | `fangcun.toml`、锁、套装文件、`skills-lock.json` 互通 | 清单、个人覆盖、套装、锁的读写已有（M1-07），互通 M1-08 |
| `scanner` | 按适配器收集（`collect`）、全盘扫描（`scan`）、体检发现（`checkup`）、会话记录（`usage`） | 已有 |
| `resolver` | 生效上下文、`why`；写锁 | 都已有，写锁在 `resolver/lock.rs`（M1-07） |
| `global` | 全局页：每项常驻、用到它的项目、状态与建议 | 已有 |
| `estimator` | token 估算 | 已有 |
| `journal` | 活动记录 | 已有。体检记录和 `apply` 的写入记录；写不进去时留在内存里 |
| `registry` | 登记表 `~/.fangcun/projects.toml` | 读已有（M1-04），登记 M1-09 |
| `settings` | 本机设置 `~/.fangcun/settings.toml`：只读模式 | 已有（M1-04） |
| `library` | 内容寻址的正本、索引、来源；`content` 是内容哈希（扫描也用它） | 正本与索引（M1-05）、来源与市场搜索（M1-06）已有，上游检查 M1-16 |
| `materializer` | 计划、应用、快照、回滚、所有权、按键合并 | 骨架已有（M1-04）：`PlanBuilder`、`WriteScope`、`apply`、`snapshot`。所有权与按键合并 M1-10，回滚 M1-11 |
| `watcher` | 监听正本与 `.agents/skills`，有变化时回调（`notify`） | 已有（M1-15） |
| `secrets` | 钥匙串（`security-framework`，服务名 `fangcun`），测试用 `MemoryStore` | 已有（M1-17） |

内核是同步的，不依赖异步运行时。桌面外壳用 `AppState::run` 把每次调用放到阻塞线程。

**扫描缓存**：第一次调只读用例时全盘扫描一次（读近 30 天的会话记录、各工具的全局层、发现的项目），结果存在 `Kernel` 里，后面的用例都读它。`Kernel::rescan` 重新扫描，对应侧栏的「重新体检」。桌面应用的每次扫描、命令行的 `doctor` 记一条体检记录，写进 `~/.fangcun/journal/records.jsonl`；命令行 `status`、`why` 顺带的扫描不记。

**工具分两种**：适配器 `support = "full"` 的（Claude Code、Codex）进项目页、全局页、使用数据，对应 `Scan::tools`；`scan-only` 的只参加体检，`Scan::global`、`Scan::project_items` 按全部工具（`Scan::scanned_tools`）收集。

**项目从哪来**：M0 没有登记。项目是近 30 天会话记录里出现过的工作目录，往上归到 git 根目录；主目录本身、临时目录、主目录下的点目录不算。命令行另外把当前目录加进来。

**测试主目录**：`Kernel::with_user_home` 换掉用户主目录，回归样例用它。

测试写在各模块里。要碰磁盘的测试用 `FangcunHome::at(临时目录)` 和 `with_user_home`，不碰真实的主目录。适配器的回归样例在 `crates/fangcun-core/tests/adapters.rs`，样例目录是 `adapters/fixtures/`。

## 外壳与 IPC

加一个命令改四处，缺一处编译或类型检查会报错：

1. `fangcun-core`：`Kernel` 上加方法；新的数据类型加 `#[derive(TS)] #[ts(export)]`。
2. `apps/desktop/src-tauri/src/commands.rs` 加命令，`main.rs` 的 `generate_handler!` 登记。
3. `pnpm bindings`。
4. `apps/desktop/src/ipc/commands.ts`：`Commands` 加一行，`kernel` 加一个函数。

错误过 IPC 时是 `{ code, message }`。界面按 `code` 分支，`message` 原样展示。

写入命令成对：`plan_*`（`plan_sync`、`plan_init`、`plan_adopt`、`plan_finding`、`plan_rollback`、`plan_rollback_record`）出计划，`apply_plan` 落盘。外壳另有三个不属于内核用例的命令：`pick_directory`（「添加项目…」，外壳用 tauri-plugin-dialog 弹选文件夹面板，capabilities 不开它的权限，界面调不到插件）、`settings` / `save_settings`（只读开关变了时 `AppState::reopen` 换一个新内核）。应用菜单「设置…」（⌘,）发 `open-settings` 事件，界面切到 `/settings`。

内核事件（漂移、活动记录新增）：外壳启动时 `AppState::watch` 按 `Kernel::watch_paths` 开监听，有变化就 `Kernel::refresh_drift`，新的事件以 `kernel-event` 发给界面，`AppShell` 收到后让查询失效；`apply_plan` 之后重设监听目录。启动时另在后台做一次上游检查（`AppState::check_upstream_in_background`）。

菜单栏图标、`⌘K` 命令面板都还没有，实现时在外壳里加。

## 界面

```
src/
  app/          入口：路由表、全局 provider
  shell/        窗口骨架（6.1）：整窗工具栏、侧栏、导航定义
  pages/        每个页面一个目录，对应一张原型画板
  components/   跨页面复用的组件
  ipc/          到内核的唯一通道
    bindings/   内核导出的类型，不手改
    commands.ts 命令表与带类型的调用
    queries.ts  查询 hook 与查询键
    fixtures/   示例数据，照抄原型
  state/        界面自己的状态（zustand）
  styles/       tokens.css（照抄原型）、base.css
```

- **两种状态分开放。** 内核的数据走 TanStack Query，键集中在 `ipc/queries.ts`。只属于窗口的状态（侧栏折叠、待应用的同步计划）放 `state/`。
- **样式用 CSS Modules 加 token。** 不引入第二套设计系统。原型是行内样式，实现时改写成模块样式，尺寸照抄。
- **页面自己摆「列表 + 检查器」。** 骨架只管工具栏和侧栏，因为套装页的左栏和别的页不一样。
- **跨页面的组件先写一次。** `components/` 里已有：检查器 `Inspector`（6.1 统一结构，含 `Section`、`ReadOnlyAction`）、状态 `StatusDot`/`StatusBadge`（圆点加短标签，语气见 `tone.ts`）、层级刻度 `ScopeTicks`、预算条 `BudgetBar`、目标工具 `ToolSwitch`、变更计划弹窗与提示条 `PlanHost`、登记弹窗 `RegisterHost`（后两个挂在 `AppShell` 上，全应用一份）。
- **写入一律走 `state/plan.ts`。** 页面调 `usePlan().preview({ load, done, after?, back?, undoable? })`：`load` 向内核要计划，`PlanHost` 弹窗、应用、让查询失效、出提示条；提示条的「撤销」先出 `plan_rollback_record` 的计划。登记先经 `state/register.ts` 问物化方式。
- **页面的公共版式** 在 `pages/page.module.css`：页头、列表栏、行、芯片、按钮、空状态。
- **还没接上内核用例的写入按钮置灰**，用 `ReadOnlyAction` 写一句为什么。

| 路由 | 页面 | 设计文档 | 原型画板 |
| --- | --- | --- | --- |
| `/projects`、`/projects/:projectId` | `pages/project` | 6.3 | `Main` |
| `/checkup` | `pages/checkup` | 6.2 | `Checkup` |
| `/library` | `pages/library` | 6.4 | `Library` |
| `/bundles` | `pages/bundles` | 6.5 | `Bundles` |
| `/global` | `pages/global` | 6.6 | `Global` |
| `/activity` | `pages/activity` | 6.7 | `Activity` |
| `/settings` | `pages/settings` | 6.1「设置」 | `Settings` |

## 照着原型实现一个页面

界面和内核可以分头做，中间靠类型和示例数据对齐。

1. 读设计文档对应小节，确定要做什么。
2. 看 `prototype/screens/<画板>.png`，再用 `grep` 读 `prototype/project/<画板>.dc.html` 里的结构、文案、示例数据。要点交互时用预览器（`pnpm prototype:preview`）。
3. 在内核里把这一页要的数据类型补全，`pnpm bindings`。
4. 把原型脚本段里的示例数据照抄进 `ipc/fixtures/`。类型对不上时改内核的类型，不改数据。
5. 写页面。`pnpm dev:fixtures` 在浏览器里看。
6. `pnpm screens --compare <画板>`，看 `apps/desktop/screens/compare-<画板>.png` 的「原型 · 应用 · 差异」三栏拼图，直到一致。两边在本机用同一个浏览器渲染，视口、配色、数据相同。示例数据跟上这张画板后，`pnpm workflow accept <画板>`。
7. 在内核里实现真实数据。`pnpm screens --snapshot` 用本机数据截图，`pnpm dev` 在桌面外壳里看。改了窗口外壳（`tauri.conf.json`、工具栏）时跑 `pnpm window-shot`。

对比时已知的环境差异，不算不一致：原型画了红黄绿三个窗口按钮，应用里是系统画的，截图里是空位；原型给滚动条预留了位置，侧栏内容比应用窄约 10px。

示例数据只在 `fixtures` 模式下加载，发布版里没有这段代码。

## 命令

都在仓库根目录运行。

| 命令 | 作用 |
| --- | --- |
| `pnpm install` | 装前端依赖 |
| `pnpm dev` | 桌面应用，连内核 |
| `pnpm dev:fixtures` | 浏览器里看界面，用示例数据 |
| `pnpm cli -- <命令>` | 运行命令行，例如 `pnpm cli -- doctor` |
| `pnpm bindings` | 把内核类型导出到前端 |
| `pnpm screens [画板…]` | 给应用截图，和原型截图对比 |
| `pnpm screens --snapshot target/ipc-snapshot.json [画板…]` | 用本机真实数据截图，写到 `screens/real-<画板>.png`。快照先用 `cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json` 生成，只读 |
| `pnpm screens --compare [画板…]` | 本机重渲原型，写出「原型 · 应用 · 差异」拼图 `screens/compare-<画板>.png`，打印差异占比 |
| `pnpm screens Main@codex …` | 截非默认状态的场景，场景表在 `apps/desktop/scripts/scenes.mjs`：一张画板加几步按文字点击，原型和应用走同样的步骤。`--compare` 另出局部放大对比 `compare-<场景>-focus.png`（默认检查器，弹窗场景是弹窗）。不带参数时截全部画板和场景 |
| `pnpm window-shot [--launch]` | 给真实的桌面窗口截图（`screens/window.png`），放大左上角，量关闭按钮离工具栏按钮中心多少 pt。要终端有「屏幕录制」权限 |
| `pnpm icons` | 从 `icons/source/app-icon.svg`（16 px 一档用 `app-icon-16.svg`）出各尺寸 png、`icon.icns`、`icon.ico`，覆盖 `apps/desktop/src-tauri/icons/`。改图标先改原型的设计系统画板，再改 SVG |
| `pnpm workflow [--fetch]` | 工作流卡在哪：待人处理、没合并的分支、原型待改、落后的示例数据、没复盘的摩擦。`accept <画板…>` 记下示例数据已跟上 |
| `pnpm prototype:preview` | 起本地服务器看原型，打开 http://localhost:8000/tools/prototype-preview/ |
| `pnpm prototype:check` | 原型同步后的检查，重新生成 `prototype/screens/` |
| `pnpm check` | 提交前跑：格式、lint、类型检查、全部测试、前端构建 |
| `pnpm format` | 自动格式化 |
| `pnpm build` | 打包桌面应用 |

环境：Rust stable（1.85 以上，用到 2024 edition）、Node 24、pnpm 11。截图和原型检查要 Playwright 的浏览器：`pnpm exec playwright install chromium --only-shell`。

CI 在 `.github/workflows/ci.yml`：跑 `pnpm check`，检查类型绑定与内核一致，检查原型能渲染；示例数据落后于原型时留警告，不失败。

## 实现要点

实现时容易踩错、又不在代码里一眼看得出的事。原来在 STATUS 的「接手须知」里，2026-11-14 挪到这里（W-04）。

- **写一个写入用例**：`PlanBuilder::new(标题, 记录类型, 快照范围)`，逐个 `write_file`、`symlink`、`remove`，`Kernel::issue` 出计划。每加一个路径就记下它现在的样子，`apply` 前再核对，变了返回 `Error::StalePlan`。缺的上层目录自动补成建目录的动作，进快照，回滚时一起删。一份计划里一个路径只出现一次；删目录树要先逐个删里面的。登记项目的计划用 `registers` 把项目算进写入范围。
- **资产库**：正本在 `~/.fangcun/library/objects/<哈希>`，skill 存的是参与哈希的那些文件（`.git`、`__pycache__`、`metadata.json` 不存，软链存成普通文件，保留权限），所以正本的哈希就是源目录的哈希。索引是 SQLite `~/.fangcun/index.sqlite`（7.2），写入用 `BEGIN IMMEDIATE`、`busy_timeout` 5 秒；表结构版本在 `PRAGMA user_version`，改表时在 `library/index.rs` 的 `migrate` 里补升级。来源未知存空字符串。资产库写的是方寸自己的目录，不经变更计划，由写入用例在 `apply` 时调用。「被哪些项目使用」不进索引，从锁算。
- **来源**（`library::source`）：`Sources::resolve_skill` 把一项 skill 解析到提交、标签、仓库内路径并存进资产库；`match_installed` 找本机一份已装的 skill 对应上游哪个提交。kernel 用的 `Sources` 开了 `fetching_once`，一个用例里同一个仓库只拉一次。git 和 `curl` 都调本机的（私有仓库走用户自己的凭据），git 关了交互提示。GitHub 裸仓库缓存在 `~/.fangcun/git/github.com/<owner>/<repo>.git`，每次解析先 `fetch`。`pin` 先当标签再当提交（至少 4 位十六进制）；`track` 取 `refs/heads/<分支>`；都没写取 `HEAD`。本地来源没写版本时取工作目录现在的样子，`commit` 为空。测试用 `with_github_base("file://…")` 冒充 GitHub。
- **市场**（`library::market`）：`search` 调 `https://skills.sh/api/search?q=`，只留 `owner/repo` 形式的来源，落成 `github:`。
- **清单与锁**：`manifest::load_project` 读清单、个人覆盖、引用的套装，并校验每份文件只写自己的节。`Project::team_declarations` 是写锁用的（套装 + 清单），`local_declarations` 再叠个人覆盖（同步用）。`resolver::resolve_lock` 对约束没变的项沿用锁里的版本、不联网；新增或换了约束的经 `Sources` 解析，解析时就把那一份存进资产库（资产库是方寸自己的缓存，不算写用户文件）。锁的文本按键排序，同样的锁总是同样的字节，所以「锁已是最新」就是计划里没有变更。
- **登记与反推**（`Kernel::plan_init`、`resolver::infer`）：一份计划写清单、锁、登记表三个文件；已有清单的只登记、缺锁补锁。反推出的 skill 先解析好，作为「上一份锁」交给 `resolve_lock`，约束没变就原样沿用，所以登记后 `fangcun lock` 是「已是最新」。查不出来源、做不到的写进 `ChangePlan.notes`，不报错。`skills-lock.json` 的位置从适配器查（项目层 skill 目录的 `lock`）。扫描时把登记表里的项目也扫进来；可写时带 `fangcun.toml` 的项目直接登记（`registry::register_found`）。
- **`skills-lock.json` 互通**（`manifest::interop`）：v1 哈希是旧算法（`content::legacy_hash_dir`，路径按 `localeCompare` 排），读的时候对已安装的目录校验，对得上才换算成 v2，和 CLI 一样。导出照 CLI 写项目锁的字段顺序。边界样例在 `library::hash_samples`，`content.rs` 的测试和 CI 的比对（W-13）共用，改了两边一起变。
- **同步**（`materializer::sync`）：先查锁与清单一致（`resolver::lock_mismatches`），再按台账（`materializer::ledger`，`~/.fangcun/state/projects/<键>.toml`）判断哪些是自己的。整棵目录的增删用 `PlanBuilder::set_quiet` 只列一行。目录由软链换成实体目录时，下面的路径是「推出来的」（`Action::implied`）：不核对、不进快照，回滚和失败还原时由上级目录的快照整棵还原。MCP 只比较 command、args、env 三个字段，别的字段保留。
- **回滚**（`materializer::rollback`）：快照的每个路径记着写入前（`state`）和写入后（`after`）。回滚把每个范围的快照倒着还原；现状和 `after` 不同就挡住计划。记录只追加，`Journal::read` 从新到旧扫一遍，被未撤销的回滚记录 `reverts` 指着的算「已回滚」。时间只精确到秒，同一秒的按写入先后排。
- **收编**（`materializer::adopt`）：同名副本按 `real_dir` 去重，只换 `link_target` 为空、在写入范围里的；台账 `state/adopted.toml`。
- **命令行应用计划**：`commands.rs` 的 `confirm_and_apply` 先打印计划再问，`--yes` 不问，不在终端里又没加 `--yes` 只打印。新的写入命令都走它。
- **快照**保留期在 `settings.toml` 的 `snapshot-days`、`snapshot-keep`，`Kernel::apply` 之后清理。按真实路径分范围：同一个项目经软链的不同写法是同一个范围（macOS 的 `/var` 是 `/private/var`）。写到一半失败时按快照倒着还原。应用后扫描缓存不自动清，要重扫由用例决定。

- **适配器**格式和现状在 `adapters/README.md`。改了适配器跑 `cargo test -p fangcun-core --test adapters`。没核实、没读的位置写在各适配器文件开头，不要凭记忆补。Trae 用的是二手来源，待核实。
- **同名优先级按工具真实规则**（D-19，4.3）。工具规则与方寸相反时，物化不自动移动上层同名项，标「同名冲突」由人选。Claude Code 的 skill 全局优先，MCP 个人 > 项目 > 全局、整条取用；Codex 的同名 skill 都加载，MCP 项目层赢、按字段合并，未受信任项目的 `.codex/config.toml` 不读（后两条是实测）。界面上「谁覆盖谁」的小字在规则相反时会改写。
- **全局页的同名**：项目层真正生效（MCP）是「被覆盖」；全局那份照样生效（Claude Code 的 skill）或两份都加载（Codex 的 skill）是「同名冲突」，见 `global.rs` 的 `same_name_in`。
- **只读扫描的工具**只参加体检，不进项目页、全局页的工具切换。
- **MCP 常驻不估算**，预算条 MCP 段为 0，旁边写「N 个未估算」。
- **体检记录**：从来没有体检记录时才算「首次体检」。测试一律用临时目录，不碰真实的 `~/.fangcun`；`ipc_snapshot` 会在真实的 `~/.fangcun` 记一条，不想要就设 `FANGCUN_HOME`。
- **项目预算**读 `fangcun.toml` 的 `[project].budget`，M0 只读这一个键，别的键不报错；值写错了会报出来。
- **示例数据**在 `apps/desktop/src/ipc/fixtures/`，按画板分文件，照抄原型。原型和真实内核的差别（例如全局指令的「6 / 6」、MCP 有常驻数）保留在示例数据里，以便和原型截图对比。各画板的示例数据有几处对不上（全局层有什么、tdd 与 project-harness 的差异行数等），文档各小节照各自画板写，没有强行统一。和原型对齐到哪一版记在 `prototype-sync.json`，用 `pnpm workflow accept` 改。
- **看真实数据**：`cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json`，再 `pnpm screens --snapshot target/ipc-snapshot.json`，图在 `apps/desktop/screens/real-*.png`。
- **使用数据**：Claude Code 认 `Skill` 工具调用、用户输入的 `/命令`、`mcp__服务__工具`；Codex 认命令里读了 `…/skills/<名称>/SKILL.md`（插件缓存里的不算）和 `mcp__` 调用。Codex 的 MCP 调用本机没有样本。
- **计划只留最近 64 份**：界面每次预览都在内核里出一份计划（`Kernel::issue`），旧的被挤掉后应用会报「计划不存在」，重新预览即可。「同步 · N」的查询不自动重取，写入后才失效。
- **历史页签的「当前磁盘状态」**是最新一条已应用、不是回滚的写入（`journal::history_of`）；「回滚到此」调 `plan_rollback(项目, 快照)`，即撤销比它新的写入。
- **体检的一键动作**按发现编号分派（`Kernel::plan_finding`），能用的由 `scanner::finding_ready` 决定，界面只看 `Finding.ready`。
- **设置**存在 `~/.fangcun/settings.toml`，`Settings::save` 不经变更计划（方寸自己的文件），只读模式下也能存。
- **快照按范围各自编号**，跨项目操作每个项目各留一个快照（M1）。
- **按项目停用**（M1-18，`materializer/sync/disable.rs`）：写哪儿全看适配器，`[[skill-switches]]`（`list` 数组或 `map` 对象两种形状）和 `[[mcp-switches]]`（名字表）；都没有时 MCP 写项目层服务表的 `enabled` 字段（要按字段合并）。台账的 `OwnedKey.kind` 区分 MCP 服务表的键和开关，撤销只撤值还是方寸写的那个。读的一侧：项目层、个人层的开关存在 `Collected.project_switches`、`mcp_off`，`Collected::with_project_switches` 再作用到全局层；按字段合并时只写了 `enabled` 的那条是开关，折到下面那份上。
- **同一个配置文件只写一次**：同步里 MCP 和停用都会改 `.codex/config.toml`，`PlanBuilder` 同一路径写两次会互相覆盖。改动先攒在 `Run.docs`（`take_doc`/`put_doc`），`flush_docs` 统一写。
- **界面暂存的清单改动**（6.8）：`state/staged.ts` 按项目存 `ManifestEdit`，「同步 · N」把它们交给 `plan_sync`（内核 `plan_sync_with`），清单用 `toml_edit` 改（`manifest::apply_edits`），和同步一份计划。能不能停用由内核填 `EffectiveItem.disable`，界面不判断。
- **密钥**（M1-17）：内核的测试构建默认用内存存放处，不碰真实钥匙串；要指定用 `Kernel::with_secret_store`。同步时提交的配置按适配器 `env-reference` / `env-forward` 写引用，明文只写个人层（台账 `mcp-personal` 记位置和「变量 → 密钥」）。`apply` 存快照前按全部台账把这些明文换成 `secrets::marker`，回滚用 `snapshot::content` 从钥匙串补回。计划里存钥匙串用 `PlanBuilder::store_secret`，应用时先存再写文件，各记一条密钥记录。
- **检查器的台账数据**（M1-31 至 33）：`kernel/item_state.rs` 给生效上下文填 `files`（台账）、`recent`（活动记录）、`secrets`；`kernel/manifest_view.rs` 出清单页签。
- **漂移**（M1-15）：`materializer::drift::detect` 按台账判断（入库比实体目录，链接比正本目录名）。`Kernel::plan_drift` 的三种处理都走 `sync_into`（分叉传 `forked`，同步留着那一份、不再记账）。整目录改写用 `drift::replace_dir`，只列一行。回流用 `Sources::prepare_promote` / `finish_promote`，计划体里的 `promotions` 在写完文件后执行。待处理记录带 `subject`，`Kernel::log` 读的时候按现状结算（`settle_pending`），侧栏待处理数也走它。
- **更新**（M1-16）：`Kernel::check_upstream` 写 `~/.fangcun/state/upstream.toml`，新版本各记一条待处理；`still_behind` 用锁里的哈希判断还要不要升。升级走 `apply_upgrades`（改 pin 和锁）再 `sync_into`；界面暂存的是 `ManifestEdit::Upgrade`，`plan_sync_with` 里处理，勾选的其他项目用 `upgrade_into`。差异在 `library::diff`，脚本按路径、可执行位、`#!`、扩展名判断。
- **窗口按钮**：`tauri.conf.json` 的 `trafficLightPosition.y` 按 tao 的算法，按钮中心约在 y + 1.75pt；对齐的目标是工具栏里后退、侧栏按钮的中心，实测 19.25pt（几何中线 19.5pt），见 DECISIONS「窗口按钮的位置」。用 `pnpm window-shot` 量。

## 还没定的工程选型

到那一步再定，定了记进 `DECISIONS.md`。

| 事项 | 用在哪 |
| --- | --- |
| SQLite 的库 | `library` 的索引 |
| git 的库，或直接调 `git` | `library` 的来源缓存 |
| ~~文件监听的库~~ | 已定：`notify`（FSEvents），见 DECISIONS 2026-11-21 批 5 |
| ~~钥匙串的库~~ | 已定：`security-framework`，见 DECISIONS 2026-11-21 批 4 |
| ~~token 估算的口径~~ | 已定：ASCII 每 4 字符 1 个，其余每字符 1 个，见 `estimator` |
| ~~内容哈希的算法~~ | 已定：SHA-256 十六进制，见 `scanner::fsutil::hash_dir` |
| 保留注释地改写 TOML 的库 | `manifest` 的写回 |
| ~~时间库~~ | 已定：不引入，`model::Timestamp` 自己换算 UTC |

需要人定的事在 `BACKLOG.md` 的「要人定」里。
