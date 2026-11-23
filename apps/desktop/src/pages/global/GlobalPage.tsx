import { useMemo, useState } from "react";
import { Link } from "react-router";
import { FEATURES } from "@/app/features";
import c from "@/components/controls.module.css";
import { DecisionCard } from "@/components/DecisionCard";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import { CommandHint, Inspector, Section } from "@/components/Inspector";
import { Segmented } from "@/components/Segmented";
import { ToolPicker } from "@/components/ToolPicker";
import { formatK, formatLimit, TONE_COLORS, type Tone } from "@/components/tone";
import { useListNav } from "@/components/useListNav";
import { usePresence, useRetainedValue } from "@/components/usePresence";
import { RecentList, WhoButton, WhoList, type WhoTag } from "@/components/WhoBar";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { Change } from "@/ipc/bindings/Change";
import type { GlobalRow } from "@/ipc/bindings/GlobalRow";
import type { GlobalStatus } from "@/ipc/bindings/GlobalStatus";
import { kernel } from "@/ipc/commands";
import { useGlobalOverview, useTools } from "@/ipc/queries";
import { PageToolbar } from "@/shell/PageToolbar";
import { usePlan } from "@/state/plan";
import { useUi } from "@/state/ui";
import page from "../page.module.css";
import styles from "./GlobalPage.module.css";

const KIND_NAME: Record<AssetKind, string> = { skill: "Skill", mcp: "MCP", instruction: "指令" };

/** 6.6「建议」表：标签与颜色。 */
function statusOf(row: GlobalRow): { tone: Tone; label: string } {
  const map: Record<GlobalStatus, { tone: Tone; label: string }> = {
    // v0.1 套装隐藏，标签只写事实（9.3、M1-35）。
    "suggest-bundle": { tone: "info", label: FEATURES.bundles ? "建议转套装" : "少数在用" },
    "never-used": { tone: "warn", label: "从未使用" },
    overridden: { tone: "neutral", label: `被覆盖 ${row.overridden} 处` },
    "same-name-conflict": { tone: "neutral", label: `同名冲突 ${row.conflicts} 处` },
    "unknown-source": { tone: "unknown", label: "未收录" },
    common: { tone: "ok", label: "常用" },
    "most-projects": { tone: "ok", label: "多数项目在用" },
    "every-project": { tone: "ok", label: "每个项目都用" },
    "always-loaded": { tone: "neutral", label: "全文加载" },
    disabled: { tone: "muted", label: "已停用" },
  };
  return map[row.status];
}

/** 建议对应的动作：转套装、移除、先收录、留在全局。 */
type Act = "bundle" | "remove" | "adopt" | "keep";

function actOf(row: GlobalRow): Act {
  switch (row.status) {
    case "suggest-bundle":
      return "bundle";
    case "never-used":
    case "overridden":
    case "same-name-conflict":
      return "remove";
    case "unknown-source":
      return "adopt";
    default:
      return "keep";
  }
}

const CARD_TITLE: Record<Act, string> = {
  bundle: "只有少数项目在用",
  remove: "近 30 天没有项目用到",
  adopt: "在用，但未收录（查不到来源）",
  keep: "每个项目都该有",
};

/** 地图里块的底色：指令、Skill、MCP 三种类型色。 */
const KIND_FILL: Record<AssetKind, string> = {
  instruction: "var(--k1)",
  skill: "var(--k2)",
  mcp: "var(--k3)",
};

type Filter = "all" | AssetKind;

/** 用到它的项目：在用和同名冲突的算。 */
function usersOf(row: GlobalRow): string[] {
  return row.usage.filter((u) => u.mark === "used").map((u) => u.name);
}

/** 给每个项目省下多少：没用到这一项的项目全省。 */
function savingFor(rows: GlobalRow[], project: string): number {
  return rows
    .filter((r) => !usersOf(r).includes(project))
    .reduce((n, r) => n + (r.resident ?? 0), 0);
}

