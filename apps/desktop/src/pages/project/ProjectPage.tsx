import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Navigate, useParams } from "react-router";
import { BudgetBar } from "@/components/BudgetBar";
import { CommandHint, Inspector, Section } from "@/components/Inspector";
import { SCOPE_NAMES, SCOPES, ScopeTicks } from "@/components/ScopeTicks";
import { StatusBadge, StatusDot } from "@/components/StatusDot";
import { ToolSwitch } from "@/components/ToolSwitch";
import { formatK, formatLimit, toneOfState } from "@/components/tone";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { EffectiveContext } from "@/ipc/bindings/EffectiveContext";
import type { EffectiveItem } from "@/ipc/bindings/EffectiveItem";
import type { ObservedKind } from "@/ipc/bindings/ObservedKind";
import { kernel } from "@/ipc/commands";
import { useEffectiveContext, useHistory, useProjects, useSyncPlan, useTools } from "@/ipc/queries";
import { projectPath } from "@/shell/nav";
import { usePlan } from "@/state/plan";
import { useRegister } from "@/state/register";
import { useUi } from "@/state/ui";
import page from "../page.module.css";
import styles from "./ProjectPage.module.css";

const GROUPS: { kind: AssetKind; label: string }[] = [
  { kind: "instruction", label: "指令" },
  { kind: "skill", label: "Skills" },
  { kind: "mcp", label: "MCP" },
];

const KIND_LABEL: Record<AssetKind, string> = {
  instruction: "指令文件",
  skill: "Skill",
  mcp: "MCP 服务",
};

const OBSERVED_LABEL: Record<ObservedKind, string> = {
  plugin: "插件",
  hook: "Hook",
  "slash-command": "斜杠命令",
  subagent: "子代理",
};

type Tab = "effective" | "manifest" | "history";

/** 项目页：生效上下文、清单、历史三个页签。方寸的招牌界面（6.3）。 */
export function ProjectPage() {
  const { projectId } = useParams();
  const projects = useProjects();

  if (projects.isError || !projects.data) {
    return (
      <div className={page.page}>
        <header className={page.header}>
          <h1 className={page.title}>项目</h1>
        </header>
        {projects.isError ? (
          <p className={page.error}>{projects.error.message}</p>
        ) : (
          <p className={page.empty}>正在读取项目…</p>
        )}
      </div>
    );
  }
  if (projects.data.length === 0) {
    return (
      <div className={page.empty}>
        <h1 className={page.title}>项目</h1>
        <p>
          近 30 天的会话记录里没有发现项目。在项目目录里用过 Claude Code 或 Codex
          之后，重新体检就能看到。
        </p>
      </div>
    );
  }
  const first = projects.data[0];
  if (!projectId && first) return <Navigate to={projectPath(first.id)} replace />;
  const root = projectId ? decodeURIComponent(projectId) : undefined;
  return <ProjectView key={root} root={root} />;
}

