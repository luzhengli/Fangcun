import { useState } from "react";
import { Link } from "react-router";
import { Inspector, Section } from "@/components/Inspector";
import { StatusDot } from "@/components/StatusDot";
import { ToolSwitch } from "@/components/ToolSwitch";
import { formatK, formatLimit, TONE_COLORS, type Tone } from "@/components/tone";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { GlobalRow } from "@/ipc/bindings/GlobalRow";
import type { GlobalStatus } from "@/ipc/bindings/GlobalStatus";
import { useGlobalOverview } from "@/ipc/queries";
import { useUi } from "@/state/ui";
import page from "../page.module.css";
import styles from "./GlobalPage.module.css";

const KIND_NAME: Record<AssetKind, string> = { skill: "Skill", mcp: "MCP", instruction: "指令" };

/** 6.6「状态」表：标签与颜色。 */
function statusOf(row: GlobalRow): { tone: Tone; label: string } {
  const map: Record<GlobalStatus, { tone: Tone; label: string }> = {
    "suggest-bundle": { tone: "info", label: "建议转套装" },
    "never-used": { tone: "warn", label: "从未使用" },
    overridden: { tone: "muted", label: `被覆盖 ${row.overridden} 处` },
    "same-name-conflict": { tone: "muted", label: `同名冲突 ${row.conflicts} 处` },
    "unknown-source": { tone: "unknown", label: "来源未知" },
    common: { tone: "ok", label: "常用" },
    "most-projects": { tone: "ok", label: "多数项目在用" },
    "every-project": { tone: "ok", label: "每个项目都用" },
    "always-loaded": { tone: "neutral", label: "全文加载" },
    disabled: { tone: "muted", label: "已停用" },
  };
  return map[row.status];
}

type Filter = "all" | AssetKind;

