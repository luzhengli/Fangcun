import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useLocation, useSearchParams } from "react-router";
import c from "@/components/controls.module.css";
import { DecisionCard, Done, WriteList } from "@/components/DecisionCard";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import { CommandHint, Inspector, Section } from "@/components/Inspector";
import { Segmented } from "@/components/Segmented";
import { formatK, TONE_COLORS, type Tone } from "@/components/tone";
import { useListNav } from "@/components/useListNav";
import { RecentList, WhoButton, WhoList, type WhoTag } from "@/components/WhoBar";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { LibraryRow } from "@/ipc/bindings/LibraryRow";
import type { LibraryStatus } from "@/ipc/bindings/LibraryStatus";
import type { SourceGroup } from "@/ipc/bindings/SourceGroup";
import type { VersionTag } from "@/ipc/bindings/VersionTag";
import { kernel } from "@/ipc/commands";
import { useLibraryOverview } from "@/ipc/queries";
import { PageToolbar } from "@/shell/PageToolbar";
import { applyInline, usePlan } from "@/state/plan";
import page from "../page.module.css";
import styles from "./LibraryPage.module.css";

const KIND_LONG: Record<AssetKind, string> = {
  skill: "Skill",
  mcp: "MCP 服务",
  instruction: "指令文件",
};
/** 行首方块：字、底色、字色。照抄原型的 KB。 */
const BADGE: Record<AssetKind, [string, string, string]> = {
  skill: ["S", "var(--k2)", "var(--on-k)"],
  mcp: ["M", "var(--k3)", "var(--accent)"],
  instruction: ["指", "var(--k1)", "var(--on-k)"],
};

function statusOf(row: LibraryRow): { tone: Tone; label: string } {
  const map: Record<LibraryStatus, { tone: Tone; label: string }> = {
    drift: { tone: "warn", label: `${row.drift} 处本地已改` },
    updatable: { tone: "info", label: "可更新" },
    "unknown-source": { tone: "unknown", label: "未收录" },
    synced: { tone: "ok", label: "已同步" },
    adopted: { tone: "ok", label: "已收录" },
    unused: { tone: "muted", label: "没有项目在用" },
  };
  return map[row.status];
}

const TAG: Record<VersionTag, { tone: Tone; label: string }> = {
  locked: { tone: "ok", label: "锁定中" },
  latest: { tone: "info", label: "最新" },
  fork: { tone: "warn", label: "已分化" },
  old: { tone: "neutral", label: "旧版" },
  unadopted: { tone: "unknown", label: "未收录" },
};

/** 语义浅底，只用「墨格」的几种。 */
const WEAK: Record<Tone, string> = {
  neutral: "var(--fill-1)",
  muted: "var(--fill-1)",
  ok: "var(--ok-weak)",
  warn: "var(--warn-weak)",
  info: "var(--info-weak)",
  unknown: "var(--unk-weak)",
  danger: "var(--danger-weak)",
};

const GROUP_NAME: Record<SourceGroup, string> = {
  local: "本地开发",
  github: "GitHub",
  npm: "npm",
  unknown: "未收录",
  other: "其他",
};

type Filter = "all" | AssetKind;

function Badge({ kind, big }: { kind: AssetKind; big?: boolean }) {
  const [text, bg, fg] = BADGE[kind];
  return (
    <span
      aria-hidden="true"
      className={styles.badge}
      data-big={big}
      style={{ background: bg, color: fg }}
    >
      {text}
    </span>
  );
}

/** 「谁在用」展开后的项目名：多版本并存时带版本号，本地已改标橙。 */
function whoTags(row: LibraryRow): WhoTag[] {
  const multi = row.versions.filter((v) => v.users.length > 0).length > 1;
  return row.versions.flatMap((v) =>
    v.users.map((u) => ({
      name: u.name,
      tone: u.name.startsWith("全部") ? ("all" as const) : u.drift ? ("warn" as const) : undefined,
      ver: u.drift ? "本地已改" : multi ? v.label : undefined,
    })),
  );
}

