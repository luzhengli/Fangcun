import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Link, Navigate, useLocation, useParams, useSearchParams } from "react-router";
import { FEATURES } from "@/app/features";
import c from "@/components/controls.module.css";
import { DecisionCard, Done, Options, WriteList } from "@/components/DecisionCard";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import { CommandHint, Inspector, Section } from "@/components/Inspector";
import { SCOPE_NAMES, SCOPES, ScopeTicks } from "@/components/ScopeTicks";
import { StatusDot } from "@/components/StatusDot";
import { ToolPicker } from "@/components/ToolPicker";
import { formatK, formatLimit, toneOfState } from "@/components/tone";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { Change } from "@/ipc/bindings/Change";
import type { DriftChoice } from "@/ipc/bindings/DriftChoice";
import type { DriftInfo } from "@/ipc/bindings/DriftInfo";
import type { EffectiveContext } from "@/ipc/bindings/EffectiveContext";
import type { EffectiveItem } from "@/ipc/bindings/EffectiveItem";
import type { FileDiff } from "@/ipc/bindings/FileDiff";
import type { ManifestEdit } from "@/ipc/bindings/ManifestEdit";
import type { ObservedKind } from "@/ipc/bindings/ObservedKind";
import type { ProjectSummary } from "@/ipc/bindings/ProjectSummary";
import type { UpdateInfo } from "@/ipc/bindings/UpdateInfo";
import { kernel } from "@/ipc/commands";
import {
  useEffectiveContext,
  useHistory,
  useManifestView,
  useProjects,
  useSyncPlan,
  useTools,
} from "@/ipc/queries";
import { projectPath } from "@/shell/nav";
import { PageToolbar } from "@/shell/PageToolbar";
import { usePalette } from "@/state/palette";
import { applyInline, usePlan } from "@/state/plan";
import { useRegister } from "@/state/register";
import { useStaged, useStagedEdits } from "@/state/staged";
import { useUi } from "@/state/ui";
import page from "../page.module.css";
import { ContextMap } from "./ContextMap";
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
  // 检查器里「加入同步计划」的清单改动（6.8），点「同步 · N」时一起出计划。
  const staged = useStagedEdits(root);
  const clearStaged = useStaged((s) => s.clear);
  const syncPlan = useSyncPlan(root, registered, staged);
  const preview = usePlan((s) => s.preview);
  const unstage = useStaged((s) => s.unstage);
  // 同步计划按原因分组（6.8）：每条暂存的改动单独出一份计划，减掉没暂存时的差异，就是它那一组。
  const openSync = (edits: ManifestEdit[] = staged) => {
    if (!root) return;
    let of: (change: Change) => string = () => "清单里还没同步的改动";
    preview({
      load: async () => {
        const key = (ch: Change) => `${ch.op}:${ch.path}`;
        const [full, base, ...each] = await Promise.all([
          kernel.planSync(root, edits),
          kernel.planSync(root, []),
          ...edits.map((e) => kernel.planSync(root, [e])),
        ]);
        const baseKeys = new Set(base.changes.map(key));
        const cause = new Map<string, string>();
        edits.forEach((e, i) => {
          for (const ch of each[i]?.changes ?? [])
            if (!baseKeys.has(key(ch)) && !cause.has(key(ch)))
              cause.set(key(ch), causeOf(e, items));
        });
        of = (ch) => cause.get(key(ch)) ?? "清单里还没同步的改动";
        return full;
      },
      title: (plan) =>
        plan.changes.length > 0
          ? `同步 ${data?.projectName ?? ""} · ${plan.changes.length} 处变更`
          : `同步 ${data?.projectName ?? ""}`,
      sub: (plan) =>
        plan.changes.length > 0 ? "应用前留快照 · 可在「历史」回滚" : "没有排进来的变更",
      group: {
        of: (ch) => of(ch),
        remove: (c) => {
          const left = edits.filter((e) => causeOf(e, items) !== c);
          for (const e of edits) if (causeOf(e, items) === c) unstage(root, e);
          openSync(left);
        },
      },
      done: (applied, plan) =>
        `已同步 ${plan.changes.length} 处变更 · 快照 ${applied.snapshots.map((s) => `#${s}`).join("、")}`,
      after: () => clearStaged(root),
    });
  };
  const [tab, setTab] = useState<Tab>("effective");
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [hover, setHover] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | "pending">("all");
  // 回到项目卡片（6.3）：点列表空白处、Esc、再点一次侧栏里同一个项目。
  const toCard = () => {
    setSelected(null);
    setInspectorOpen(true);
  };
  // 再点一次侧栏里同一个项目：地址不变，但每次导航都有新的 location.key。
  // 从活动页的待处理过来时带着 ?asset=，选中那一项（6.7「去处理漂移」「查看差异」）。
  const navKey = useLocation().key;
  const [params] = useSearchParams();
  const [seenNavKey, setSeenNavKey] = useState<string | null>(null);
  const [wanted, setWanted] = useState<string | null>(params.get("asset"));
  // 命令面板的动作带着 ?tab=、?do=sync 过来（6.1 ⌘K）。
  const [doSync, setDoSync] = useState(params.get("do") === "sync");
  if (navKey !== seenNavKey) {
    setSeenNavKey(navKey);
    toCard();
    setWanted(params.get("asset"));
    const wantTab = params.get("tab");
    if (wantTab === "manifest" || wantTab === "history") setTab(wantTab);
    setDoSync(params.get("do") === "sync");
  }
  const picked = selected !== null;
  useEffect(() => {
    if (!picked) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // 弹窗开着时 Esc 是关弹窗的。
      if (document.querySelector('[role="dialog"]')) return;
      if ((e.target as HTMLElement | null)?.closest("input, textarea, select")) return;
      setSelected(null);
      setInspectorOpen(true);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [picked]);

  const data = context.data;
  const items = data?.items ?? [];
  const syncReady = doSync && registered && data !== undefined;
  // biome-ignore lint/correctness/useExhaustiveDependencies: 只在命令面板要求同步、数据到齐时开一次。
  useEffect(() => {
    if (!syncReady) return;
    setDoSync(false);
    openSync();
  }, [syncReady]);
  if (wanted && data) {
    const hit = items.find((i) => i.name === wanted);
    setWanted(null);
    if (hit) {
      setTab("effective");
      setSelected(hit.id);
    }
  }
  // 没选中任何一项时看项目卡片，已登记、未登记一样（6.3）。
  const current = items.find((i) => i.id === selected);
  const tools = useTools();
  const toolName = tools.data?.find((t) => t.id === tool)?.name ?? tool;
  const pending = syncPlan.data?.changes.length ?? 0;
  const pendingItems = items.filter((i) => i.state && PENDING[i.state]);
  // 排进计划还没同步的停用、恢复：常驻不提前扣，只算「同步后约 N」（6.3）。
  const residentOf = (e: ManifestEdit) =>
    e.op === "upgrade"
      ? 0
      : items
          .filter((i) => i.disable && i.disable.names.join("\n") === e.names.join("\n"))
          .reduce((n, i) => n + (i.resident ?? 0), 0);
  const delta = staged.reduce(
    (n, e) => n + (e.op === "disable" ? -residentOf(e) : e.op === "enable" ? residentOf(e) : 0),
    0,
  );
  const afterSync =
    delta !== 0
      ? items.filter((i) => i.counted).reduce((n, i) => n + (i.resident ?? 0), 0) + delta
      : null;
  const openRegister = useRegister((s) => s.open);
  const onlyGlobal = items.length > 0 && items.every((i) => i.scope === "global");

  return (
    <div className={page.page}>
      <PageToolbar
        title={data?.projectName ?? "…"}
        subtitle={data?.projectPath}
        mono
        tag={summary && !registered ? "未登记" : undefined}
        crumb={
          current
            ? { leaf: current.name, back: toCard, backTitle: "回到项目卡片（Esc）" }
            : undefined
        }
      >
        <ToolPicker value={tool} onChange={setTool} />
        <button
          type="button"
          className={styles.palette}
          aria-label="命令面板"
          title="搜索资产、项目、动作 ⌘K"
          onClick={() => usePalette.getState().show()}
        >
          <Icon name="search" size={14} strokeWidth={1.6} />
          <span className={styles.paletteText}>搜索资产与动作</span>
          <span className={c.kbd}>⌘K</span>
        </button>
        {registered && (
          <button
            type="button"
            className={pending > 0 ? `${c.b} ${c.primary}` : c.b}
            onClick={() => openSync()}
            title="按清单与锁物化，先出变更计划"
          >
            <Icon name="sync" size={14} strokeWidth={1.6} />
            同步
            {pending > 0 && <span className={styles.count}>· {pending}</span>}
          </button>
        )}
        {summary && !registered && root && (
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            onClick={() => openRegister(root)}
          >
            登记…
          </button>
        )}
        <button
          type="button"
          className={c.icon}
          aria-pressed={inspectorOpen}
          aria-label="检查器"
          title="显示或隐藏检查器 ⌥⌘0"
          onClick={() => setInspectorOpen(!inspectorOpen)}
        >
          <Icon name="inspector" strokeWidth={1.4} />
        </button>
      </PageToolbar>

      {data && (
        <ContextMap
          items={items}
          limit={data.budget.limit}
          registered={registered}
          selectedId={current?.id}
          hoverId={hover}
          onHover={setHover}
          onPick={(id) => {
            setTab("effective");
            setSelected(id);
            setInspectorOpen(true);
          }}
          after={afterSync}
        />
      )}

      <div className={styles.viewBar}>
        {registered && (
          <div aria-label="项目视图" className={c.seg} role="tablist">
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
        <span className={styles.flex} />
        {(tab === "effective" || !registered) && data && (
          <fieldset aria-label="筛选" className={c.seg}>
            <button type="button" aria-pressed={filter === "all"} onClick={() => setFilter("all")}>
              全部 <span className={c.count}>{items.length}</span>
            </button>
            <button
              type="button"
              aria-pressed={filter === "pending"}
              onClick={() => setFilter("pending")}
            >
              待处理 <span className={c.count}>{pendingItems.length}</span>
            </button>
          </fieldset>
        )}
      </div>

      <div className={page.split}>
        {/* 点空白处回到项目卡片只是快捷方式，键盘用 Esc。 */}
        {/* biome-ignore lint/a11y/noStaticElementInteractions: 同上 */}
        {/* biome-ignore lint/a11y/useKeyWithClickEvents: 同上 */}
        <section
          className={page.list}
          onClick={(e) => {
            if (!picked) return;
            if ((e.target as HTMLElement).closest("button, a, input, select, textarea")) return;
            toCard();
          }}
        >
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
              staged={staged}
              filter={filter}
              hoverId={hover}
              onHover={setHover}
              selectedId={current?.id}
              onPick={(id) => {
                setSelected(id);
                setInspectorOpen(true);
              }}
            />
          )}
          {data && registered && tab === "manifest" && root && (
            <ManifestTab root={root} context={data} />
          )}
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
        {/* 清单页签旁边照样是项目卡片（原型 Main）；点「待处理」的一行回到生效上下文选中它。 */}
        {registered &&
          (tab === "manifest" || (tab === "effective" && !current)) &&
          inspectorOpen &&
          data &&
          root && (
            <RegisteredInspector
              root={root}
              context={data}
              summary={summary}
              staged={staged}
              onSync={() => openSync()}
              onPick={(id) => {
                setSelected(id);
                setTab("effective");
              }}
              onClose={() => setInspectorOpen(false)}
            />
          )}
        {(tab === "effective" || !registered) && inspectorOpen && current && data && (
          <ItemInspector
            key={current.id}
            item={current}
            context={data}
            root={registered ? root : undefined}
            staged={staged}
            onPick={setSelected}
            onClose={() => setInspectorOpen(false)}
          />
        )}
      </div>
    </div>
  );
}