/** 全局：瘦身（6.6）。M0 只读：列表、预算、建议；转成套装与从全局移除在 M1。 */
export function GlobalPage() {
  const tool = useUi((s) => s.tool);
  const setTool = useUi((s) => s.setTool);
  const overview = useGlobalOverview(tool);
  const [filter, setFilter] = useState<Filter>("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);

  const data = overview.data;
  const rows = (data?.rows ?? []).filter((r) => filter === "all" || r.kind === filter);
  const current = rows.find((r) => r.id === selected) ?? rows[0];
  const total = data?.projectTotal ?? 0;
  const pickedRows = (data?.rows ?? []).filter((r) => picked.has(r.id));
  const pickedCount = pickedRows.reduce((n, r) => n + r.count, 0);
  const pickedSave = pickedRows.reduce((n, r) => n + (r.resident ?? 0), 0);
  const allOn = rows.length > 0 && rows.every((r) => picked.has(r.id));

  const pick = (ids: string[]) => setPicked(new Set(ids));
  const toggle = (id: string) =>
    setPicked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const budget = data?.budget;
  const budgetTotal = budget ? budget.instructions + budget.skills + budget.mcp : 0;
  const limit = Math.max(budget?.limit ?? 1, 1);
  const otherNote = (data?.otherTools ?? [])
    .map((o) => `另有 ${o.count} 项只被 ${o.toolName} 加载，切换工具查看。`)
    .join("");

  return (
    <div className={page.page}>
      <header className={styles.header}>
        <div className={styles.titleLine}>
          <h1 className={page.title}>全局</h1>
          <span className={styles.subtitle}>每个项目都会加载这一层，越薄越好</span>
          <ToolSwitch value={tool} onChange={setTool} />
        </div>
        {budget && (
          <>
            <div className={styles.budgetLine}>
              <span className={styles.budgetText}>
                给每个项目带来常驻 <strong>{formatK(budgetTotal)}</strong> / 项目预算{" "}
                {formatLimit(budget.limit)} · 估算值
              </span>
              <span className={page.subtitle} style={{ flex: 1 }} />
              <span className={styles.legends}>
                <span>
                  <i style={{ background: "var(--bar-1)" }} />
                  指令 {formatK(budget.instructions)}
                </span>
                <span>
                  <i style={{ background: "var(--bar-2)" }} />
                  Skill {formatK(budget.skills)}
                </span>
                <span>
                  <i style={{ background: "var(--bar-3)" }} />
                  {budget.mcpUnestimated > 0 && budget.mcp === 0
                    ? `MCP ${budget.mcpUnestimated} 个未估算`
                    : `MCP ${formatK(budget.mcp)}`}
                </span>
              </span>
            </div>
            <div
              role="img"
              aria-label={`常驻 ${formatK(budgetTotal)}，估算值`}
              className={styles.bar}
            >
              <span
                style={{
                  width: `${(budget.instructions / limit) * 100}%`,
                  background: "var(--bar-1)",
                }}
              />
              <span
                style={{ width: `${(budget.skills / limit) * 100}%`, background: "var(--bar-2)" }}
              />
              <span
                style={{ width: `${(budget.mcp / limit) * 100}%`, background: "var(--bar-3)" }}
              />
            </div>
          </>
        )}
      </header>

      {overview.isError && <p className={page.error}>{overview.error.message}</p>}

      {data && (
        <div className={page.split}>
          <section aria-label="全局层资产" className={`${page.list} ${styles.listColumn}`}>
            <div className={styles.toolbar}>
              <fieldset aria-label="按类型筛选" className={page.chips}>
                {(
                  [
                    ["all", `全部 ${data.kinds.skill + data.kinds.mcp + data.kinds.instruction}`],
                    ["skill", `Skill ${data.kinds.skill}`],
                    ["mcp", `MCP ${data.kinds.mcp}`],
                    ["instruction", `指令 ${data.kinds.instruction}`],
                  ] as const
                ).map(([key, label]) => (
                  <button
                    key={key}
                    type="button"
                    className={page.chip}
                    aria-pressed={filter === key}
                    onClick={() => setFilter(key)}
                  >
                    {label}
                  </button>
                ))}
              </fieldset>
              <span style={{ flex: 1 }} />
              <span className={styles.quickLabel}>快速选择</span>
              <button
                type="button"
                className={styles.quick}
                onClick={() =>
                  pick(data.rows.filter((r) => r.status === "suggest-bundle").map((r) => r.id))
                }
              >
                很少用到 · {data.rarelyUsed}
              </button>
              <button
                type="button"
                className={styles.quick}
                onClick={() =>
                  pick(
                    data.rows
                      .filter(
                        (r) =>
                          r.status === "never-used" ||
                          r.status === "overridden" ||
                          r.status === "same-name-conflict",
                      )
                      .map((r) => r.id),
                  )
                }
              >
                从未使用 · {data.neverUsed}
              </button>
            </div>
            <div className={styles.colHead}>
              <input
                type="checkbox"
                checked={allOn}
                onChange={() => (allOn ? pick([]) : pick(rows.map((r) => r.id)))}
                aria-label="全选当前列表"
              />
              <span className={styles.cols}>
                <span className={styles.colName}>名称 · 位置</span>
                <span className={styles.colKind}>类型</span>
                <span className={styles.colUse}>用到它的项目</span>
                <span className={styles.colStatus}>状态</span>
                <span className={styles.colCost}>常驻 ↓</span>
              </span>
            </div>
            {rows.map((row) => {
              const status = statusOf(row);
              const used = row.usedBy ?? 0;
              return (
                <div key={row.id} className={styles.row} data-current={row.id === current?.id}>
                  <input
                    type="checkbox"
                    checked={picked.has(row.id)}
                    onChange={() => toggle(row.id)}
                    aria-label={`选择 ${row.name}`}
                  />
                  <button
                    type="button"
                    className={styles.cols}
                    aria-pressed={row.id === current?.id}
                    onClick={() => {
                      setSelected(row.id);
                      setInspectorOpen(true);
                    }}
                  >
                    <span className={styles.colName}>
                      <span className={styles.nameLine}>
                        <span
                          className={`${page.name} ${row.status === "overridden" ? page.struck : ""}`}
                        >
                          {row.name}
                        </span>
                        {row.count > 1 && <span className={styles.count}>{row.count} 项</span>}
                      </span>
                      <span className={styles.loc}>{row.location}</span>
                    </span>
                    <span className={styles.colKind}>{KIND_NAME[row.kind]}</span>
                    <span className={styles.colUse}>
                      {row.usedBy === null ? (
                        <span className={styles.useText}>—</span>
                      ) : (
                        <>
                          <span aria-hidden="true" className={styles.useBar}>
                            <span
                              style={{
                                width: `${total > 0 ? (used / total) * 100 : 0}%`,
                                background:
                                  used === 0
                                    ? "transparent"
                                    : used <= 2
                                      ? "var(--warn-dot)"
                                      : "var(--ok-dot)",
                              }}
                            />
                          </span>
                          <span className={styles.useText}>
                            {used} / {total}
                          </span>
                        </>
                      )}
                    </span>
                    <span className={styles.colStatus}>
                      <StatusDot tone={status.tone} label={status.label} />
                    </span>
                    <span className={styles.colCost}>
                      {row.resident === null ? "—" : formatK(row.resident)}
                    </span>
                  </button>
                </div>
              );
            })}
            {rows.length === 0 && (
              <div className={styles.emptyFilter}>
                这一类在当前工具的全局层里没有资产。
                <button type="button" className={styles.link} onClick={() => setFilter("all")}>
                  清除筛选
                </button>
              </div>
            )}
            <p className={styles.foot}>
              {otherNote}「用到」依据本机会话记录近 30 天的触发情况，不上传。
            </p>
            {pickedRows.length > 0 && (
              <div className={styles.batch}>
                <span className={styles.batchText}>
                  <strong>已选 {pickedCount} 项</strong>
                  <span> · 每个项目常驻 </span>
                  <span className={styles.save}>−{formatK(pickedSave)}</span>
                  <span>（估算）</span>
                </span>
                <button type="button" className={styles.ghost} onClick={() => pick([])}>
                  取消选择
                </button>
                <button type="button" className={page.button} disabled title="M0 只读，在 M1 开放">
                  从全局移除…
                </button>
                <button
                  type="button"
                  className={page.buttonPrimary}
                  disabled
                  title="M0 只读，在 M1 开放"
                >
                  转成套装…
                </button>
              </div>
            )}
          </section>
          {inspectorOpen && current && (
            <RowInspector row={current} total={total} onClose={() => setInspectorOpen(false)} />
          )}
        </div>
      )}
    </div>
  );
}

function RowInspector({
  row,
  total,
  onClose,
}: {
  row: GlobalRow;
  total: number;
  onClose: () => void;
}) {
  const status = statusOf(row);
  const residentTone: Tone | undefined =
    row.resident !== null && row.resident >= 3000 ? "warn" : undefined;
  const fields = [
    { k: "位置", v: row.location, mono: true },
    {
      k: "来源",
      v: row.source ?? "未知",
      mono: row.source !== null,
      tone: row.source || row.kind !== "skill" ? undefined : ("unknown" as const),
    },
    ...(row.version ? [{ k: "版本", v: row.version, mono: true }] : []),
    {
      k: "常驻",
      v:
        row.resident === null
          ? "未估算：要启动服务才能读到工具定义"
          : `${formatK(row.resident)} / 每个项目（估算）`,
      tone: residentTone,
    },
    ...(row.usedBy !== null ? [{ k: "用到它", v: `${row.usedBy} / ${total} 个项目` }] : []),
    ...(row.command
      ? [{ k: "命令", v: row.command, mono: true, tone: "danger" as const }]
      : [
          {
            k: "含脚本",
            v: row.hasScripts ? "是" : "否",
            tone: row.hasScripts ? ("danger" as const) : undefined,
          },
        ]),
  ];
  const adviceTone =
    row.actions.length === 0 ? "keep" : row.status === "unknown-source" ? "adopt" : "act";
  return (
    <Inspector
      kind={`${KIND_NAME[row.kind]}${row.count > 1 ? ` · ${row.count} 项` : ""} · 全局层`}
      name={row.name}
      status={status}
      intro={row.summary}
      fields={fields}
      onClose={onClose}
    >
      <Section title="建议">
        <div className={styles.advice} data-tone={adviceTone}>
          <p>{row.advice}</p>
          {row.actions.length > 0 && (
            <div className={styles.adviceActions}>
              {row.actions.map((a, i) =>
                a === "去资产库收编" ? (
                  <Link key={a} to="/library" className={styles.adviceLink}>
                    {a}
                  </Link>
                ) : (
                  <button
                    key={a}
                    type="button"
                    className={i === 0 ? page.buttonPrimary : page.button}
                    disabled
                    title="M0 只读，在 M1 开放"
                  >
                    {a}
                  </button>
                ),
              )}
            </div>
          )}
          {row.actions.some((a) => a !== "去资产库收编") && (
            <span className={styles.cli}>M0 只读，这两个动作在 M1 开放。</span>
          )}
        </div>
        <span className={styles.cli}>$ fangcun why {row.members[0] ?? row.name} --global</span>
      </Section>
      {row.usage.length > 0 && (
        <Section title="用到它的项目 · 近 30 天">
          <ol className={page.box} style={{ margin: 0, padding: 0, listStyle: "none" }}>
            {row.usage.map((p) => (
              <li key={p.path} className={page.boxRow}>
                <span className={styles.projectName}>{p.name}</span>
                <span
                  className={styles.tag}
                  style={{
                    color: TONE_COLORS[p.mark === "used" ? "ok" : "neutral"][0],
                    opacity: p.mark === "unused" ? 0.8 : 1,
                  }}
                >
                  {p.mark === "used"
                    ? `用到 · ${p.triggers} 次`
                    : p.mark === "overridden"
                      ? "被项目层覆盖"
                      : p.mark === "conflict"
                        ? "同名冲突"
                        : "未用到"}
                </span>
              </li>
            ))}
          </ol>
        </Section>
      )}
      <Section title="分发到哪些工具">
        <ol className={page.box} style={{ margin: 0, padding: 0, listStyle: "none" }}>
          {row.distribution.map((d) => (
            <li key={d.tool} className={page.boxRow}>
              <span className={styles.distTool}>{d.toolName}</span>
              <span className={styles.distPath}>{d.path ?? ""}</span>
              <span
                className={styles.tag}
                style={{ color: TONE_COLORS[d.present ? "ok" : "neutral"][0] }}
              >
                {d.label}
              </span>
            </li>
          ))}
        </ol>
      </Section>
      <Section title="最近活动">
        <p className={styles.note}>
          还没有活动。M0 只记录体检。<Link to="/activity">在「活动」里查看全部</Link>
        </p>
      </Section>
    </Inspector>
  );
}
