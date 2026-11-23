import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import c from "@/components/controls.module.css";
import { DecisionCard, Done, WriteList } from "@/components/DecisionCard";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import { CommandHint, Inspector } from "@/components/Inspector";
import { TONE_COLORS, type Tone } from "@/components/tone";
import { useListNav } from "@/components/useListNav";
import type { Finding } from "@/ipc/bindings/Finding";
import type { FindingCategory } from "@/ipc/bindings/FindingCategory";
import type { LocationTone } from "@/ipc/bindings/LocationTone";
import { kernel } from "@/ipc/commands";
import { useCheckup } from "@/ipc/queries";
import { PageToolbar } from "@/shell/PageToolbar";
import { applyInline, usePlan } from "@/state/plan";
import page from "../page.module.css";
import styles from "./CheckupPage.module.css";

const CATEGORY: Record<FindingCategory, { label: string; tone: Tone }> = {
  duplicate: { label: "重复", tone: "warn" },
  "global-tax": { label: "全局税", tone: "warn" },
  inconsistent: { label: "不一致", tone: "info" },
  detached: { label: "脱节", tone: "warn" },
  secret: { label: "密钥", tone: "warn" },
};

/** 四张量化卡片按这个顺序，各取这一类的第一条发现（6.2）。 */
const CARD_ORDER: FindingCategory[] = ["global-tax", "duplicate", "inconsistent", "detached"];
const CARD_LABEL: Partial<Record<FindingCategory, string>> = {
  "global-tax": "全局税",
  duplicate: "重复副本",
  inconsistent: "分发不齐",
  detached: "脱节",
};

const STATUS_LABEL: Record<Finding["status"], string> = {
  pending: "待处理",
  resolved: "已处理",
  "not-scanned": "尚未扫描",
};

/** 发现列表行首方块的浅底，只用「墨格」的语义浅色。 */
const WEAK: Record<Tone, string> = {
  neutral: "var(--fill-1)",
  muted: "var(--fill-1)",
  ok: "var(--ok-weak)",
  warn: "var(--warn-weak)",
  info: "var(--info-weak)",
  unknown: "var(--unk-weak)",
  danger: "var(--danger-weak)",
};

function locationColor(tone: LocationTone | null): string {
  return tone ? TONE_COLORS[tone][0] : "var(--tx-2)";
}

function toneOf(f: Finding): Tone {
  if (f.status === "resolved") return "ok";
  if (f.status === "not-scanned") return "muted";
  return CATEGORY[f.category].tone;
}