/** 一条暂存改动在同步计划里的原因（6.8），例如「在本项目停用 lark-doc 等 16 项」。 */
function causeOf(e: ManifestEdit, items: EffectiveItem[]): string {
  if (e.op === "upgrade") {
    const to = items.find((i) => i.name === e.asset)?.update?.to;
    const others = e.projects.length > 0 ? `，另 ${e.projects.length} 个项目` : "";
    return `升级 ${e.asset}${to ? ` 到 ${to}` : ""}${others}`;
  }
  const what = e.names.length > 1 ? `${e.names[0]} 等 ${e.names.length} 项` : (e.names[0] ?? "");
  return `${e.op === "disable" ? "在本项目停用" : "在本项目恢复"} ${what}`;
}

function EffectiveList({
  context,
  staged,
  filter,
  hoverId,
  onHover,
  selectedId,
  onPick,
}: {
  context: EffectiveContext;
  staged: ManifestEdit[];
  filter: "all" | "pending";
  hoverId: string | null;
  onHover: (id: string | null) => void;
  selectedId: string | undefined;
  onPick: (id: string) => void;
}) {
  const missing = context.observedMissing.map((k) => OBSERVED_LABEL[k]);
  const shown = (i: EffectiveItem) => filter === "all" || (i.state !== null && PENDING[i.state]);
  if (filter === "pending" && !context.items.some(shown))
    return (
      <div className={page.empty}>
        <p className={styles.emptyTitle}>没有待处理的项</p>
        <p>漂移、可更新、脚本变更都处理完了。</p>
      </div>
    );
  return (
    <div>
      <div className={page.colHead}>
        <span className={styles.colName}>名称 · 版本</span>
        <span className={styles.colLayer}>作用层</span>
        <span className={styles.colStatus}>状态</span>
        <span className={`${styles.colCost} ${page.cost}`}>常驻</span>
      </div>
      {GROUPS.map(({ kind, label }) => {
        const rows = context.items.filter((i) => i.kind === kind && shown(i));
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
            {rows.map((raw) => {
              const pending = stagedOp(staged, raw);
              const upgrade = staged.some((e) => e.op === "upgrade" && e.asset === raw.name);
              // 排进计划还没同步的：写「停用（待同步）」「待同步 ↑ 新版本」，常驻不提前扣（6.3）。
              const item: EffectiveItem =
                pending === "disable"
                  ? { ...raw, label: "停用（待同步）" }
                  : pending === "enable"
                    ? { ...raw, label: "恢复（待同步）" }
                    : upgrade && raw.update
                      ? { ...raw, label: `待同步 ↑ ${raw.update.to}` }
                      : raw;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={page.row}
                  aria-pressed={item.id === selectedId}
                  data-hl={hoverId === item.id}
                  onMouseEnter={() => onHover(item.id)}
                  onMouseLeave={() => onHover(null)}
                  onClick={() => onPick(item.id)}
                >
                  <span className={styles.colName}>
                    <span
                      className={`${page.name} ${item.state === "overridden" || pending === "disable" ? page.struck : ""}`}
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
                      strong={item.state !== null && STRONG[item.state] === true}
                    />
                  </span>
                  <span className={`${styles.colCost} ${page.cost}`}>
                    {!item.counted ? "–" : item.resident === null ? "—" : formatK(item.resident)}
                  </span>
                </button>
              );
            })}
          </div>
        );
      })}
      <div>
        <h2 className={page.groupHead} style={{ justifyContent: "flex-start" }}>
          其他加载项<span className={c.tag}>只看不管</span>
          <span className={c.tag}>不计入预算</span>
          <InfoTip label="其他加载项说明">
            插件、Hook、斜杠命令、子代理也会被 Agent
            加载。列在这里是为了看清全貌；当前版本方寸不修改它们，也不计入预算。
          </InfoTip>
        </h2>
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
  root,
  staged,
  onPick,
  onClose,
}: {
  item: EffectiveItem;
  context: EffectiveContext;
  /** 已登记的项目才给：没有清单的不能在本项目停用。 */
  root: string | undefined;
  staged: ManifestEdit[];
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  const tools = useTools();
  const toolName = tools.data?.find((t) => t.id === context.tool)?.name ?? context.tool;
  const [driftDone, setDriftDone] = useState<DriftDone | null>(null);
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
    ...(item.secrets.length > 0 ? [{ k: "密钥", v: item.secrets.join("；") }] : []),
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
      kind={`${KIND_LABEL[item.kind]}${item.members.length > 1 ? ` · ${item.members.length} 项` : ""} · ${SCOPE_NAMES[item.scope]}${item.bundle ? ` · ${item.bundle}` : ""}`}
      name={item.name}
      status={{ tone: item.state ? toneOfState(item.state) : "neutral", label: item.label }}
      intro={
        <>
          {item.reason}
          <br />
          <CommandHint command={`fangcun why ${item.members[0] ?? item.name}`} />
        </>
      }
      decision={
        <>
          {driftDone ? (
            <Done
              title={DRIFT_DONE[driftDone.choice].title}
              desc={DRIFT_DONE[driftDone.choice].desc}
              extra={
                driftDone.others.length > 0 && (
                  <span className={styles.cardText}>
                    同样用它的项目：{driftDone.others.join("、")}，已进「可更新」。
                  </span>
                )
              }
              onUndo={() => undoRecord(driftDone.record)}
            />
          ) : (
            root &&
            item.drift && (
              <DriftBox item={item} drift={item.drift} root={root} onDone={setDriftDone} />
            )
          )}
          {root && item.update && (
            <UpdateBox item={item} update={item.update} root={root} staged={staged} />
          )}
          {root && item.disable && (
            <DisableBox item={item} root={root} staged={staged} toolName={toolName} />
          )}
          {item.overriddenBy && (
            <OverriddenBox
              item={item}
              winner={context.items.find((i) => i.id === item.overriddenBy)}
              toolName={toolName}
              onPick={onPick}
            />
          )}
          {item.state === "unknown-source" && <UnknownBox item={item} />}
        </>
      }
      fields={fields}
      onClose={onClose}
    >
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
        {item.files.length === 0 ? (
          <p className={styles.sectionNote}>方寸没有写过文件。</p>
        ) : (
          <ul className={styles.files}>
            {item.files.map((f) => (
              <li key={f} className={page.code}>
                {f}
              </li>
            ))}
          </ul>
        )}
      </Section>
      <Section title="最近活动">
        {item.recent.length === 0 ? (
          <p className={styles.sectionNote}>还没有活动。</p>
        ) : (
          <ol className={styles.acts}>
            {item.recent.map((a) => (
              <li key={a.record}>
                <Link to={`/activity?record=${encodeURIComponent(a.record)}`} title="在活动里查看">
                  <span aria-hidden="true" className={styles.actDot} />
                  <span className={styles.actWhen}>{ago(a.at.toString())}</span>
                  <span className={styles.actText}>{a.text}</span>
                </Link>
              </li>
            ))}
          </ol>
        )}
      </Section>
    </Inspector>
  );
}

