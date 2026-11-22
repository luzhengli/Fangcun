---
name: self-harness
description: >-
  管理一个项目的 Agent harness（让 Agent 自主推进而质量不降的那套文档、规则、脚本、判官与流程）的完整生命周期：
  初始化最小骨架、开工看状态、收工交接、审计（常驻上下文、规则档位、判官缺口、断链）、度量、复盘与进化。
  用户说「开工」「看状态」「收工」「复盘」「审计 harness」「初始化 harness」「记一条摩擦」，
  或 Agent 在流程上卡住、同一类错犯第二次、文档越写越长时使用。与平台无关（Claude Code、Codex 都能用），与项目形态无关。
metadata:
  author: luyao
  version: "0.1.1"
---

# self-harness

harness 是项目里让 Agent 能自主干活、质量又不掉的那一层：意图、约束、状态、协议、能力、反馈、自我改进。这个 skill 不替项目写 harness，而是给它一套**由证据驱动生长**的方法和几个脚本。

三条主张：

1. **自主度取决于判官，不取决于文档。** 哪个质量维度有自动判官，Agent 在那里就能放手。先列判官，再写规则（`references/oracles.md`）。
2. **规则往上升档。** 同一条要求，写不出来 > 做错就拦 > 适时提醒 > 按需加载 > 常驻 > 查询文档。被违反过的规则至少升一档；从没起过作用的删掉（`references/rules.md`）。
3. **从最小开始，由摩擦驱动生长。** 不预填模板。每加一样东西，都要有一条摩擦或一次违反作为证据（`references/evolve.md`）。

## 项目侧的数据

harness 本身是数据，脚本才能审计和度量。都在项目根目录的 `.harness/` 下：

| 文件 | 内容 |
| --- | --- |
| `config.json` | 入口文件、状态、待办、决定、摩擦记录各在哪，检查命令，端口，边界词，预算。没有时脚本自己探测 |
| `rules.json` | 规则清单：每条在第几档、谁在守、被违反过几次、下一步升到哪 |
| `oracles.json` | 判官清单：每个质量维度由谁判、几档、缺口怎么补 |
| `metrics.jsonl` | `metrics --record` 记的快照，只追加 |

项目原有的文件（AGENTS.md、PLAN.md、tasks.json、BACKLOG.md……）不搬家，在 `config.json` 里指过去即可。

## 口令

脚本：`node <本 skill 目录>/scripts/harness.mjs <命令> [--root <项目>]`。下文简写成 `harness <命令>`。

| 人说 | 做什么 |
| --- | --- |
| 初始化 harness | 读 `references/bootstrap.md`。先 `harness init` 看探测结果，人点头再 `--write` |
| 开工 / 看状态 | `harness status --fetch`，按输出汇报，给建议的下一步。再读项目的状态文件里手写的判断 |
| 收工 | 见下面「收工」 |
| 审计 harness | `harness audit`，按结论逐条给建议。要细看时读 `references/context.md`、`rules.md`、`oracles.md` |
| 度量 | `harness metrics --record`，和 `.harness/metrics.jsonl` 里上一次比。口径见 `references/metrics.md` |
| 复盘 | 读 `references/evolve.md` |
| 定没定过 | `harness find <关键词…>`。问人之前、做和既定方案不一样的取舍之前先查 |

## 收工

1. 跑项目的检查命令（`config.json` 的 `check`），没过不收工。
2. 状态文件只写人话判断：下一步、卡在哪、为什么。能由 `harness status` 算出来的（分支、PR、计数）不写。
3. 待办改状态，新发现的活写进去。不要只留在对话里。
4. 人新定的事记进决定文件，带原话和日期；Agent 自己的取舍也记，标「Agent 定」。之后被人推翻的，在原条目后补「推翻」二字，度量靠它。
5. 这次在流程上卡过的地方，记一条摩擦（格式见 `references/evolve.md`）。

## 平台差异

skill 放在哪、hook 怎么装、子代理和权限怎么用：`adapters/claude-code.md`、`adapters/codex.md`。平台缺某种能力时降一档实现，不要硬凑。

## 这个 skill 自己的进化

- 用的时候卡住了，先分清是**项目问题**（项目的工具、文档不够）还是**方法问题**（这里的方法、脚本、口径不对）。方法问题记进本目录的 `FRICTION.md`，不要直接改 SKILL.md。
- 改方法时更新 `CHANGELOG.md`，写清楚证据来自哪个项目的哪条摩擦。版本号写在两处：frontmatter 的 `metadata.version` 和 `CHANGELOG.md` 最新一节，`harness check` 会比对。
- 只在一个项目里出现过的做法，不进这里。至少两个项目都用得上，才算方法。