function ProjectView({ root }: { root: string | undefined }) {
  const tool = useUi((s) => s.tool);
  const setTool = useUi((s) => s.setTool);
  const context = useEffectiveContext(root, tool);
  const summary = useProjects().data?.find((p) => p.id === root);
  const registered = summary?.registered ?? false;
  const syncPlan = useSyncPlan(root, registered);
  const preview = usePlan((s) => s.preview);
  const openSync = () =>
    root &&
    preview({
      load: () => kernel.planSync(root),
      done: (applied, plan) =>
        `已同步 ${plan.changes.length} 处变更 · 快照 ${applied.snapshots.map((s) => `#${s}`).join("、")}`,
    });
  const [tab, setTab] = useState<Tab>("effective");
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);

  const data = context.data;
  const items = data?.items ?? [];
  // 未登记的目录默认看项目本身（检查器里有「登记」）；点了某一行才看那一项。
  const current =
    items.find((i) => i.id === selected) ??
    (registered ? (items.find((i) => i.state === "drifted") ?? items[0]) : undefined);
  const tools = useTools();
  const toolName = tools.data?.find((t) => t.id === tool)?.name ?? tool;
  const onlyGlobal = items.length > 0 && items.every((i) => i.scope === "global");

  return (
    <div className={page.page}>
      <header className={page.header}>
        <div className={page.titleGroup}>
          <h1 className={page.title}>{data?.projectName ?? "…"}</h1>
          <span className={page.path}>{data?.projectPath}</span>
        </div>
        {summary && !registered && <StatusBadge tone="muted" label="未登记" />}
        <ToolSwitch value={tool} onChange={setTool} />
        <button type="button" className={styles.palette} disabled title="命令面板还没有实现">
          <span>搜索资产与动作</span>
          <span className={styles.kbd}>⌘K</span>
        </button>
        {registered && (
          <button
            type="button"
            className={page.buttonPrimary}
            onClick={openSync}
            title="按清单与锁物化，先出变更计划"
          >
            同步 <span className={styles.count}>· {syncPlan.data?.changes.length ?? "…"}</span>
          </button>
        )}
      </header>

      {data && (
        <div className={page.band}>
          <BudgetBar
            budget={data.budget}
            lead={
              <>
                <span className={styles.budgetLead}>常驻上下文</span>
                <span className={styles.budgetTotal}>
                  {formatK(data.budget.instructions + data.budget.skills + data.budget.mcp)}
                </span>
                <span className={styles.budgetLimit}>
                  / {registered ? "项目预算" : "默认预算"} {formatLimit(data.budget.limit)} tokens ·
                  估算值
                </span>
              </>
            }
          />
        </div>
      )}

      {registered && (
        <div role="tablist" aria-label="项目视图" className={page.tabs}>
          {(
            [
              ["effective", "生效上下文"],
              ["manifest", "清单"],
              ["history", "历史"],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={tab === key}
              onClick={() => setTab(key)}
            >
              {label}
            </button>
          ))}
        </div>
      )}

      <div className={page.split}>
        <section className={page.list}>
          {context.isError && <p className={page.error}>{context.error.message}</p>}
          {!data && !context.isError && <p className={page.empty}>正在解析生效上下文…</p>}
          {data && !registered && (
            <p className={styles.recentNote}>
              {items.length === 0
                ? `未登记：${toolName} 在这个目录没有加载任何 skill、MCP 与指令文件。`
                : `未登记：下面是 ${toolName} 在这个目录实际加载的内容，只读。${
                    onlyGlobal ? "项目层没有发现资产，全是全局层带进来的。" : ""
                  }`}
            </p>
          )}
          {data && (tab === "effective" || !registered) && (
            <EffectiveList
              context={data}
              selectedId={current?.id}
              onPick={(id) => {
                setSelected(id);
                setInspectorOpen(true);
              }}
            />
          )}
          {data && registered && tab === "manifest" && <ManifestTab context={data} />}
          {data && registered && tab === "history" && root && <HistoryTab root={root} />}
        </section>
        {!registered && inspectorOpen && !current && data && root && (
          <UnregisteredInspector
            root={root}
            context={data}
            onlyGlobal={onlyGlobal}
            onClose={() => setInspectorOpen(false)}
          />
        )}
        {(tab === "effective" || !registered) && inspectorOpen && current && data && (
          <ItemInspector
            item={current}
            context={data}
            onPick={setSelected}
            onClose={() => setInspectorOpen(false)}
          />
        )}
      </div>
    </div>
  );
}