type SwitchOp = "disable" | "enable";

/** 这一项在暂存里是要停用还是要恢复。 */
function stagedOp(staged: ManifestEdit[], item: EffectiveItem): SwitchOp | null {
  const names = item.disable?.names;
  if (!names) return null;
  for (const e of staged) {
    if (e.op === "upgrade") continue;
    if (e.kind === item.kind && e.names.join("\n") === names.join("\n")) return e.op;
  }
  return null;
}

/** 差异片段：一个文件的行，增删着色（原型 Main 漂移块）。 */
function DiffLines({ file }: { file: FileDiff }) {
  if (file.binary)
    return <p className={styles.sectionNote}>{file.path} 是二进制文件，不列差异。</p>;
  return (
    <div className={styles.diff}>
      {file.lines.map((l, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: 差异行的位置就是键
        <div key={i} data-kind={l.kind}>
          {l.kind === "add" ? "+ " : l.kind === "remove" ? "- " : l.kind === "context" ? "  " : ""}
          {l.text}
        </div>
      ))}
      {file.truncated > 0 && <div data-kind="hunk">…另有 {file.truncated} 行</div>}
    </div>
  );
}

type DriftDone = { choice: DriftChoice; record: string; others: string[] };

const DRIFT_DONE: Record<DriftChoice, { title: string; desc: string }> = {
  promote: { title: "已回流到来源", desc: "来源生成了新版本，本项目改用它。" },
  discard: {
    title: "已丢弃修改",
    desc: "副本已恢复为锁定的版本。被丢弃的修改在快照里，回滚能找回。",
  },
  fork: {
    title: "已分叉为项目专属",
    desc: "改成本项目自己的文件，提交进仓库。清单和锁里去掉了这一项，方寸不再管它。",
  },
};

