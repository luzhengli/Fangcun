# 工程架构

代码怎么组织、哪些规则不能破。产品要做什么看 `product-design.md`，这里只讲工程。改了代码结构或规则，同步改这份文档。

2026-11-14 建立（M0-01），同日实现 M0 的只读用例：扫描、体检、生效上下文、全局页、活动、命令行。写入用例仍返回「当前是只读模式」。

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
| `adapters/` | 适配器，每个工具一份 TOML。格式见 `adapters/README.md` |
| `tools/prototype-preview/` | 原型预览器与同步检查，不属于产品代码 |

## 不能破的规则

每条后面是它保护的设计原则。要破例，先问人。

1. **业务逻辑只在内核。** 外壳和命令行只做参数转换和输出。界面能做的事命令行都能做，行为一致（7.1）。
2. **外壳只调 `Kernel`。** 不直接用内核下面的模块。一个用例一个方法。
3. **M0 只读。** 外壳用 `Access::ReadOnly` 打开内核，写入用例直接返回 `Error::ReadOnly`，有测试守着。打开内核不创建任何目录。
4. **写入先出计划。** 写入用例一律返回 `ChangePlan`，只有 `Kernel::apply` 落盘。方寸目录以外的文件只有 `materializer` 能改（4.6 第 3、4、5 条）。
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
| `manifest` | `fangcun.toml`、锁、套装文件、`skills-lock.json` 互通 | 清单解析已有，其余 M1 |
| `scanner` | 按适配器收集（`collect`）、全盘扫描（`scan`）、体检发现（`checkup`）、会话记录（`usage`） | 已有 |
| `resolver` | 生效上下文、`why`；写锁 | 前两项已有，写锁 M1 |
| `global` | 全局页：每项常驻、用到它的项目、状态与建议 | 已有 |
| `estimator` | token 估算 | 已有 |
| `journal` | 活动记录 | 读已有；M0 的记录只在内存里，写入 M1 |
| `library` | 内容寻址的正本、索引、来源 | M1 |
| `materializer` | 计划、应用、快照、回滚、所有权、按键合并 | M1 |
| `watcher` | 监听漂移，向外壳推事件 | M1 |
| `secrets` | 钥匙串 | M1 |

内核是同步的，不依赖异步运行时。桌面外壳用 `AppState::run` 把每次调用放到阻塞线程。

**扫描缓存**：第一次调只读用例时全盘扫描一次（读近 30 天的会话记录、各工具的全局层、发现的项目），结果存在 `Kernel` 里，后面的用例都读它。`Kernel::rescan` 重新扫描，对应侧栏的「重新体检」。每次扫描在内存里记一条体检记录，不落盘。

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

内核事件（漂移、活动记录新增）的转发还没接：`watcher::EventSink` 已定义，M1 在外壳里实现它，用 Tauri 事件发给界面，界面收到后让 `ipc/queries.ts` 里对应的查询失效。

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
- **跨页面的组件先写一次。** `components/` 里已有：检查器 `Inspector`（6.1 统一结构，含 `Section`、`ReadOnlyAction`）、状态 `StatusDot`/`StatusBadge`（圆点加短标签，语气见 `tone.ts`）、层级刻度 `ScopeTicks`、预算条 `BudgetBar`、目标工具 `ToolSwitch`。变更计划弹窗（6.8）在 M1 第一次用到时加。
- **页面的公共版式** 在 `pages/page.module.css`：页头、列表栏、行、芯片、按钮、空状态。
- **M0 的写入按钮一律置灰**，旁边写「M0 只读，在 M1 开放」。

| 路由 | 页面 | 设计文档 | 原型画板 |
| --- | --- | --- | --- |
| `/projects`、`/projects/:projectId` | `pages/project` | 6.3 | `Main` |
| `/checkup` | `pages/checkup` | 6.2 | `Checkup` |
| `/library` | `pages/library` | 6.4 | `Library` |
| `/bundles` | `pages/bundles` | 6.5 | `Bundles` |
| `/global` | `pages/global` | 6.6 | `Global` |
| `/activity` | `pages/activity` | 6.7 | `Activity` |

## 照着原型实现一个页面

界面和内核可以分头做，中间靠类型和示例数据对齐。

1. 读设计文档对应小节，确定要做什么。
2. 看 `prototype/screens/<画板>.png`，再用 `grep` 读 `prototype/project/<画板>.dc.html` 里的结构、文案、示例数据。要点交互时用预览器（`pnpm prototype:preview`）。
3. 在内核里把这一页要的数据类型补全，`pnpm bindings`。
4. 把原型脚本段里的示例数据照抄进 `ipc/fixtures/`。类型对不上时改内核的类型，不改数据。
5. 写页面。`pnpm dev:fixtures` 在浏览器里看。
6. `pnpm screens <画板>`，把 `apps/desktop/screens/<画板>.png` 和 `prototype/screens/<画板>.png` 对着看，直到一致。两张图的视口、配色、数据相同。
7. 在内核里实现真实数据。`pnpm dev` 在桌面外壳里看。

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
| `pnpm prototype:preview` | 起本地服务器看原型，打开 http://localhost:8000/tools/prototype-preview/ |
| `pnpm prototype:check` | 原型同步后的检查，重新生成 `prototype/screens/` |
| `pnpm check` | 提交前跑：格式、lint、类型检查、全部测试、前端构建 |
| `pnpm format` | 自动格式化 |
| `pnpm build` | 打包桌面应用 |

环境：Rust stable（1.85 以上，用到 2024 edition）、Node 24、pnpm 11。截图和原型检查要 Playwright 的浏览器：`pnpm exec playwright install chromium --only-shell`。

CI 在 `.github/workflows/ci.yml`：跑 `pnpm check`，检查类型绑定与内核一致，检查原型能渲染。

## 还没定的工程选型

到那一步再定，定了记进 `DECISIONS.md`。

| 事项 | 用在哪 |
| --- | --- |
| SQLite 的库 | `library` 的索引 |
| git 的库，或直接调 `git` | `library` 的来源缓存 |
| 文件监听的库 | `watcher` |
| 钥匙串的库 | `secrets` |
| ~~token 估算的口径~~ | 已定：ASCII 每 4 字符 1 个，其余每字符 1 个，见 `estimator` |
| ~~内容哈希的算法~~ | 已定：SHA-256 十六进制，见 `scanner::fsutil::hash_dir` |
| 保留注释地改写 TOML 的库 | `manifest` 的写回 |
| ~~时间库~~ | 已定：不引入，`model::Timestamp` 自己换算 UTC |

需要人定的事在 `BACKLOG.md` 的「要人定」里。