function EffectiveList({
  context,
  selectedId,
  onPick,
}: {
  context: EffectiveContext;
  selectedId: string | undefined;
  onPick: (id: string) => void;
}) {
  const missing = context.observedMissing.map((k) => OBSERVED_LABEL[k]);
  return (
    <div>
      <div className={page.colHead}>
        <span className={styles.colName}>名称 · 版本</span>
        <span className={styles.colLayer}>作用层</span>
        <span className={styles.colStatus}>状态</span>
        <span className={`${styles.colCost} ${page.cost}`}>常驻</span>
      </div>
      {GROUPS.map(({ kind, label }) => {
        const rows = context.items.filter((i) => i.kind === kind);
        if (rows.length === 0) return null;
        const counted = rows.filter((r) => r.counted);
        const cost = counted.reduce((n, r) => n + (r.resident ?? 0), 0);
        const unestimated = counted.some((r) => r.resident === null);
        return (
          <div key={kind}>
            <h2 className={page.groupHead}>
              <span>{label}</span>
              <span className={page.groupCost}>
                常驻 {unestimated && cost === 0 ? "未估算" : formatK(cost)}
              </span>
            </h2>
            {rows.map((item) => (
              <button
                key={item.id}
                type="button"
                className={page.row}
                aria-pressed={item.id === selectedId}
                onClick={() => onPick(item.id)}
              >
                <span className={styles.colName}>
                  <span
                    className={`${page.name} ${item.state === "overridden" ? page.struck : ""}`}
                  >
                    {item.name}
                  </span>
                  {item.version && <span className={page.version}>{item.version}</span>}
                </span>
                <span className={styles.colLayer}>
                  <ScopeTicks scope={item.scope} />
                  <span>{SCOPE_NAMES[item.scope]}</span>
                </span>
                <span className={styles.colStatus}>
                  <StatusDot
                    tone={item.state ? toneOfState(item.state) : "neutral"}
                    label={item.label}
                  />
                </span>
                <span className={`${styles.colCost} ${page.cost}`}>
                  {!item.counted ? "–" : item.resident === null ? "—" : formatK(item.resident)}
                </span>
              </button>
            ))}
          </div>
        );
      })}
      <div>
        <h2 className={page.groupHead} style={{ justifyContent: "flex-start" }}>
          其他加载项<span className={styles.tag}>只看不管</span>
        </h2>
        <p className={styles.othersIntro}>
          插件、Hook、斜杠命令、子代理也会被 Agent
          加载。列在这里是为了看清全貌；当前版本方寸不修改它们，也不计入上方预算。
        </p>
        {context.observed.map((o) => (
          <div key={`${o.kind}:${o.scope}:${o.name}`} className={page.row}>
            <span className={styles.colName}>
              <span className={styles.kindTag}>{OBSERVED_LABEL[o.kind]}</span>
              <span className={page.name}>{o.name}</span>
            </span>
            <span className={styles.colLayer}>
              <ScopeTicks scope={o.scope} />
              <span>{SCOPE_NAMES[o.scope]}</span>
            </span>
            <span className={styles.colStatus}>
              <StatusDot tone="muted" label={o.note ?? "—"} />
            </span>
            <span className={`${styles.colCost} ${page.cost}`}>–</span>
          </div>
        ))}
        {missing.length > 0 && <p className={page.note}>未发现{missing.join("、")}。</p>}
      </div>
    </div>
  );
}