function diffCount(files: FileDiff[]): string {
  let add = 0;
  let del = 0;
  for (const f of files)
    for (const l of f.lines) {
      if (l.kind === "add") add++;
      if (l.kind === "remove") del++;
    }
  return `+${add} −${del}`;
}

function undoRecord(record: string) {
  void usePlan.getState().preview({
    load: () => kernel.planRollbackRecord(record),
    done: (_, plan) => `已撤销：${plan.title}`,
    undoable: false,
  });
}

/**
 * 漂移的决定卡片（6.3、F4、9.3）：差异片段，回流、丢弃、分叉三选一，「会写入 · N 处」随选项变，
 * 点按钮就写。回流只对本地来源，GitHub 来源置灰并说明。
 */
function DriftBox({
  item,
  drift,
  root,
  onDone,
}: {
  item: EffectiveItem;
  drift: DriftInfo;
  root: string;
  onDone: (done: DriftDone) => void;
}) {
  const client = useQueryClient();
  const [pick, setPick] = useState<DriftChoice>(drift.promoteBlocked ? "discard" : "promote");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const others = drift.others.length;
  const options: {
    id: DriftChoice;
    title: string;
    desc: string;
    off: string | null;
    recommended?: boolean;
  }[] = [
    {
      id: "promote",
      title: "回流到来源",
      desc: `写回 ${item.source ?? "来源"} 生成新版本${others > 0 ? `，其他 ${others} 个项目进「可更新」` : ""}。`,
      off: drift.promoteBlocked,
      recommended: true,
    },
    {
      id: "discard",
      title: "丢弃修改",
      desc: "被丢弃的修改另存一份，可找回。",
      off: drift.original ? null : "原版取不回（本地来源没有提交）",
    },
    {
      id: "fork",
      title: "分叉为项目专属",
      desc: "改成本项目自己的文件，提交进仓库，不再跟随来源。",
      off: item.scope === "bundle" ? "来自套装的项还不能分叉" : null,
    },
  ];
  const chosen = options.find((o) => o.id === pick && !o.off) ?? options.find((o) => !o.off);
  const plan = useQuery({
    queryKey: ["drift-plan", root, item.name, chosen?.id],
    queryFn: () => kernel.planDrift(root, item.name, chosen?.id ?? "discard"),
    enabled: chosen !== undefined,
  });
  const button: Record<DriftChoice, string> = {
    promote: "回流到来源",
    discard: "丢弃修改",
    fork: "分叉到项目",
  };
  const apply = async () => {
    if (!chosen || !plan.data || plan.data.blocked) return;
    setBusy(true);
    setError(null);
    try {
      const applied = await applyInline(
        plan.data,
        (a) =>
          `${DRIFT_DONE[chosen.id].title} · 快照 ${a.snapshots.map((x) => `#${x}`).join("、")}`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
      onDone({
        choice: chosen.id,
        record: applied.record,
        others: chosen.id === "promote" ? drift.others.map(projectName) : [],
      });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };
  return (
    <DecisionCard
      title="副本被改过"
      metric={diffCount(drift.files)}
      tone="warn"
      foot={
        <>
          <span className={styles.footNote}>写前留快照 · 可撤销</span>
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            disabled={!chosen || !plan.data || plan.data.blocked || busy}
            onClick={() => void apply()}
          >
            {busy ? "正在写入…" : chosen ? button[chosen.id] : "处理"}
          </button>
        </>
      }
    >
      <p className={styles.cardText}>{drift.summary}</p>
      {drift.files.slice(0, 2).map((f) => (
        <div key={f.path}>
          <span className={styles.diffPath}>{f.path}</span>
          <DiffLines file={f} />
        </div>
      ))}
      {drift.files.length > 2 && (
        <span className={styles.sectionNote}>另有 {drift.files.length - 2} 个文件有变化。</span>
      )}
      <Options
        name={`drift-${item.id}`}
        legend="处理方式"
        options={options}
        value={chosen?.id}
        onChange={setPick}
      />
      <WriteList
        changes={plan.data?.changes}
        loading={plan.isLoading}
        error={error ?? (plan.isError ? plan.error.message : null)}
        blockedNote={plan.data?.blocked ? plan.data.notes.map((n) => n.text).join(" ") : null}
      />
    </DecisionCard>
  );
}

/**
 * 可更新的决定卡片（6.3、F6）：版本差异，脚本变更标红；同一版本的其他项目可勾选，
 * 一起加入同步计划，每个项目各留一个快照。
 */
function UpdateBox({
  item,
  update,
  root,
  staged,
}: {
  item: EffectiveItem;
  update: UpdateInfo;
  root: string;
  staged: ManifestEdit[];
}) {
  const stage = useStaged((s) => s.stage);
  const unstage = useStaged((s) => s.unstage);
  const queued = staged.find(
    (e): e is Extract<ManifestEdit, { op: "upgrade" }> =>
      e.op === "upgrade" && e.asset === item.name,
  );
  const [picked, setPicked] = useState<string[]>([]);
  const chosen = queued?.projects ?? picked;
  const toggle = (id: string) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
  const edit: ManifestEdit = { op: "upgrade", asset: item.name, projects: chosen };
  const total = chosen.length + 1;
  const add = update.files.reduce((n, f) => n + f.added, 0);
  const del = update.files.reduce((n, f) => n + f.removed, 0);
  return (
    <DecisionCard
      title={`有新版本 ${update.from} → ${update.to}`}
      metric={`+${add} −${del}`}
      tone="info"
      foot={
        queued ? (
          <>
            <span className={styles.footNote}>
              已加入计划 · {total > 1 ? `共 ${total} 个项目` : "仅本项目"}
            </span>
            <button
              type="button"
              className={`${c.b} ${c.small}`}
              onClick={() => unstage(root, queued)}
            >
              撤回
            </button>
          </>
        ) : (
          <>
            <span className={styles.footNote}>各留快照</span>
            <button
              type="button"
              className={`${c.b} ${c.primary}`}
              onClick={() => stage(root, edit)}
            >
              {total > 1 ? `加入计划 · ${total} 个项目` : "加入计划"}
            </button>
          </>
        )
      }
    >
      <ul className={styles.updateFiles}>
        {update.files.map((f) => (
          <li key={f.path} className={styles.updateFile} data-script={f.script}>
            <span className={styles.updateOp}>
              {f.op === "add" ? "+" : f.op === "remove" ? "−" : "~"}
            </span>
            <span className={styles.updateName}>
              {f.script ? (
                <>
                  <strong>脚本变更</strong> · {f.path}
                  {f.op === "add"
                    ? " 新增，会执行命令"
                    : f.op === "remove"
                      ? " 删除"
                      : " 改动，会执行命令"}
                </>
              ) : (
                f.path
              )}
            </span>
            <span className={styles.updateOp}>
              {f.added > 0 ? `+${f.added}` : ""}
              {f.removed > 0 ? ` −${f.removed}` : ""}
            </span>
          </li>
        ))}
      </ul>
      <span className={styles.cliHint}>完整差异：fangcun update {item.name} --dry-run</span>
      {update.others.length > 0 && (
        <fieldset className={styles.affected}>
          <legend>同版本项目一起升级</legend>
          <label>
            <input type="checkbox" checked disabled />
            <span className={styles.affectedName}>{projectName(root)}</span>
            <span>本项目</span>
          </label>
          {update.others.map((p) => (
            <label key={p.project}>
              <input
                type="checkbox"
                checked={chosen.includes(p.project)}
                disabled={queued !== undefined}
                onChange={() => toggle(p.project)}
              />
              <span className={styles.affectedName}>{p.name}</span>
              <span>{p.note}</span>
            </label>
          ))}
        </fieldset>
      )}
    </DecisionCard>
  );
}

function projectName(root: string): string {
  return root.split("/").filter(Boolean).pop() ?? root;
}

/**
 * 全局的 skill 与 MCP「在本项目停用」（6.3 检查器「全局税」、7.4）。点了先加入同步计划（6.8），
 * 由「同步 · N」统一预览和应用。清单里已经停用的给「恢复」。
 */
function DisableBox({
  item,
  root,
  staged,
  toolName,
}: {
  item: EffectiveItem;
  root: string;
  staged: ManifestEdit[];
  toolName: string;
}) {
  const stage = useStaged((s) => s.stage);
  const unstage = useStaged((s) => s.unstage);
  const option = item.disable;
  if (!option) return null;
  const pending = stagedOp(staged, item);
  if (!option.idle && !option.active && !pending) return null;
  if (item.kind === "instruction") return null;
  const kind = item.kind;
  const edit = (op: SwitchOp): ManifestEdit => ({ op, kind, names: option.names });
  const count = item.members.length || option.names.length;
  const what =
    item.kind === "mcp"
      ? "这个 MCP 服务每次会话都会启动，本项目从未调用。"
      : count > 1
        ? `${count} 个 skill 常驻上下文，本项目从未触发。`
        : "它的名称与描述常驻上下文，本项目从未触发。";
  const saving = item.resident && item.resident > 0 ? `−${formatK(item.resident)}` : null;

  if (pending) {
    return (
      <DecisionCard
        title={pending === "disable" ? "已加入计划" : "已加入计划 · 恢复加载"}
        metric={pending === "disable" ? saving : null}
        foot={
          <>
            <span className={styles.footNote}>点「同步」时一起预览和应用</span>
            <button
              type="button"
              className={`${c.b} ${c.small}`}
              onClick={() => unstage(root, edit(pending))}
            >
              撤回
            </button>
          </>
        }
      />
    );
  }
  if (option.active) {
    return (
      <DecisionCard
        title="已在本项目停用"
        foot={
          <>
            <span className={styles.footNote}>其他项目照常加载</span>
            <button type="button" className={c.b} onClick={() => stage(root, edit("enable"))}>
              在本项目恢复
            </button>
          </>
        }
      >
        <p className={styles.cardText}>
          写在清单 [disable]，同步时写进 {toolName} 自己的开关。
          {option.note && ` ${option.note}`}
        </p>
      </DecisionCard>
    );
  }
  return (
    <DecisionCard
      title="全局税：每个项目都在缴"
      metric={saving && `${saving} 停用后`}
      tone="warn"
      foot={
        <>
          <span className={styles.footNote}>只写本项目的清单</span>
          <button type="button" className={c.b} disabled title="还没开放，见 M1-19">
            改为套装…
          </button>
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            disabled={option.unsupported !== null}
            title={option.unsupported ?? "加入同步计划，点「同步」时一起预览和应用"}
            onClick={() => stage(root, edit("disable"))}
          >
            在本项目停用
          </button>
        </>
      }
    >
      <p className={styles.cardText}>{what}</p>
      {option.unsupported && <span className={styles.sectionNote}>{option.unsupported}</span>}
      {option.note && <span className={styles.sectionNote}>{option.note}</span>}
    </DecisionCard>
  );
}

/**
 * 被覆盖的决定卡片（6.3）。一律能跳到覆盖它的项；
 * 全局层那一份赢、被挡住的是下层这一份时（Claude Code 的 skill），另给两个处理全局那一份的动作。
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
  const globalWins = winner?.scope === "global" && item.scope !== "global";
  const version = item.version ? `${item.version} ` : "";
  const tool = useUi((s) => s.tool);
  // 从全局移除（M1-34、M1-02）：改的是全局那一份，多处、跨项目，进变更计划面板（6.1 决定卡片第 5 条）。
  const removeGlobal = (w: EffectiveItem) =>
    void usePlan.getState().preview({
      load: () => kernel.planRemoveGlobal(tool, [{ kind: w.kind, name: w.name }]),
      sub: () => "移出全局层，资产库保留正本 · 应用前留快照，可撤销",
      done: () => `已从全局移除 ${w.name} · 已留快照`,
    });
  return (
    <DecisionCard
      title={globalWins ? "被全局同名项挡住" : `被${winLayer}层同名项覆盖`}
      metric={`${version}不加载`}
      foot={
        <>
          <span className={styles.footNote}>
            {toolName} 同名时{globalWins ? "全局" : `${winLayer}层`}优先
            <InfoTip label="被覆盖说明" align="right">
              {globalWins
                ? "同步不会自动移走全局那一份：其他项目也在读它。两个动作处理的是全局那一份，都先出变更计划，列出受影响的项目。"
                : `${winLayer}层的那一份生效，这一份常驻成本为 0。`}
            </InfoTip>
          </span>
          {winner && (
            <button type="button" className={c.b} onClick={() => onPick(winner.id)}>
              查看覆盖项
            </button>
          )}
        </>
      }
    >
      {globalWins && winner && (
        <div className={styles.cardActions}>
          {FEATURES.bundles && (
            <button type="button" className={c.b} disabled title="还没开放，见 M1-19">
              转成套装…
            </button>
          )}
          <button
            type="button"
            className={`${c.b} ${c.danger}`}
            title="移出全局层，先出变更计划，列出受影响的项目。资产库保留正本"
            onClick={() => removeGlobal(winner)}
          >
            从全局移除…
          </button>
        </div>
      )}
    </DecisionCard>
  );
}

/** 来源未知的决定卡片（6.3）：收编后才能追踪版本与更新，去资产库收编。 */
function UnknownBox({ item }: { item: EffectiveItem }) {
  return (
    <DecisionCard
      title="查不到来源"
      metric="哈希不匹配任何来源"
      foot={
        <>
          <span className={styles.footNote}>
            收编后才能追踪版本与更新
            <InfoTip label="来源未知说明" align="right">
              在 {item.location} 发现，内容哈希与资产库里任何来源都不匹配。
            </InfoTip>
          </span>
          <Link to={`/library?asset=${encodeURIComponent(item.name)}`} className={c.b}>
            去收编
          </Link>
        </>
      }
    />
  );
}

/** 「在哪一层声明」一行的文字。示例数据里已经写好了「生效 · …」，内核给的是位置。 */
function stackText(d: { location: string | null; effective: boolean }): string {
  if (!d.location) return "未声明";
  if (/生效|遮蔽|停用/.test(d.location)) return d.location;
  return `${d.effective ? "生效" : "被遮蔽"} · ${d.location}`;
}

/** 清单页签（6.3、M1-31）：三张文件卡可切换，结构化视图按区块列表格，源文件带行号。只读。 */
function ManifestTab({ root, context }: { root: string; context: EffectiveContext }) {
  const view = useManifestView(root, context.hasManifest);
  const [picked, setPicked] = useState(0);
  const [mode, setMode] = useState<"table" | "src">("table");
  if (!context.hasManifest) {
    return (
      <div className={page.empty}>
        <p>清单是本项目对 Agent 环境的声明：面向哪些工具、装哪些资产、各锁到哪个版本。</p>
        <p>
          这个项目还没有 <span className={page.version}>fangcun.toml</span>
          。重新登记会从现状反推清单，命令行写法是{" "}
          <span className={page.version}>fangcun init --from-existing</span>。
        </p>
      </div>
    );
  }
  const files = view.data?.files ?? [];
  const file = files[picked] ?? files[0];
  return (
    <div className={styles.manifest}>
      <p className={styles.manifestIntro}>
        清单是本项目对 Agent
        环境的声明：面向哪些工具、装哪些资产、各锁到哪个版本。下面三个文件共同决定「生效上下文」里看到的结果。
      </p>
      <fieldset aria-label="清单文件" className={styles.fileCards}>
        {files.map((f, i) => (
          <button
            key={f.name}
            type="button"
            aria-pressed={f === file}
            className={styles.fileCard}
            onClick={() => setPicked(i)}
          >
            <span className={styles.fileName}>{f.name}</span>
            <span className={styles.fileRole}>{f.role}</span>
            <span
              className={styles.fileTag}
              data-tone={f.name.endsWith(".lock") ? "info" : f.committed ? "ok" : "muted"}
            >
              {f.tag}
            </span>
          </button>
        ))}
      </fieldset>
      {file && (
        <>
          <div className={styles.manifestBar}>
            <span className={styles.manifestDesc}>{file.description}</span>
            <fieldset aria-label="显示方式" className={styles.viewSwitch}>
              {(
                [
                  ["table", "结构化"],
                  ["src", "源文件"],
                ] as const
              ).map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  aria-pressed={mode === id}
                  onClick={() => setMode(id)}
                >
                  {label}
                </button>
              ))}
            </fieldset>
          </div>
          {!file.exists ? (
            <p className={styles.sectionNote}>还没有 {file.name}。</p>
          ) : mode === "table" ? (
            <div className={styles.manifestSections}>
              {file.sections.length === 0 && <p className={styles.sectionNote}>文件里没有内容。</p>}
              {file.sections.map((sec) => (
                <section key={sec.title} className={styles.manifestSection}>
                  <h3>{sec.title}</h3>
                  <div className={styles.tableWrap}>
                    <table className={styles.table}>
                      <thead>
                        <tr>
                          {sec.headers.map((h) => (
                            <th key={h} scope="col">
                              {h}
                            </th>
                          ))}
                        </tr>
                      </thead>
                      <tbody>
                        {sec.rows.map((r) => (
                          <tr key={r.cells.join("\u0000")}>
                            {r.cells.map((c, i) => (
                              <td
                                // biome-ignore lint/suspicious/noArrayIndexKey: 三格位置固定
                                key={i}
                                data-mono={r.mono[i]}
                                data-col={i}
                                data-warn={i === 2 && r.warn}
                              >
                                {c}
                                {i === 2 && r.note && (
                                  <span className={styles.cellNote}>{r.note}</span>
                                )}
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </section>
              ))}
            </div>
          ) : (
            <div className={styles.source}>
              {file.lines.map((line, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: 行号就是键
                <div key={i} className={styles.sourceLine}>
                  <span aria-hidden="true">{i + 1}</span>
                  <span
                    data-kind={
                      line.startsWith("#") ? "comment" : line.startsWith("[") ? "table" : undefined
                    }
                  >
                    {line}
                  </span>
                </div>
              ))}
            </div>
          )}
        </>
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
        { k: "提交者", v: committersText(committers) },
        {
          k: "常驻",
          v: `${formatK(total)} tokens（估算）${onlyGlobal ? "· 全部来自全局层" : ""}`,
        },
      ]}
      intro="近 30 天的会话记录里出现过这个目录，但它还没登记。方寸只读显示它实际加载了什么，不写入。"
      decision={
        <DecisionCard
          title="登记后才能管理"
          foot={
            <>
              <span className={styles.footNote}>
                从现状生成 fangcun.toml 与 .lock
                <InfoTip label="登记说明" align="right">
                  从目录现状反推清单。查不出来源的项不进清单，留在原处照常生效。先问一次物化方式，再出变更计划。命令行：fangcun
                  init --from-existing
                </InfoTip>
              </span>
              <button
                type="button"
                className={`${c.b} ${c.primary}`}
                onClick={() => openRegister(root)}
              >
                登记…
              </button>
            </>
          }
        />
      }
      onClose={onClose}
    ></Inspector>
  );
}

/** 要处理的状态文字加粗（6.3、6.10）。 */
const STRONG: Partial<Record<NonNullable<EffectiveItem["state"]>, true>> = {
  drifted: true,
  updatable: true,
  "script-changed": true,
  "unknown-source": true,
};

const PENDING: Partial<Record<NonNullable<EffectiveItem["state"]>, true>> = {
  drifted: true,
  updatable: true,
  "script-changed": true,
};

/** 提交者一栏：git 历史里的人数，不算 [bot]。 */
function committersText(n: number | undefined): string {
  if (n === undefined) return "…";
  return n === 0 ? "不是 git 仓库" : `${n} 位（git 历史，不算 [bot]）`;
}

/** 已登记的项目：没选中任何一项时看项目本身，「待处理」点一行选中那一项（6.3）。 */
function RegisteredInspector({
  root,
  context,
  summary,
  staged,
  onSync,
  onPick,
  onClose,
}: {
  root: string;
  context: EffectiveContext;
  summary: ProjectSummary | undefined;
  staged: ManifestEdit[];
  onSync: () => void;
  onPick: (id: string) => void;
  onClose: () => void;
}) {
  const suggestion = useQuery({
    queryKey: ["suggest-mode", root],
    queryFn: () => kernel.suggestMode(root),
  });
  const tools = useTools();
  const toolName = (id: string) => tools.data?.find((t) => t.id === id)?.name ?? id;
  const total = context.budget.instructions + context.budget.skills + context.budget.mcp;
  const pending = context.items.filter((i) => i.state && PENDING[i.state]);
  const drifted = pending.filter((i) => i.state === "drifted").length;
  const status =
    drifted > 0
      ? { tone: toneOfState("drifted"), label: `${drifted} 处漂移` }
      : pending.length > 0
        ? { tone: toneOfState(summary?.state ?? "updatable"), label: `${pending.length} 处可更新` }
        : { tone: toneOfState("synced"), label: "已登记" };
  return (
    <Inspector
      kind="项目"
      name={context.projectName}
      status={status}
      decision={
        staged.length > 0 && (
          <DecisionCard
            title={`待同步 ${staged.length} 处改动`}
            foot={
              <>
                <span className={styles.footNote}>未写入磁盘</span>
                <button type="button" className={`${c.b} ${c.primary}`} onClick={onSync}>
                  预览
                </button>
              </>
            }
          >
            <ul className={styles.causes}>
              {staged.map((e) => (
                <li key={causeOf(e, context.items)}>{causeOf(e, context.items)}</li>
              ))}
            </ul>
          </DecisionCard>
        )
      }
      fields={[
        { k: "路径", v: context.projectPath, mono: true },
        {
          k: "清单",
          v: context.hasManifest ? "fangcun.toml · fangcun.lock" : "没有 fangcun.toml",
          mono: context.hasManifest,
        },
        {
          k: "物化方式",
          v: context.mode === "vendor" ? "入库" : context.mode === "link" ? "链接" : "—",
        },
        {
          k: "目标工具",
          v: context.targets.length > 0 ? context.targets.map(toolName).join("、") : "—",
        },
        { k: "提交者", v: committersText(suggestion.data?.committers) },
        {
          k: "常驻",
          v: `${formatK(total)} tokens（估算）/ 项目预算 ${formatLimit(context.budget.limit)}`,
        },
      ]}
      onClose={onClose}
    >
      <Section title={`待处理 ${pending.length}`}>
        {pending.length === 0 ? (
          <p className={styles.pendingNone}>没有待处理的项</p>
        ) : (
          pending.map((item) => (
            <button
              key={item.id}
              type="button"
              className={styles.pendingRow}
              onClick={() => onPick(item.id)}
            >
              <span className={styles.pendingName}>{item.name}</span>
              <StatusDot tone={toneOfState(item.state)} label={item.label} />
            </button>
          ))
        )}
      </Section>
    </Inspector>
  );
}

/** 检查器「最近活动」的时间：「刚刚」「5 分钟前」或「11-10」，和原型的写法一样。 */
function ago(iso: string): string {
  const d = new Date(iso);
  const minutes = Math.floor((Date.now() - d.getTime()) / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

/** 「刚刚」或「11-13 10:42」，和原型的写法一样。 */
function when(iso: string): string {
  const d = new Date(iso);
  if (Date.now() - d.getTime() < 60_000) return "刚刚";
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

/**
 * 历史页签：这个项目的快照，最新的是当前磁盘状态（6.3、M1-14）。
 * 「回滚到此…」在这一条下面展开「回滚到 #N：会写入 N 处」，确认后才写。
 */
function HistoryTab({ root }: { root: string }) {
  const history = useHistory(root, true);
  const client = useQueryClient();
  const [open, setOpen] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const plan = useQuery({
    queryKey: ["rollback-plan", root, open],
    queryFn: () => kernel.planRollback(root, open ?? 0),
    enabled: open !== null,
  });
  if (history.isError) return <p className={page.error}>{history.error.message}</p>;
  if (!history.data) return <p className={page.empty}>正在读取历史…</p>;
  if (history.data.length === 0)
    return (
      <div className={page.empty}>
        <p>还没有快照。同步、收编、升级都会留快照，之后在这里一键回滚。</p>
      </div>
    );
  const rollback = async (snapshot: number) => {
    if (!plan.data || plan.data.blocked) return;
    setBusy(true);
    setError(null);
    try {
      await applyInline(
        plan.data,
        () => `已回滚到 #${snapshot} · 磁盘状态逐字节一致`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
      setOpen(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <ol className={styles.history}>
      {history.data.map((h) => (
        <li key={h.record} className={styles.historyItem}>
          <div className={styles.historyRow}>
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
                className={`${c.b} ${c.small}`}
                aria-expanded={open === h.snapshot}
                onClick={() => {
                  setError(null);
                  setOpen(open === h.snapshot ? null : h.snapshot);
                }}
              >
                回滚到此…
              </button>
            )}
          </div>
          {open === h.snapshot && (
            <DecisionCard
              title={`回滚到 #${h.snapshot}：会写入 ${plan.data?.changes.length ?? "…"} 处`}
              foot={
                <>
                  <span className={styles.footNote}>回滚本身也留快照，可以再撤销</span>
                  <button type="button" className={c.b} onClick={() => setOpen(null)}>
                    取消
                  </button>
                  <button
                    type="button"
                    className={`${c.b} ${c.primary}`}
                    disabled={!plan.data || plan.data.blocked || busy}
                    onClick={() => void rollback(h.snapshot)}
                  >
                    {busy ? "正在回滚…" : "回滚"}
                  </button>
                </>
              }
            >
              <WriteList
                changes={plan.data?.changes}
                loading={plan.isLoading}
                error={error ?? (plan.isError ? plan.error.message : null)}
                blockedNote={
                  plan.data?.blocked ? plan.data.notes.map((n) => n.text).join(" ") : null
                }
              />
            </DecisionCard>
          )}
        </li>
      ))}
    </ol>
  );
}