/**
 * 全局：瘦身（6.6，方寸 2）。概要与地图、吸顶的筛选与快速勾选、「谁在用」比例条可展开、
 * 检查器的决定卡片、底部待清理浮条与按建议分组的清理计划。v0.1 不出转成套装（M1-35）。
 */
export function GlobalPage() {
  const tool = useUi((s) => s.tool);
  const setTool = useUi((s) => s.setTool);
  const tools = useTools();
  const overview = useGlobalOverview(tool);
  const [filter, setFilter] = useState<Filter>("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [stuck, setStuck] = useState(false);
  const [whoOpen, setWhoOpen] = useState<Set<string>>(new Set());
  const [idleOpen, setIdleOpen] = useState<string | null>(null);

  const data = overview.data;
  const all = [...(data?.rows ?? [])].sort((a, b) => (b.resident ?? 0) - (a.resident ?? 0));
  const rows = all.filter((r) => filter === "all" || r.kind === filter);
  const current = all.find((r) => r.id === selected) ?? all[0];
  const total = data?.projectTotal ?? 0;
  const limit = Math.max(data?.budget.limit ?? 20000, 1);
  const sum = (list: GlobalRow[]) => list.reduce((n, r) => n + (r.resident ?? 0), 0);
  const totalCost = sum(all);
  // 「照建议瘦身后」只算有动作的建议。v0.1 套装隐藏，少数在用的项算留下（9.3、M1-35）。
  const keepCost = sum(
    all.filter(
      (r) =>
        actOf(r) === "keep" || actOf(r) === "adopt" || (actOf(r) === "bundle" && !FEATURES.bundles),
    ),
  );
  const toolName = tools.data?.find((t) => t.id === tool)?.name ?? tool;
  // 全部项目名：指令文件没有用到可数，展开时列出每个项目。
  const projectNames = [...new Set(all.flatMap((r) => r.usage.map((u) => u.name)))];

  const pickedRows = all.filter((r) => picked.has(r.id));
  const bundleCount = FEATURES.bundles
    ? pickedRows.filter((r) => actOf(r) === "bundle" && usersOf(r).length > 0).length
    : 0;
  const traySave = Math.max(0, ...projectNames.map((p) => savingFor(pickedRows, p)));
  const trayCount = pickedRows.length;
  const trayNames = pickedRows.map((r) => r.name).join("、");
  const trayCanOpen = pickedRows.some((r) => r.inLibrary);
  const traySnap = useMemo(
    () =>
      trayCount > 0
        ? { count: trayCount, bundleCount, traySave, names: trayNames, canOpen: trayCanOpen }
        : null,
    [trayCount, bundleCount, traySave, trayNames, trayCanOpen],
  );
  const trayPresence = usePresence(trayCount > 0, 100);
  const activeTray = useRetainedValue(traySnap);
  const allOn = rows.length > 0 && rows.every((r) => picked.has(r.id));

  const toggle = (id: string) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const pickWhere = (f: (r: GlobalRow) => boolean) =>
    setPicked((prev) => new Set([...prev, ...all.filter(f).map((r) => r.id)]));
  const toggleWho = (id: string) =>
    setWhoOpen((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  const pick = (id: string) => {
    setSelected(id);
    setInspectorOpen(true);
  };
  useListNav(
    rows.map((r) => r.id),
    current?.id,
    pick,
  );

  const kindCount = (k: Filter) =>
    all.filter((r) => k === "all" || r.kind === k).reduce((n, r) => n + r.count, 0);
  const filters = (
    [
      ["all", "全部"],
      ["skill", "Skill"],
      ["mcp", "MCP"],
      ["instruction", "指令"],
    ] as const
  ).filter(([k]) => kindCount(k) > 0);
  const neverN = all.filter((r) => actOf(r) === "remove").length;
  const fewN = all.filter((r) => actOf(r) === "bundle").length;

  const openPlan = () => {
    const ready = pickedRows.filter((r) => r.inLibrary);
    const skipped = pickedRows.filter((r) => !r.inLibrary);
    const n = pickedRows.length;
    const save = formatK(traySave);
    void usePlan.getState().preview({
      load: () =>
        kernel.planRemoveGlobal(
          tool,
          ready.flatMap((r) => r.members.map((name) => ({ kind: r.kind, name }))),
        ),
      title: () => `清理全局层 · ${n} 项`,
      sub: () =>
        skipped.length > 0
          ? `${skipped.map((r) => r.name).join("、")} 还没收录，没放进计划：请先去资产库收录（7.5）`
          : "用到它的项目照常加载 · 应用前各留快照，可撤销",
      group: { of: (ch) => groupOf(ch, ready) },
      done: () => `已清理全局层 ${ready.length} 项，每个项目最多 −${save} · 已留快照`,
      after: () => setPicked(new Set()),
    });
  };

  const otherNote = data?.otherTools[0];

  return (
    <div className={page.page}>
      <PageToolbar title="全局" subtitle="每个项目都会加载这一层，越薄越好">
        <ToolPicker value={tool} onChange={setTool} />
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

      {overview.isError && <p className={page.error}>{overview.error.message}</p>}

      {data && (
        <div className={page.split}>
          <main
            className={`${page.list} ${styles.main}`}
            onScroll={(e) => setStuck(e.currentTarget.scrollTop > 4)}
          >
            <section aria-label="全局层概要" className={styles.summary}>
              <div className={styles.sumRow}>
                <div className={styles.sumCol}>
                  <span className={styles.cap}>全局层 · 每个项目都在缴 · 估算</span>
                  <div className={styles.sumLine}>
                    <span className={styles.big}>{formatK(totalCost)}</span>
                    <span className={styles.sumNote}>
                      / 项目预算 {formatLimit(limit)} · 占 {Math.round((totalCost / limit) * 100)}%
                    </span>
                  </div>
                </div>
                <span className={styles.flex} />
                <div className={`${styles.sumCol} ${styles.sumEnd}`}>
                  <span className={styles.cap}>
                    {FEATURES.bundles ? "照建议瘦身后" : "清理闲置全局项后"}
                  </span>
                  <div className={styles.sumLine}>
                    <span className={styles.after}>{formatK(keepCost)}</span>
                    <span className={styles.saveNote}>
                      每个项目省 {formatK(totalCost - keepCost)}
                    </span>
                  </div>
                </div>
              </div>
              <div
                role="img"
                aria-label={`全局层常驻约 ${formatK(totalCost)}，每个项目都在缴`}
                className={styles.map}
              >
                {all.map((r) => {
                  const w = ((r.resident ?? 0) / limit) * 100;
                  const act = actOf(r);
                  return (
                    <button
                      key={r.id}
                      type="button"
                      className={styles.blk}
                      data-on={r.id === current?.id}
                      data-ring={act === "remove" ? "warn" : act === "bundle" ? "info" : "none"}
                      title={`${r.name} · ${formatK(r.resident ?? 0)} · ${statusOf(r).label}`}
                      onClick={() => pick(r.id)}
                      style={{
                        width: `calc(${w.toFixed(2)}% - 3px)`,
                        backgroundColor: KIND_FILL[r.kind],
                        color: r.kind === "mcp" ? "var(--accent)" : "var(--on-k)",
                      }}
                    >
                      {w >= 8 && (
                        <>
                          <span className={styles.blkName}>{r.name}</span>
                          <span className={styles.blkCost}>{formatK(r.resident ?? 0)}</span>
                        </>
                      )}
                    </button>
                  );
                })}
                <div className={styles.room}>
                  留给项目自己的 {formatK(Math.max(0, limit - totalCost))}
                </div>
              </div>
            </section>

            <div className={styles.sticky} data-stuck={stuck}>
              <div className={styles.filterRow}>
                <Segmented
                  label="类型"
                  value={filter}
                  onChange={setFilter}
                  options={filters.map(([k, label]) => ({
                    value: k,
                    label,
                    count: kindCount(k),
                  }))}
                />
                <span className={styles.flex} />
                <span className={styles.lg} title="快速勾选">
                  <Icon name="picked" size={14} strokeWidth={1.4} />
                  勾选
                </span>
                <button
                  type="button"
                  className={`${c.b} ${c.small}`}
                  title={`勾选近 30 天没有项目用到的 ${neverN} 项`}
                  onClick={() => pickWhere((r) => actOf(r) === "remove")}
                >
                  <span className={styles.dot} style={{ background: "var(--warn-dot)" }} />
                  从未使用<span className={c.count}>{neverN}</span>
                </button>
                <button
                  type="button"
                  className={`${c.b} ${c.small}`}
                  title="勾选只有 1–2 个项目用到的项"
                  onClick={() => pickWhere((r) => actOf(r) === "bundle")}
                >
                  <span className={styles.dot} style={{ background: "var(--info-dot)" }} />
                  少数在用<span className={c.count}>{fewN}</span>
                </button>
              </div>
              <div className={styles.hdr}>
                <span className={styles.cCheck}>
                  <input
                    type="checkbox"
                    checked={allOn}
                    onChange={() =>
                      setPicked(allOn ? new Set() : new Set([...picked, ...rows.map((r) => r.id)]))
                    }
                    aria-label="全选当前列表"
                  />
                </span>
                <span className={styles.cName}>名称 · 位置</span>
                <span className={styles.cWho}>
                  谁在用
                  <InfoTip label="谁在用说明">
                    近 30 天在本机会话里触发过它的项目 / 已登记的项目。只在本机统计，不上传。
                    点比例条展开项目名，右侧「用到它的项目」也有完整名单。指令文件每次会话都全文加载，没有触发可数。
                  </InfoTip>
                </span>
                <span className={styles.cStatus}>建议</span>
                <span className={styles.cCost}>常驻</span>
              </div>
            </div>

            <div className={styles.rows}>
              {rows.length === 0 && (
                <div className={page.empty}>
                  <p>这一类在当前工具的全局层里没有资产</p>
                  <button type="button" className={c.b} onClick={() => setFilter("all")}>
                    清除筛选
                  </button>
                </div>
              )}
              {rows.map((r) => {
                const status = statusOf(r);
                const [tc, dc] = TONE_COLORS[status.tone];
                const loud = ["warn", "info", "unknown"].includes(status.tone);
                const open = whoOpen.has(r.id);
                const full = r.usedBy === null;
                const conflicts = r.usage.filter(
                  (u) => u.mark === "conflict" || u.mark === "overridden",
                );
                const users = usersOf(r);
                const idle = r.usage.filter((u) => u.mark === "unused").map((u) => u.name);
                const tags: WhoTag[] = full
                  ? projectNames.map((name) => ({ name }))
                  : r.usage
                      .filter((u) => u.mark !== "unused")
                      .map((u) =>
                        u.mark === "used"
                          ? { name: u.name }
                          : {
                              name: u.name,
                              tone: "warn" as const,
                              ver: u.mark === "conflict" ? "同名冲突" : "被覆盖",
                            },
                      );
                return (
                  <div key={r.id} data-nav-id={r.id}>
                    <div
                      className={styles.row}
                      data-on={r.id === current?.id}
                      data-struck={r.status === "overridden"}
                    >
                      <input
                        type="checkbox"
                        className={styles.check}
                        checked={picked.has(r.id)}
                        onChange={() => {
                          toggle(r.id);
                          setSelected(r.id);
                        }}
                        aria-label={`勾选 ${r.name}`}
                      />
                      <button type="button" className={styles.nameCell} onClick={() => pick(r.id)}>
                        <span className={styles.nameLine}>
                          <span className={styles.rname}>{r.name}</span>
                          {r.count > 1 && <span className={c.tag}>{r.count} 项</span>}
                          <span className={styles.kindTag}>{KIND_NAME[r.kind]}</span>
                        </span>
                        <span className={styles.loc}>{r.location}</span>
                      </button>
                      <span className={styles.cWho}>
                        <WhoButton
                          id={r.id}
                          used={r.usedBy ?? 0}
                          total={total}
                          full={full}
                          open={open}
                          onToggle={() => toggleWho(r.id)}
                        />
                      </span>
                      <span
                        className={styles.cStatus}
                        style={{ color: tc, fontWeight: loud ? 600 : 400 }}
                      >
                        <span className={styles.dot} style={{ background: dc }} />
                        {status.label}
                      </span>
                      <span className={styles.cCost}>{formatK(r.resident ?? 0)}</span>
                    </div>
                    <WhoList
                      id={r.id}
                      label={`${r.name} 用在哪些项目`}
                      indent={42}
                      open={open}
                      lead={full ? "每次会话都读全文：" : undefined}
                      tags={tags}
                      idle={
                        full || idle.length === 0
                          ? undefined
                          : idle.length === total && users.length + conflicts.length === 0
                            ? `近 30 天 ${total} 个项目都没用到`
                            : `没用到：${idle.join("、")}`
                      }
                    />
                  </div>
                );
              })}
              {otherNote ? (
                <button
                  type="button"
                  className={styles.foot}
                  onClick={() => setTool(otherNote.tool)}
                >
                  <span>
                    另有 {otherNote.count} 项仅在 {otherNote.toolName} 加载 · 切换查看
                  </span>
                  <Icon name="arrowRight" size={12} />
                </button>
              ) : (
                <p className={styles.footText}>只显示 {toolName} 加载的项</p>
              )}
            </div>

            {trayPresence.mounted && activeTray && (
              <section
                className={styles.tray}
                aria-label="待清理项目"
                data-exiting={trayPresence.exiting || undefined}
                aria-hidden={trayPresence.exiting || undefined}
              >
                <span className={styles.trayText}>
                  <span className={styles.trayTitle}>
                    待清理 {activeTray.count} 项 ·{" "}
                    {activeTray.bundleCount > 0 ? `转套装 ${activeTray.bundleCount} · ` : ""}移除{" "}
                    {activeTray.count - activeTray.bundleCount} · 每个项目最多 −
                    {formatK(activeTray.traySave)}
                  </span>
                  <span className={styles.trayNames}>{activeTray.names}</span>
                </span>
                <button
                  type="button"
                  className={`${c.b} ${c.quiet}`}
                  onClick={() => setPicked(new Set())}
                >
                  清空
                </button>
                <button
                  type="button"
                  className={`${c.b} ${c.primary}`}
                  disabled={!activeTray.canOpen}
                  title={
                    activeTray.canOpen
                      ? "逐条预览会写入的变更"
                      : "选中的都未收录：从全局移除只对已收录进资产库的项开放，请先去资产库收录"
                  }
                  onClick={openPlan}
                >
                  预览计划
                </button>
              </section>
            )}
          </main>

          {inspectorOpen && current && (
            <GlobalInspector
              key={current.id}
              row={current}
              total={total}
              projectNames={projectNames}
              idleOpen={idleOpen === current.id}
              inTray={picked.has(current.id)}
              onIdle={() => setIdleOpen(idleOpen === current.id ? null : current.id)}
              onPickIt={() => toggle(current.id)}
              onClose={() => setInspectorOpen(false)}
            />
          )}
        </div>
      )}
    </div>
  );
}

/**
 * 瘦身计划按建议分组（6.6「瘦身计划」）：仍有项目在用的标出来，没人用的直接移除。
 * 一行变更按路径或说明里的名字找回它属于哪一行。
 */
function groupOf(change: Change, rows: GlobalRow[]): string {
  const hit = rows.find((r) =>
    [r.name, ...r.members].some(
      (n) =>
        change.path.endsWith(`/${n}`) ||
        change.path.endsWith(`.${n}`) ||
        change.detail.includes(`${n}@`) ||
        change.detail.includes(`（${n}`) ||
        change.detail.includes(`、${n}`),
    ),
  );
  if (!hit) return "从全局移除";
  if (usersOf(hit).length > 0 && hit.usedBy !== null)
    return "从全局移除 · 仍有项目在用：这些项目会不再加载它，除非它们的清单里另有声明";
  return "从全局移除 · 近 30 天没有项目用到，资产库保留正本，随时可加回";
}

function GlobalInspector({
  row,
  total,
  projectNames,
  idleOpen,
  inTray,
  onIdle,
  onPickIt,
  onClose,
}: {
  row: GlobalRow;
  total: number;
  projectNames: string[];
  idleOpen: boolean;
  inTray: boolean;
  onIdle: () => void;
  onPickIt: () => void;
  onClose: () => void;
}) {
  const act = actOf(row);
  const status = statusOf(row);
  const users = usersOf(row);
  const full = row.usedBy === null;
  const marked = row.usage.filter((u) => u.mark !== "unused");
  const idle = row.usage.filter((u) => u.mark === "unused").map((u) => u.name);
  const freed = act === "bundle" ? total - users.length : total;
  const advice =
    act === "bundle"
      ? FEATURES.bundles
        ? `转成套装，只给 ${users.join("、")} 启用`
        : `只有 ${users.join("、")} 用到，其余 ${freed} 个项目每个多付 ${formatK(row.resident ?? 0)} 常驻（估算）`
      : act === "remove" && row.conflicts === 0 && row.overridden === 0
        ? ""
        : row.advice;
  const cost = row.resident ?? 0;
  const cli = row.members[0] ?? row.name;
  const notAdopted = !row.inLibrary && (act === "bundle" || act === "remove");
  return (
    <Inspector
      kind={`${KIND_NAME[row.kind]}${row.count > 1 ? ` · ${row.count} 项` : ""} · 全局层`}
      name={row.name}
      status={status}
      intro={row.summary}
      decision={
        <DecisionCard
          title={CARD_TITLE[act]}
          metric={
            (act === "bundle" && FEATURES.bundles) || act === "remove" ? (
              <>
                −{formatK(cost)} <span className={styles.times}>× {freed} 个项目</span>
              </>
            ) : undefined
          }
          foot={
            act === "keep" ? (
              <span className={styles.keep}>
                <span className={styles.dot} style={{ background: "var(--ok-dot)" }} />
                留在全局 · 不用动
              </span>
            ) : act === "adopt" ? (
              <>
                <span className={styles.flex} />
                <Link
                  to={`/library?asset=${encodeURIComponent(cli)}`}
                  className={`${c.b} ${c.primary}`}
                  title="收录到资产库，记录来源与版本"
                >
                  去资产库收录 →
                </Link>
              </>
            ) : act === "bundle" && !FEATURES.bundles ? undefined : act === "bundle" ? (
              <>
                <button
                  type="button"
                  className={c.b}
                  onClick={onPickIt}
                  disabled={notAdopted}
                  title={
                    notAdopted
                      ? "从全局移除只对已收录进资产库的项开放，请先去资产库收录"
                      : "放入下方待清理列表，和其他项一起出变更计划"
                  }
                >
                  {inTray ? "✓ 已在待清理" : "加入待清理"}
                </button>
                <span className={styles.flex} />
                {notAdopted && (
                  <Link to={`/library?asset=${encodeURIComponent(cli)}`} className={c.b}>
                    去资产库收录 →
                  </Link>
                )}
              </>
            ) : (
              <>
                <span className={styles.lg} style={{ flex: 1 }}>
                  可加回
                  <InfoTip label="可加回说明" align="right">
                    资产库保留正本，随时可加回。
                  </InfoTip>
                </span>
                {notAdopted && (
                  <Link to={`/library?asset=${encodeURIComponent(cli)}`} className={c.b}>
                    去资产库收录 →
                  </Link>
                )}
                <button
                  type="button"
                  className={inTray ? c.b : `${c.b} ${c.primary}`}
                  onClick={onPickIt}
                  disabled={notAdopted}
                  title={
                    notAdopted
                      ? "从全局移除只对已收录进资产库的项开放，请先去资产库收录"
                      : "放入下方待清理列表，和其他项一起出变更计划"
                  }
                >
                  {inTray ? "✓ 已在待清理" : "加入待清理"}
                </button>
              </>
            )
          }
        >
          {advice && <p className={styles.advice}>{advice}</p>}
        </DecisionCard>
      }
      onClose={onClose}
    >
      <Section
        title="用到它的项目 · 近 30 天"
        aside={
          <span className={styles.asideNum}>
            {full ? "全文加载" : `${users.length} / ${total}`}
          </span>
        }
      >
        {full ? (
          <div className={styles.useRow}>
            <span className={styles.useSwatch} style={{ background: "var(--fill-3)" }} />
            {projectNames.length || total} 个项目每次会话都读全文
          </div>
        ) : (
          <>
            {marked.map((u) => {
              const conflict = u.mark !== "used";
              return (
                <div key={u.name} className={styles.useRow}>
                  <span
                    className={styles.useSwatch}
                    style={
                      conflict
                        ? { boxShadow: "inset 0 0 0 2px var(--warn-dot)" }
                        : { background: "var(--accent)" }
                    }
                  />
                  <span className={styles.useName}>{u.name}</span>
                  <span
                    className={styles.useNote}
                    style={{ color: conflict ? "var(--warn)" : "var(--accent)" }}
                  >
                    {u.mark === "used" ? "用到" : u.mark === "conflict" ? "同名冲突" : "被覆盖"}
                  </span>
                </div>
              );
            })}
            {idle.length > 0 && (
              <>
                <button
                  type="button"
                  className={`${c.b} ${c.quiet} ${c.small} ${styles.idleToggle}`}
                  aria-expanded={idleOpen}
                  onClick={onIdle}
                >
                  <Icon name="chevron" size={10} strokeWidth={1.8} className={styles.idleChev} />
                  {idle.length === total
                    ? `${total} 个项目都没用到`
                    : `其余 ${idle.length} 个没用到`}
                </button>
                <div
                  className={styles.idleBody}
                  data-open={idleOpen}
                  aria-hidden={!idleOpen || undefined}
                >
                  <div className={styles.idleInner}>
                    {idle.map((p) => (
                      <div key={p} className={styles.idleRow}>
                        {p}
                      </div>
                    ))}
                  </div>
                </div>
              </>
            )}
          </>
        )}
      </Section>
      <Section title="分发到哪些工具">
        {row.distribution.map((d) => (
          <div key={d.tool} className={styles.distRow}>
            <span className={styles.distTool}>{d.toolName}</span>
            <span className={styles.distPath} data-on={d.present}>
              {d.present ? (d.path ?? d.label) : row.kind === "skill" ? "没有分发" : d.label}
            </span>
          </div>
        ))}
      </Section>
      <Section title="详情">
        <dl className={styles.fields}>
          <dt>位置</dt>
          <dd className={styles.mono}>{row.location}</dd>
          <dt>来源</dt>
          <dd
            className={styles.mono}
            style={row.status === "unknown-source" ? { color: "var(--unk)" } : undefined}
          >
            {row.source ?? "来源未知"}
          </dd>
          <dt>版本</dt>
          <dd className={styles.mono}>{row.version ?? "—"}</dd>
          <dt>常驻</dt>
          <dd style={cost >= 3000 ? { color: "var(--warn)" } : undefined}>
            {formatK(cost)} / 每个项目（估算）
          </dd>
          <dt>含脚本</dt>
          <dd style={row.hasScripts || row.command ? { color: "var(--danger)" } : undefined}>
            {row.command ? `启动命令 ${row.command}` : row.hasScripts ? "是" : "否"}
          </dd>
        </dl>
      </Section>
      <Section title="最近活动">
        <RecentList items={row.recent} />
        <div className={styles.cli}>
          <CommandHint command={`fangcun why ${cli} --global`} />
        </div>
      </Section>
    </Inspector>
  );
}