function ItemInspector({
  item,
  context,
  onPick,
  onClose,
}: {
  item: EffectiveItem;
  context: EffectiveContext;
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  const tools = useTools();
  const toolName = tools.data?.find((t) => t.id === context.tool)?.name ?? context.tool;
  const total = context.budget.instructions + context.budget.skills + context.budget.mcp;
  const share =
    item.counted && item.resident !== null && total > 0
      ? Math.round((item.resident / total) * 100)
      : 0;
  const shareText =
    !item.counted || item.resident === null
      ? item.kind === "mcp" && item.counted
        ? "未估算：要启动服务才能读到工具定义"
        : "不占用常驻上下文"
      : `${formatK(item.resident)} · 占本项目常驻 ${share}%`;
  const effective = item.declared.find((d) => d.effective);
  const shadowedBelow = item.declared.some(
    (d) =>
      d.location &&
      !d.effective &&
      effective &&
      SCOPES.indexOf(d.scope) > SCOPES.indexOf(effective.scope),
  );
  const fields = [
    { k: "作用层", v: SCOPE_NAMES[item.scope] },
    {
      k: "常驻",
      v: !item.counted
        ? "不计入"
        : item.resident === null
          ? "未估算"
          : `${formatK(item.resident)} tokens（估算）`,
    },
    { k: "位置", v: item.location, mono: true },
    ...(item.linkTarget ? [{ k: "链接到", v: item.linkTarget, mono: true }] : []),
    ...(item.kind === "skill"
      ? [
          {
            k: "来源",
            v: item.source ?? "未知",
            mono: item.source !== null,
            tone: item.source ? undefined : ("unknown" as const),
          },
        ]
      : []),
    ...(item.version ? [{ k: "版本", v: item.version, mono: true }] : []),
    ...(item.command ? [{ k: "命令", v: item.command, mono: true, tone: "danger" as const }] : []),
    ...(item.kind === "skill"
      ? [
          {
            k: "含脚本",
            v: item.hasScripts ? "是" : "否",
            tone: item.hasScripts ? ("danger" as const) : undefined,
          },
        ]
      : []),
    ...(item.triggers !== null ? [{ k: "触发", v: `近 30 天 ${item.triggers} 次` }] : []),
    ...(item.members.length > 0 ? [{ k: "包含", v: item.members.join("、") }] : []),
  ];
  return (
    <Inspector
      kind={
        item.members.length > 1
          ? `${KIND_LABEL[item.kind]} · ${item.members.length} 项`
          : KIND_LABEL[item.kind]
      }
      name={item.name}
      status={{ tone: item.state ? toneOfState(item.state) : "neutral", label: item.label }}
      fields={fields}
      onClose={onClose}
    >
      <Section title="为什么是这个状态">
        <p className={page.why}>{item.reason}</p>
        <CommandHint command={`fangcun why ${item.members[0] ?? item.name}`} />
      </Section>
      {item.overriddenBy && (
        <OverriddenBox
          item={item}
          winner={context.items.find((i) => i.id === item.overriddenBy)}
          toolName={toolName}
          onPick={onPick}
        />
      )}
      <Section title="在哪一层声明 · 谁覆盖谁">
        <ol className={styles.stack}>
          {item.declared.map((d) => (
            <li
              key={d.scope}
              data-effective={d.effective}
              data-shadowed={!d.effective && d.location !== null}
            >
              <ScopeTicks scope={d.scope} />
              <span className={styles.stackLayer}>{SCOPE_NAMES[d.scope]}</span>
              <span className={styles.stackValue}>{stackText(d)}</span>
            </li>
          ))}
        </ol>
        <span className={styles.stackNote}>
          {shadowedBelow
            ? "这个工具同名时上层优先，与「下层覆盖上层」相反。"
            : "从上到下优先级递增，下层覆盖上层。"}
        </span>
      </Section>
      <Section title="上下文占用" aside={<span className={styles.shareText}>{shareText}</span>}>
        <div role="img" aria-label={shareText} className={styles.shareBar}>
          <span
            style={{
              width: `${Math.min(100, share)}%`,
              background: share >= 30 ? "var(--warn-dot)" : "var(--bar-1)",
            }}
          />
        </div>
      </Section>
      <Section title="写入了哪些文件">
        <p className={styles.sectionNote}>下面是工具实际读取的位置。</p>
        <ul className={styles.files}>
          {item.files.map((f) => (
            <li key={f} className={page.code}>
              {f}
            </li>
          ))}
        </ul>
      </Section>
      <Section title="最近活动">
        <p className={styles.sectionNote}>还没有活动。</p>
      </Section>
    </Inspector>
  );
}

/**
 * 被覆盖的项的处理块（6.3 检查器「被覆盖」）。一律能跳到覆盖它的项；
 * 全局层那一份赢、被挡住的是下层这一份时（Claude Code 的 skill），另给两个处理全局那一份的动作，M1 开放。
 */
function OverriddenBox({
  item,
  winner,
  toolName,
  onPick,
}: {
  item: EffectiveItem;
  winner: EffectiveItem | undefined;
  toolName: string;
  onPick: (id: string) => void;
}) {
  const winLayer = winner ? SCOPE_NAMES[winner.scope] : "其他";
  const thisOne = `${SCOPE_NAMES[item.scope]}里这份${item.version ? ` ${item.version} ` : ""}`;
  const globalWins = winner?.scope === "global" && item.scope !== "global";
  const kind = item.kind === "skill" ? "skill" : KIND_LABEL[item.kind];
  const text = globalWins
    ? `全局层有同名 ${kind}。${toolName} 同名时全局优先，${thisOne}不加载，常驻成本为 0。同步不会自动移走全局那一份：其他项目也在读它。`
    : `${winLayer}层有同名 ${kind}。${toolName} 同名时以${winLayer}层的为准，${thisOne}不加载，常驻成本为 0。`;
  return (
    <div className={styles.stateBox}>
      <p className={styles.stateText}>{text}</p>
      <div className={styles.stateActions}>
        {winner && (
          <button type="button" className={page.button} onClick={() => onPick(winner.id)}>
            查看覆盖它的项
          </button>
        )}
        {globalWins &&
          ["转成套装…", "从全局移除…"].map((a) => (
            <button
              key={a}
              type="button"
              className={page.button}
              disabled
              title="还没开放，见 M1-19、M1-20"
            >
              {a}
            </button>
          ))}
      </div>
      {globalWins && (
        <span className={styles.sectionNote}>
          两个动作处理的是全局那一份，都先出变更计划，列出受影响的项目。还没开放。
        </span>
      )}
    </div>
  );
}

/** 「在哪一层声明」一行的文字。示例数据里已经写好了「生效 · …」，内核给的是位置。 */
function stackText(d: { location: string | null; effective: boolean }): string {
  if (!d.location) return "未声明";
  if (/生效|遮蔽|停用/.test(d.location)) return d.location;
  return `${d.effective ? "生效" : "被遮蔽"} · ${d.location}`;
}

function ManifestTab({ context }: { context: EffectiveContext }) {
  return (
    <div className={page.empty}>
      <p>清单是本项目对 Agent 环境的声明：面向哪些工具、装哪些资产、各锁到哪个版本。</p>
      {context.hasManifest ? (
        <p>
          这个项目有 fangcun.toml。清单的表格视图还没做，可以直接打开文件，或用 fangcun status
          查看。
        </p>
      ) : (
        <p>
          这个项目还没有 <span className={page.version}>fangcun.toml</span>
          。重新登记会从现状反推清单，命令行写法是{" "}
          <span className={page.version}>fangcun init --from-existing</span>。
        </p>
      )}
    </div>
  );
}

/** 未登记的目录：检查器看项目本身，有「登记…」（6.1、F3、M1-25）。 */
function UnregisteredInspector({
  root,
  context,
  onlyGlobal,
  onClose,
}: {
  root: string;
  context: EffectiveContext;
  onlyGlobal: boolean;
  onClose: () => void;
}) {
  const openRegister = useRegister((s) => s.open);
  const suggestion = useQuery({
    queryKey: ["suggest-mode", root],
    queryFn: () => kernel.suggestMode(root),
  });
  const total = context.budget.instructions + context.budget.skills + context.budget.mcp;
  const committers = suggestion.data?.committers;
  return (
    <Inspector
      kind="项目"
      name={context.projectName}
      status={{ tone: "muted", label: "未登记" }}
      fields={[
        { k: "路径", v: context.projectPath, mono: true },
        {
          k: "清单",
          v: context.hasManifest ? "fangcun.toml" : "没有 fangcun.toml",
          mono: context.hasManifest,
        },
        {
          k: "提交者",
          v:
            committers === undefined
              ? "…"
              : committers === 0
                ? "不是 git 仓库"
                : `${committers} 位（git 历史，不算 [bot]）`,
        },
        {
          k: "常驻",
          v: `${formatK(total)} tokens（估算）${onlyGlobal ? "· 全部来自全局层" : ""}`,
        },
      ]}
      onClose={onClose}
    >
      <Section title="为什么是这个状态">
        <p className={page.why}>
          近 30 天的会话记录里出现过这个目录，但它还没登记。{"\n"}
          方寸只读显示它实际加载了什么，不写入。登记后才能同步、停用、升级。
        </p>
      </Section>
      <Section title="登记">
        <p className={styles.sectionNote}>
          从目录现状反推清单，生成 fangcun.toml 与
          fangcun.lock。查不出来源的项不进清单，留在原处照常生效。先问一次物化方式，再出变更计划。
        </p>
        <div>
          <button type="button" className={page.buttonPrimary} onClick={() => openRegister(root)}>
            登记…
          </button>
        </div>
        <CommandHint command="fangcun init --from-existing" />
      </Section>
    </Inspector>
  );
}

/** 「刚刚」或「11-13 10:42」，和原型的写法一样。 */
function when(iso: string): string {
  const d = new Date(iso);
  if (Date.now() - d.getTime() < 60_000) return "刚刚";
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/** 历史页签：这个项目的快照，最新的是当前磁盘状态，别的能「回滚到此」（6.3、M1-14）。 */
function HistoryTab({ root }: { root: string }) {
  const history = useHistory(root, true);
  const preview = usePlan((s) => s.preview);
  if (history.isError) return <p className={page.error}>{history.error.message}</p>;
  if (!history.data) return <p className={page.empty}>正在读取历史…</p>;
  if (history.data.length === 0)
    return (
      <div className={page.empty}>
        <p>还没有快照。同步、收编、升级都会留快照，之后在这里一键回滚。</p>
      </div>
    );
  return (
    <ol className={styles.history}>
      {history.data.map((h) => (
        <li key={h.record} className={styles.historyRow}>
          <span className={styles.historyId}>#{h.snapshot}</span>
          <span className={styles.historyTitle}>
            <span className={styles.historyName}>{h.title}</span>
            <span className={styles.historyWhen}> · {when(h.at)}</span>
          </span>
          {h.current ? (
            <span className={styles.historyCurrent}>当前磁盘状态</span>
          ) : (
            <button
              type="button"
              className={styles.historyButton}
              onClick={() =>
                preview({
                  load: () => kernel.planRollback(root, h.snapshot),
                  done: () => `已回滚到快照 #${h.snapshot}，磁盘状态逐字节一致`,
                })
              }
            >
              回滚到此
            </button>
          )}
        </li>
      ))}
    </ol>
  );
}