/**
 * 资产库（6.4，方寸 2）：四个数字、类型与来源筛选、「谁在用」比例条、版本轨道。
 * 未收录资产在检查器里提供「收录到资产库」，点了就写，原地显示结果和撤销。
 */
export function LibraryPage() {
  const overview = useLibraryOverview();
  const [filter, setFilter] = useState<Filter>("all");
  const [source, setSource] = useState("all");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [whoOpen, setWhoOpen] = useState<Set<string>>(new Set());
  const [adopted, setAdopted] = useState<{ name: string; record: string | null } | null>(null);
  const search = useRef<HTMLInputElement>(null);

  // 从项目页、全局页的「去资产库收录」过来时带着 ?asset=，选中那一项。
  const navKey = useLocation().key;
  const [params] = useSearchParams();
  const [seenNavKey, setSeenNavKey] = useState<string | null>(null);
  const [stuck, setStuck] = useState(false);
  if (navKey !== seenNavKey) {
    setSeenNavKey(navKey);
    const wanted = params.get("asset");
    if (wanted) {
      setSelected(wanted);
      setFilter("all");
      setSource("all");
      setQuery("");
      setInspectorOpen(true);
    }
  }

  // ⌘F 聚焦「筛选资产」。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const data = overview.data;
  const all = data?.rows ?? [];
  const sourceKey = (r: LibraryRow) =>
    r.group === "unknown"
      ? "来源未知"
      : r.group === "npm"
        ? "npm"
        : r.source.startsWith("local:")
          ? `local:${r.source.slice(6).split("/")[0]}`
          : r.source;
  const q = query.trim().toLowerCase();
  const shown = all.filter(
    (r) =>
      (filter === "all" || r.kind === filter) &&
      (source === "all" || sourceKey(r) === source) &&
      (!q || `${r.name} ${r.source}`.toLowerCase().includes(q)),
  );
  // 按名字选中：收录后这一行的编号会变，名字不变。
  const current = shown.find((r) => r.name === selected) ?? shown[0];
  const total = data?.projectTotal ?? 0;
  useListNav(
    shown.map((r) => r.id),
    current?.id,
    (id) => {
      const hit = shown.find((x) => x.id === id);
      if (hit) {
        setSelected(hit.name);
        setInspectorOpen(true);
      }
    },
  );

  const toggleWho = (id: string) =>
    setWhoOpen((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const kinds = data?.kinds ?? { skill: 0, mcp: 0, instruction: 0 };
  const kindTotal = kinds.skill + kinds.mcp + kinds.instruction;
  const filters: [Filter, string, number][] = [
    ["all", "全部", kindTotal],
    ["skill", "Skill", kinds.skill],
    ["mcp", "MCP", kinds.mcp],
    ["instruction", "指令", kinds.instruction],
  ];
  const sources = data?.sources ?? [];
  const git = sources.filter((s) => s.group === "github").length;
  const local = sources.filter((s) => s.group === "local" || s.group === "unknown").length;
  const npm = sources.some((s) => s.group === "npm");
  const sourceNote = [
    git > 0 ? `${git} 个 git 仓库` : null,
    local > 0 ? `${local} 个本地` : null,
    npm ? "npm" : null,
  ]
    .filter(Boolean)
    .join("、");
  const todo = (data?.drift ?? 0) + (data?.updatable ?? 0) + (data?.unknown ?? 0);
  const todoNote = [
    data?.drift ? `本地已改 ${data.drift}` : null,
    data?.updatable ? `可更新 ${data.updatable}` : null,
    data?.unknown ? `未收录 ${data.unknown}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const groups = (["local", "github", "npm", "unknown", "other"] as SourceGroup[])
    .map((g) => ({ g, list: sources.filter((s) => s.group === g) }))
    .filter((x) => x.list.length > 0);

  return (
    <div className={page.page}>
      <PageToolbar title="资产库" subtitle="每项资产本机只有一份正本 · 多版本并存">
        <label className={styles.search}>
          <Icon name="search" size={14} strokeWidth={1.6} />
          <span className={styles.srOnly}>筛选资产</span>
          <input
            ref={search}
            value={query}
            placeholder="筛选资产"
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setQuery("");
                e.currentTarget.blur();
              }
            }}
          />
          <span className={c.kbd}>⌘F</span>
        </label>
        <button
          type="button"
          className={c.b}
          disabled
          title="从 git 仓库、本地目录或 npm 包加入资产库。还没开放：先用 fangcun add <来源>@<名字>"
        >
          添加来源…
        </button>
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
            <section aria-label="概要" className={styles.tiles}>
              <Tile label="已收录资产" value={Math.max(0, kindTotal - data.unknown)}>
                {data.unknown > 0
                  ? `另有 ${data.unknown} 项未收录 · 共发现 ${kindTotal} 项`
                  : `Skill ${kinds.skill} · MCP ${kinds.mcp} · 指令 ${kinds.instruction}`}
              </Tile>
              <Tile label="来源" value={sources.length}>
                {sourceNote || "还没有来源"}
              </Tile>
              <Tile label="多版本并存" value={data.multiVersion}>
                各项目固定自己要的那版
              </Tile>
              <Tile label="要处理" value={todo} warn={todo > 0}>
                {todoNote || "没有要处理的"}
              </Tile>
            </section>

            <div className={styles.sticky} data-stuck={stuck}>
              <Segmented
                label="类型"
                value={filter}
                onChange={setFilter}
                options={filters.map(([k, label, n]) => ({ value: k, label, count: n }))}
              />
              <span className={styles.flex} />
              <label className={styles.srcLabel}>
                来源
                <select
                  className={styles.select}
                  value={source}
                  onChange={(e) => setSource(e.target.value)}
                >
                  <option value="all">全部来源（{sources.length}）</option>
                  {groups.map(({ g, list }) => (
                    <optgroup key={g} label={GROUP_NAME[g]}>
                      {list.map((s) => (
                        <option key={s.source} value={s.source}>
                          {s.source}（{s.count}）
                        </option>
                      ))}
                    </optgroup>
                  ))}
                </select>
              </label>
            </div>
            <div className={styles.hdr}>
              <span className={styles.cMain}>
                名称 · 来源
                <InfoTip label="名称与来源说明">
                  每项资产在本机只存一份正本，按内容哈希寻址；名称下面是它的来源。
                </InfoTip>
              </span>
              <span className={styles.cWho}>
                谁在用
                <InfoTip label="谁在用说明">
                  固定了这项资产的项目 / 已登记的项目。在全局层的算全部项目。点比例条展开项目名。
                </InfoTip>
              </span>
              <span className={styles.cVer}>版本</span>
              <span className={styles.cStatus}>状态</span>
            </div>
            <div className={styles.rows}>
              {shown.length === 0 && (
                <div className={page.empty}>
                  <p>{all.length === 0 ? "资产库还是空的" : "没有符合筛选的资产"}</p>
                  {all.length > 0 && (
                    <button
                      type="button"
                      className={c.b}
                      onClick={() => {
                        setFilter("all");
                        setSource("all");
                        setQuery("");
                      }}
                    >
                      清除筛选
                    </button>
                  )}
                </div>
              )}
              {shown.map((r) => {
                const status = statusOf(r);
                const [tc, dc] = TONE_COLORS[status.tone];
                const open = whoOpen.has(r.id);
                const inUse = r.versions.filter((v) => v.users.length > 0).length;
                const names = new Set(r.versions.flatMap((v) => v.users.map((u) => u.name)));
                const everyone = [...names].some((n) => n.startsWith("全部") || n === "全局层");
                const idle = everyone ? [] : data.projects.filter((p) => !names.has(p));
                return (
                  <div key={r.id} data-nav-id={r.id}>
                    <div className={styles.row} data-on={r.id === current?.id}>
                      <button
                        type="button"
                        className={styles.mainCell}
                        aria-pressed={r.id === current?.id}
                        onClick={() => {
                          setSelected(r.name);
                          setInspectorOpen(true);
                        }}
                      >
                        <Badge kind={r.kind} />
                        <span className={styles.nameCol}>
                          <span className={styles.rname}>{r.name}</span>
                          <span className={styles.src}>{r.source}</span>
                        </span>
                      </button>
                      <span className={styles.cWho}>
                        <WhoButton
                          id={r.id}
                          used={r.usedBy}
                          total={total}
                          open={open}
                          onToggle={() => toggleWho(r.id)}
                        />
                      </span>
                      <span className={styles.cVer}>
                        {r.versions.length > 1
                          ? `${r.versions.length} 个版本 · ${inUse} 个在用`
                          : (r.versions[0]?.label ?? "—")}
                      </span>
                      <span
                        className={styles.cStatus}
                        style={{ color: tc, fontWeight: status.tone !== "ok" ? 600 : 400 }}
                      >
                        <span className={styles.dot} style={{ background: dc }} />
                        {status.label}
                      </span>
                    </div>
                    <WhoList
                      id={r.id}
                      label={`${r.name} 用在哪些项目`}
                      indent={48}
                      open={open}
                      tags={whoTags(r)}
                      idle={idle.length > 0 ? `没用到：${idle.join("、")}` : undefined}
                    />
                  </div>
                );
              })}
            </div>
          </main>

          {inspectorOpen && current && (
            <LibraryInspector
              key={current.name}
              row={current}
              total={total}
              adopted={adopted?.name === current.name ? adopted : null}
              onAdopted={(record) => setAdopted({ name: current.name, record })}
              onClose={() => setInspectorOpen(false)}
            />
          )}
        </div>
      )}
    </div>
  );
}

function Tile({
  label,
  value,
  warn,
  children,
}: {
  label: string;
  value: number;
  warn?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={styles.tile}>
      <span className={styles.cap}>{label}</span>
      <span className={styles.tileNum} data-warn={warn}>
        {value}
      </span>
      <span className={styles.tileNote}>{children}</span>
    </div>
  );
}

function LibraryInspector({
  row,
  total,
  adopted,
  onAdopted,
  onClose,
}: {
  row: LibraryRow;
  total: number;
  adopted: { record: string | null } | null;
  onAdopted: (record: string | null) => void;
  onClose: () => void;
}) {
  const status = statusOf(row);
  const fields = [
    { k: "来源", v: row.source, mono: true, unk: row.adoptable },
    { k: "正本", v: row.store ?? "尚未进入资产库", mono: true },
    {
      k: "常驻",
      v: row.resident === null ? "没有估算" : `${formatK(row.resident)} tokens（估算）`,
    },
    {
      k: "含脚本",
      v: row.hasScripts ? (row.kind === "mcp" ? "启动命令" : "是") : "否",
      danger: row.hasScripts,
    },
  ];
  return (
    <Inspector
      kind={`${KIND_LONG[row.kind]} · 资产库`}
      name={row.name}
      badge={<Badge kind={row.kind} big />}
      status={status}
      intro={row.summary}
      decision={
        adopted ? (
          <Done
            animate
            title="已收录"
            desc="已存入本机资产库统一管理，原位置换为受管软链，Agent 读取不受影响，事前已备份。"
            onUndo={
              adopted.record
                ? () =>
                    void usePlan.getState().preview({
                      load: () => kernel.planRollbackRecord(adopted.record ?? ""),
                      done: (_, plan) => `已撤销：${plan.title}`,
                      undoable: false,
                    })
                : undefined
            }
          />
        ) : row.adoptable ? (
          <AdoptCard row={row} total={total} onDone={onAdopted} />
        ) : undefined
      }
      onClose={onClose}
    >
      <Section
        title="版本 · 谁固定在哪一版"
        aside={<span className={styles.asideNum}>{row.versions.length} 个版本</span>}
      >
        <ol className={styles.track}>
          {row.versions.map((v, i) => {
            const tag = TAG[v.tag];
            const [, dot] = TONE_COLORS[tag.tone];
            return (
              <li key={`${v.hash}:${v.label}`} className={styles.node}>
                <span aria-hidden="true" className={styles.rail}>
                  {i < row.versions.length - 1 && <span className={styles.stem} />}
                  <span
                    className={styles.knob}
                    style={
                      v.users.length > 0
                        ? { background: dot }
                        : { background: "var(--insp)", border: `2px solid ${dot}` }
                    }
                  />
                </span>
                <div className={styles.nodeBody}>
                  <span className={styles.nodeHead}>
                    <span className={styles.verLabel}>{v.label}</span>
                    <span
                      className={c.tag}
                      style={{
                        background: WEAK[tag.tone],
                        color: TONE_COLORS[tag.tone][0],
                        boxShadow: "none",
                      }}
                    >
                      {tag.label}
                    </span>
                    <span className={styles.flex} />
                    <span className={styles.verNote}>{v.note}</span>
                  </span>
                  <span className={styles.users}>
                    {v.users.length === 0 ? (
                      <span className={styles.nobody}>没有项目固定在这一版</span>
                    ) : (
                      v.users.map((u) => (
                        <span
                          key={`${u.name}:${u.mode}`}
                          className={c.tag}
                          data-drift={u.drift}
                          style={
                            u.drift
                              ? {
                                  background: "var(--warn-weak)",
                                  color: "var(--warn)",
                                  boxShadow: "none",
                                }
                              : { background: "var(--raised)", color: "var(--tx-1)" }
                          }
                          title={u.path ?? undefined}
                        >
                          {u.name}
                          <span className={styles.mode}>
                            {u.mode}
                            {u.drift ? " · 本地已改" : ""}
                          </span>
                        </span>
                      ))
                    )}
                  </span>
                </div>
              </li>
            );
          })}
        </ol>
      </Section>
      <Section title="详情">
        <dl className={styles.fields}>
          {fields.map((f) => (
            <div key={f.k} className={styles.fieldRow}>
              <dt>{f.k}</dt>
              <dd
                className={f.mono ? styles.mono : undefined}
                style={
                  f.danger
                    ? { color: "var(--danger)" }
                    : f.unk
                      ? { color: "var(--unk)" }
                      : undefined
                }
              >
                {f.v}
              </dd>
            </div>
          ))}
        </dl>
      </Section>
      <Section title="最近活动">
        <RecentList items={row.recent} />
        <div className={styles.cli}>
          <CommandHint
            command={row.adoptable ? `fangcun adopt ${row.name}` : `fangcun why ${row.name}`}
          />
        </div>
      </Section>
    </Inspector>
  );
}

/** 未收录资产的决定卡片（6.4）：收录到资产库，点了就写。 */
function AdoptCard({
  row,
  total,
  onDone,
}: {
  row: LibraryRow;
  total: number;
  onDone: (record: string | null) => void;
}) {
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const plan = useQuery({
    queryKey: ["adopt-plan", row.name],
    queryFn: () => kernel.planAdopt(row.name),
  });
  const keeps =
    row.usedBy >= total && total > 0 ? `${total} 个项目照常加载` : "用到它的项目照常加载";
  const apply = async () => {
    if (!plan.data || plan.data.blocked) return;
    setBusy(true);
    setError(null);
    try {
      const applied = await applyInline(
        plan.data,
        () => `已收录 ${row.name} · 原位置换为受管软链 · 已留快照`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
      onDone(applied.record);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };
  return (
    <DecisionCard
      title="收录到资产库"
      foot={
        <>
          <span className={styles.lg}>
            事前备份 · 可撤销
            <InfoTip label="收录说明" align="right">
              把 {row.source} 存入本机资产库统一管理，原位置换为受管软链。Agent
              读取不受影响，只换软链、不改清单。
            </InfoTip>
          </span>
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            disabled={!plan.data || plan.data.blocked || busy}
            onClick={() => void apply()}
          >
            {busy ? "正在收录…" : "收录到资产库"}
          </button>
        </>
      }
    >
      <p className={styles.advice}>
        存入本机资产库，原位置自动换为软链（Agent 读取不受影响）· {keeps}
      </p>
      {plan.data && plan.data.changes.length === 0 ? (
        <p className={styles.advice}>
          该资产已在资产库中，点击上方「收录到资产库」即可完成收录登记。
        </p>
      ) : (
        <WriteList
          changes={plan.data?.changes}
          loading={plan.isLoading}
          error={error ?? (plan.isError ? plan.error.message : null)}
          blockedNote={plan.data?.blocked ? plan.data.notes.map((n) => n.text).join(" ") : null}
        />
      )}
    </DecisionCard>
  );
}