function when(iso: string): string {
  const d = new Date(iso);
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getMonth() + 1)}-${two(d.getDate())} ${two(d.getHours())}:${two(d.getMinutes())}`;
}

function undoRecord(record: string) {
  void usePlan.getState().preview({
    load: () => kernel.planRollbackRecord(record),
    done: (_, plan) => `已撤销：${plan.title}`,
    undoable: false,
  });
}

/**
 * 体检报告（6.2，方寸 2）：结论在前，三个数字和处理进度；四张量化卡片；发现列表加检查器。
 * 检查器里每条发现一张决定卡片，处理完显示「已处理 · 已留快照」和撤销，1.5 秒后跳到下一条。
 */
export function CheckupPage() {
  const report = useCheckup();
  const navigate = useNavigate();
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  // 刚处理完、还在等 1.5 秒跳下一条的那一条（D-25）。中途撤销或点了别的就不跳。
  const [justDone, setJustDone] = useState<string | null>(null);

  const data = report.data;
  const findings = data?.findings ?? [];
  const current = findings.find((f) => f.id === selected) ?? findings[0];
  const handled = data?.progress.handled ?? 0;
  const total = data?.progress.total ?? 0;
  const left = total - handled;
  const readyLeft = findings.filter((f) => f.status === "pending" && f.ready).length;
  const cards = CARD_ORDER.map((cat) => findings.find((f) => f.category === cat)).filter(
    (f): f is Finding => f !== undefined && f.metric !== null,
  );

  useEffect(() => {
    if (!justDone || current?.id !== justDone) return;
    const timer = window.setTimeout(() => {
      const next = findings.find((f) => f.status === "pending" && f.id !== justDone);
      if (next) setSelected(next.id);
      setJustDone(null);
    }, 1500);
    return () => window.clearTimeout(timer);
  }, [justDone, current, findings]);

  const pick = (id: string) => {
    setSelected(id);
    setJustDone(null);
    setInspectorOpen(true);
  };
  useListNav(
    findings.map((f) => f.id),
    current?.id,
    pick,
  );

  return (
    <div className={page.page}>
      <PageToolbar
        title="体检报告"
        subtitle={
          report.isLoading ? "正在扫描…" : data ? `体检 · ${when(data.scannedAt)}` : undefined
        }
      >
        <button type="button" className={c.b} onClick={() => navigate(-1)}>
          稍后处理
        </button>
        <button
          type="button"
          className={`${c.b} ${c.primary}`}
          onClick={() => navigate("/projects")}
        >
          进入项目
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

      {report.isError && <p className={page.error}>{report.error.message}</p>}

      {data && (
        <div className={page.split}>
          <section className={`${page.list} ${styles.main}`}>
            <section aria-label="结论" className={styles.hero}>
              <div className={styles.heroText}>
                <span className={styles.cap}>
                  <span aria-hidden="true" className={styles.dot} data-tone="ok" />
                  只读扫描完成 · 没有改动任何文件
                </span>
                <h2 className={styles.heroTitle}>
                  {findings.length === 0
                    ? "没有发现。"
                    : left === 0 && total > 0
                      ? `${total} 件都处理完了。下一步：进入项目看看生效上下文。`
                      : readyLeft > 0
                        ? `发现 ${findings.length} 件事：${readyLeft} 件可直接收录，其余可直达处理。`
                        : `发现 ${findings.length} 件事，可直达对应页面处理。`}
                </h2>
                <p className={styles.numbers}>
                  <span>
                    <span className={styles.num}>{data.toolsScanned}</span> 个工具
                  </span>
                  <span>
                    <span className={styles.num}>{data.projectDirsScanned}</span> 个项目目录
                  </span>
                  <span>
                    <span className={styles.num}>{data.assetsFound}</span> 项资产
                  </span>
                  <span className={styles.safe}>
                    先预览 · 写前留快照
                    <InfoTip label="写入说明">
                      每一项都先列出会写入的文件，点了才写；写前留快照，可撤销。
                    </InfoTip>
                  </span>
                </p>
              </div>
              {total > 0 && (
                <div className={styles.progress}>
                  <div className={styles.progressNums}>
                    <span className={styles.progressBig}>{handled}</span>
                    <span className={styles.progressTotal}>/ {total}</span>
                    <span className={styles.progressLabel}>已处理</span>
                  </div>
                  <div
                    role="img"
                    aria-label={`已处理 ${handled} / ${total} 项发现`}
                    className={styles.segments}
                  >
                    {findings
                      .filter((f) => f.status !== "not-scanned")
                      .map((f) => (
                        <span key={f.id} data-done={f.status === "resolved"} />
                      ))}
                  </div>
                </div>
              )}
            </section>

            {cards.length > 0 && (
              <section aria-label="量化" className={styles.cards}>
                {cards.map((f) => {
                  const done = f.status === "resolved";
                  return (
                    <button
                      key={f.id}
                      type="button"
                      className={styles.card}
                      data-on={current?.id === f.id}
                      onClick={() => pick(f.id)}
                    >
                      <span className={styles.cardLabel}>
                        <span
                          aria-hidden="true"
                          className={styles.dot}
                          style={{ background: TONE_COLORS[toneOf(f)][1] }}
                        />
                        {CARD_LABEL[f.category]}
                      </span>
                      <span className={styles.cardValue} data-done={done}>
                        {done ? "已处理" : f.metric}
                      </span>
                      <span className={styles.cardNote}>
                        {done ? "已留快照，可撤销" : f.metricNote}
                      </span>
                    </button>
                  );
                })}
              </section>
            )}

            <div className={styles.listHead}>
              <span className={styles.cap}>发现 · 按影响排序</span>
            </div>
            {findings.length === 0 && (
              <p className={page.note}>
                没有发现。各工具的全局层、{data.projectDirsScanned} 个项目目录都看过了。
              </p>
            )}
            <div className={styles.findings}>
              {findings.map((f) => {
                const tone = toneOf(f);
                const resolved = f.status === "resolved";
                return (
                  <button
                    key={f.id}
                    type="button"
                    data-nav-id={f.id}
                    className={styles.finding}
                    aria-pressed={f.id === current?.id}
                    onClick={() => pick(f.id)}
                  >
                    <span
                      aria-hidden="true"
                      className={styles.tile}
                      style={{ background: WEAK[tone] }}
                    >
                      <span className={styles.dot} style={{ background: TONE_COLORS[tone][1] }} />
                    </span>
                    <span className={styles.findingText}>
                      <span
                        className={
                          resolved ? `${styles.findingTitle} ${page.struck}` : styles.findingTitle
                        }
                      >
                        {f.title}
                      </span>
                      <span className={styles.findingSub}>
                        {CATEGORY[f.category].label} · {f.brief || f.action}
                      </span>
                    </span>
                    {(f.impact ?? f.metric) && f.status !== "resolved" && (
                      <span className={styles.impact}>
                        {f.impact ?? `${f.metric} ${f.metricNote ?? ""}`}
                      </span>
                    )}
                    <span className={styles.pill} style={{ color: TONE_COLORS[tone][0] }}>
                      <span
                        aria-hidden="true"
                        className={styles.dot}
                        style={{ background: TONE_COLORS[tone][1] }}
                      />
                      {STATUS_LABEL[f.status]}
                    </span>
                  </button>
                );
              })}
            </div>
            <Link
              to="/activity"
              className={styles.activityLink}
              title="处理记录都留在「活动」里，随时可以回滚"
            >
              记录在「活动」· 可回滚
            </Link>
          </section>

          {inspectorOpen && current && (
            <Inspector
              key={current.id}
              kind={`体检发现 · ${CATEGORY[current.category].label}`}
              name={current.title}
              status={{ tone: toneOf(current), label: STATUS_LABEL[current.status] }}
              intro={current.intro}
              decision={
                current.status === "resolved" ? (
                  <Done
                    title="已处理 · 已留快照"
                    desc={current.foot}
                    onUndo={
                      current.record
                        ? () => {
                            setJustDone(null);
                            undoRecord(current.record ?? "");
                          }
                        : undefined
                    }
                  />
                ) : (
                  <FindingCard finding={current} onDone={() => setJustDone(current.id)} />
                )
              }
              onClose={() => setInspectorOpen(false)}
            />
          )}
        </div>
      )}
    </div>
  );
}

function routeForFinding(f: Finding): { to: string; label: string } | null {
  switch (f.category) {
    case "global-tax":
      return { to: "/global", label: "去全局页清理" };
    case "inconsistent":
      return { to: "/global", label: "去全局页查看" };
    case "duplicate":
      return { to: "/library", label: "去资产库查看" };
    case "detached":
      return { to: "/", label: "去项目查看" };
    case "secret":
      return null;
  }
}

function cliForFinding(f: Finding): string {
  switch (f.category) {
    case "global-tax":
    case "inconsistent":
      return "fangcun status --global";
    case "duplicate":
      return f.id === "repos"
        ? "fangcun library sources"
        : `fangcun adopt ${f.id.replace(/^dup:/, "")}`;
    case "detached":
      return "fangcun lock --refresh";
    case "secret":
      return "fangcun secret scan";
  }
}

/** 一条发现的决定卡片：涉及的位置、会写入哪些文件、一键动作或直达导航。 */
function FindingCard({ finding: f, onDone }: { finding: Finding; onDone: () => void }) {
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const writes = f.status === "pending" && f.ready;
  const route = routeForFinding(f);
  const cli = cliForFinding(f);
  const plan = useQuery({
    queryKey: ["finding-plan", f.id],
    queryFn: () => kernel.planFinding(f.id),
    enabled: writes,
  });
  const apply = async () => {
    if (!plan.data || plan.data.blocked) return;
    setBusy(true);
    setError(null);
    try {
      await applyInline(
        plan.data,
        () => `${f.action} · 已留快照`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
      onDone();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };
  return (
    <DecisionCard
      title="涉及的位置"
      metric={f.locations.length}
      foot={
        <>
          <span className={styles.footNote}>{writes ? "写前留快照 · 可撤销" : f.foot}</span>
          {writes ? (
            <button
              type="button"
              className={`${c.b} ${c.primary}`}
              disabled={!plan.data || plan.data.blocked || busy}
              title={f.foot}
              onClick={() => void apply()}
            >
              {busy ? "正在写入…" : f.action}
            </button>
          ) : (
            route && (
              <Link to={route.to} className={`${c.b} ${c.primary} ${styles.routeLink}`}>
                <span>{route.label}</span>
                <Icon name="arrowRight" size={12} />
              </Link>
            )
          )}
        </>
      }
    >
      <ul className={styles.locations}>
        {f.locations.map((l) => (
          <li key={`${l.path}:${l.detail}`}>
            <span className={styles.locPath}>{l.path}</span>
            <span className={styles.locMeta}>
              <span className={styles.locDetail}>{l.detail}</span>
              <span style={{ color: locationColor(l.tone), fontWeight: l.tone ? 500 : 400 }}>
                {l.note}
              </span>
            </span>
          </li>
        ))}
      </ul>
      {writes ? (
        <WriteList
          changes={plan.data?.changes}
          loading={plan.isLoading}
          error={error ?? (plan.isError ? plan.error.message : null)}
          blockedNote={plan.data?.blocked ? plan.data.notes.map((n) => n.text).join(" ") : null}
        />
      ) : (
        <div className={styles.noWritesBox}>
          <p className={styles.noWrites}>
            {f.status === "not-scanned"
              ? "只读扫描，不写任何文件。点击下方可复制命令在终端运行："
              : route
                ? "前往对应页面直接处理，或点击复制终端命令："
                : "可点击复制下方终端命令查看详情："}
          </p>
          <CommandHint command={cli} />
        </div>
      )}
    </DecisionCard>
  );
}
