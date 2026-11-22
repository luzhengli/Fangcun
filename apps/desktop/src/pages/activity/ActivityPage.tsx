import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import c from "@/components/controls.module.css";
import { DecisionCard, WriteList } from "@/components/DecisionCard";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import { Inspector, Section } from "@/components/Inspector";
import { StatusDot } from "@/components/StatusDot";
import type { Tone } from "@/components/tone";
import type { FileOp } from "@/ipc/bindings/FileOp";
import type { Record as ActivityRecord } from "@/ipc/bindings/Record";
import type { RecordKind } from "@/ipc/bindings/RecordKind";
import type { RecordStatus } from "@/ipc/bindings/RecordStatus";
import type { Trigger } from "@/ipc/bindings/Trigger";
import { kernel } from "@/ipc/commands";
import { useActivity, useOverview, useProjects } from "@/ipc/queries";
import { projectPath } from "@/shell/nav";
import { PageToolbar } from "@/shell/PageToolbar";
import { applyInline, usePlan } from "@/state/plan";
import page from "../page.module.css";
import styles from "./ActivityPage.module.css";

/** 类型筛选，顺序固定（6.7）。「可更新」没有单独一项，算在「升级」里。 */
const CHIPS: [RecordKind, string][] = [
  ["sync", "同步"],
  ["drift", "漂移"],
  ["upgrade", "升级"],
  ["adopt", "收编"],
  ["rollback", "回滚"],
  ["checkup", "体检"],
  ["verify", "校验"],
  ["secret", "密钥"],
];

const KIND_LABEL: Record<RecordKind, string> = {
  sync: "同步",
  drift: "漂移",
  upgrade: "升级",
  updatable: "可更新",
  adopt: "收编",
  rollback: "回滚",
  checkup: "体检",
  verify: "校验",
  secret: "密钥",
};

const TRIGGER_LABEL: Record<Trigger, string> = {
  desktop: "桌面应用",
  cli: "命令行",
  watcher: "文件监听",
  "upstream-check": "上游检查",
  "first-launch": "首次启动",
};

function statusOf(record: ActivityRecord): { tone: Tone; label: string } {
  const map: Record<RecordStatus, { tone: Tone; label: string }> = {
    applied: { tone: record.kind === "rollback" ? "neutral" : "ok", label: "已应用" },
    pending: { tone: record.kind === "updatable" ? "info" : "warn", label: "待处理" },
    failed: { tone: "danger", label: "失败" },
    "rolled-back": { tone: "neutral", label: "已回滚" },
    recorded: { tone: "neutral", label: "已记录" },
    "read-only": { tone: "neutral", label: "只读" },
  };
  return map[record.status];
}

const OP_MARK: Record<FileOp, string> = { add: "+", modify: "~", remove: "−", read: "=" };
const WEEKDAYS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function pad(n: number) {
  return String(n).padStart(2, "0");
}

/** 本地时间的「11-13」与「10:42」。 */
function localParts(iso: string) {
  const d = new Date(iso);
  return {
    day: `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`,
    time: `${pad(d.getHours())}:${pad(d.getMinutes())}`,
    date: d,
  };
}

/** 分组标题：「今天 · 11-13」「昨天 · 11-12」「11-11 周二」。 */
function dayLabel(date: Date, now = new Date()) {
  const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((start(now) - start(date)) / 86_400_000);
  const day = `${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  if (diff === 0) return `今天 · ${day}`;
  if (diff === 1) return `昨天 · ${day}`;
  return `${day} ${WEEKDAYS[date.getDay()]}`;
}

type Filter = "all" | "pending" | RecordKind;

/**
 * 活动：时间线与回滚（6.7，方寸 2）。筛选条在列表最上方吸顶，切换时只有下面的结果变；
 * 要你决定的事置顶在「待处理」一组，不进时间线；检查器里是「回滚到这一步之前」的决定卡片。
 */
export function ActivityPage() {
  const [filter, setFilter] = useState<Filter>("all");
  const [project, setProject] = useState<string>("all");
  // 项目页检查器「最近活动」点一条，带着 ?record= 过来，直接选中那条（6.3 检查器第 6 条）。
  const [params] = useSearchParams();
  const [selected, setSelected] = useState<string | null>(params.get("record"));
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const [lastCheck, setLastCheck] = useState<string | null>(null);
  const client = useQueryClient();
  const projects = useProjects();
  const overview = useOverview();
  const kind = filter === "all" || filter === "pending" ? null : filter;
  const query = {
    kind,
    project: project === "all" || project === "global" ? null : project,
  };
  const activity = useActivity(query);
  // 「检查更新」（D-32）：手动查一次所有已声明的上游来源，查到的新版本进「待处理」。
  const check = useMutation({
    mutationFn: () => kernel.checkUpstream(),
    onSuccess: async (report) => {
      setLastCheck(localParts(new Date().toISOString()).time);
      await client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" });
      usePlan.getState().showToast({
        text:
          report.errors.length > 0
            ? `已检查上游 · ${report.items.length} 项可更新，${report.errors.length} 个来源没查到`
            : `已检查上游 · ${report.items.length > 0 ? `${report.items.length} 项可更新` : "没有新的可更新"}`,
        record: null,
      });
    },
  });

  const records = (activity.data ?? []).filter((r) => project !== "global" || r.project === null);
  const pending = records.filter((r) => r.status === "pending");
  const timeline = filter === "pending" ? [] : records.filter((r) => r.status !== "pending");
  const pendingAll = overview.data?.pending ?? pending.length;
  const hiddenPending = kind ? Math.max(0, pendingAll - pending.length) : 0;
  const days: { label: string; items: ActivityRecord[] }[] = [];
  for (const r of timeline) {
    const label = dayLabel(localParts(r.at.toString()).date);
    const last = days[days.length - 1];
    if (last && last.label === label) last.items.push(r);
    else days.push({ label, items: [r] });
  }
  const shown = [...pending, ...timeline];
  // 没选时看时间线最新一条（原型 Activity），没有时间线再看待处理。
  const current = shown.find((r) => r.id === selected) ?? timeline[0] ?? pending[0];
  const pick = (id: string) => {
    setSelected(id);
    setInspectorOpen(true);
  };

  const row = (r: ActivityRecord) => {
    const status = statusOf(r);
    return (
      <button
        key={r.id}
        type="button"
        className={styles.row}
        aria-pressed={r.id === current?.id}
        onClick={() => pick(r.id)}
      >
        <span className={styles.colTime}>{localParts(r.at.toString()).time}</span>
        <span className={styles.colKind}>
          <span className={styles.kindTag}>{KIND_LABEL[r.kind]}</span>
        </span>
        <span className={styles.colEvent}>
          <span className={`${styles.eventTitle} ${r.status === "rolled-back" ? page.struck : ""}`}>
            {r.title}
          </span>
          <span className={styles.meta}>
            {r.projectLabel} · {TRIGGER_LABEL[r.trigger]}
          </span>
        </span>
        <span className={styles.colSnap}>{r.snapshot === null ? "—" : `#${r.snapshot}`}</span>
        <span className={styles.colStatus}>
          <StatusDot tone={status.tone} label={status.label} />
        </span>
      </button>
    );
  };

  return (
    <div className={page.page}>
      <PageToolbar title="活动" subtitle="所有项目的落盘记录 · 每次写入都留快照，可一键回滚">
        <label className={styles.projectLabel}>
          项目
          <select
            value={project}
            onChange={(e) => setProject(e.target.value)}
            className={styles.select}
            data-active={project !== "all"}
          >
            <option value="all">全部项目（{projects.data?.length ?? 0}）</option>
            {(projects.data ?? []).map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
            <option value="global">全局层</option>
          </select>
        </label>
        <button
          type="button"
          className={c.b}
          disabled={check.isPending}
          title="检查所有已声明的上游来源 · fangcun update --dry-run"
          onClick={() => check.mutate()}
        >
          {check.isPending ? "正在检查…" : "检查更新"}
        </button>
        <span className={styles.lastCheck}>{lastCheck ? `上次 ${lastCheck}` : "启动时查过"}</span>
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
      <div className={page.split}>
        <section aria-label="活动时间线" className={`${page.list} ${styles.list}`}>
          <div className={styles.filterBar}>
            <fieldset aria-label="筛选" className={`${c.seg} ${styles.filters}`}>
              <button
                type="button"
                aria-pressed={filter === "all"}
                onClick={() => setFilter("all")}
              >
                全部
              </button>
              <button
                type="button"
                aria-pressed={filter === "pending"}
                onClick={() => setFilter("pending")}
              >
                待处理{" "}
                <span className={styles.pendingCount} data-zero={pendingAll === 0}>
                  {pendingAll}
                </span>
              </button>
              <span aria-hidden="true" className={styles.divider} />
              {CHIPS.map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  aria-pressed={filter === k}
                  onClick={() => setFilter(k)}
                >
                  {label}
                </button>
              ))}
            </fieldset>
            <span className={styles.count}>{shown.length} 条</span>
          </div>
          {activity.isError && <p className={page.error}>{activity.error.message}</p>}

          {(filter === "all" ||
            filter === "pending" ||
            pending.length > 0 ||
            hiddenPending > 0) && (
            <section aria-label="待处理" className={styles.pinned}>
              <h3 className={styles.pinnedHead}>
                待处理 · 置顶 <span className={styles.pendingCount}>{pending.length}</span>
                <InfoTip label="待处理说明">
                  要你决定的事置顶在这里，不进下面的时间线；处理后才作为一条记录落进时间线。也显示在菜单栏图标的小圆点上。
                </InfoTip>
              </h3>
              {pending.map((r) => (
                <PendingRow key={r.id} record={r} on={r.id === current?.id} onPick={pick} />
              ))}
              {hiddenPending > 0 && (
                <button
                  type="button"
                  className={styles.hiddenRow}
                  onClick={() => setFilter("pending")}
                >
                  {hiddenPending} 条待处理不在「{KIND_LABEL[kind as RecordKind]}」里 · 查看
                </button>
              )}
              {pending.length === 0 && hiddenPending === 0 && (
                <p className={styles.pinnedEmpty}>
                  没有待处理的事。漂移、上游更新出现时会置顶在这里。
                </p>
              )}
            </section>
          )}

          {days.map((g) => (
            <div key={g.label}>
              <h3 className={styles.dayHead}>
                {g.label} <span className={styles.dayCount}>{g.items.length}</span>
              </h3>
              {g.items.map((r) => row(r))}
            </div>
          ))}
          {activity.data && shown.length === 0 && filter !== "pending" && (
            <div className={styles.emptyFilter}>
              这个筛选下没有记录。换个类型或项目看看。
              <button
                type="button"
                className={styles.link}
                onClick={() => {
                  setFilter("all");
                  setProject("all");
                }}
              >
                清除筛选
              </button>
            </div>
          )}
          <p className={styles.foot}>
            更早的记录保留在 ~/.fangcun/journal，命令行可用{" "}
            <span className={styles.mono}>fangcun log</span> 查看。
          </p>
        </section>
        {inspectorOpen && current && (
          <RecordInspector
            key={current.id}
            record={current}
            onClose={() => setInspectorOpen(false)}
          />
        )}
      </div>
    </div>
  );
}

/** 待处理一行：行内直接给「去处理」「查看差异」，跳到项目页并选中那一项。 */
function PendingRow({
  record: r,
  on,
  onPick,
}: {
  record: ActivityRecord;
  on: boolean;
  onPick: (id: string) => void;
}) {
  const target = usePendingTarget(r);
  const tone = r.kind === "updatable" ? "info" : "warn";
  return (
    <div className={styles.pendingRow} data-on={on}>
      <button type="button" className={styles.pendingMain} onClick={() => onPick(r.id)}>
        <span aria-hidden="true" className={styles.pendingTile} data-tone={tone}>
          <span className={styles.pendingDot} data-tone={tone} />
        </span>
        <span className={styles.colEvent}>
          <span className={styles.eventTitle}>{r.title}</span>
          <span className={styles.meta}>
            {r.projectLabel} · {localParts(r.at.toString()).time} · {TRIGGER_LABEL[r.trigger]}
          </span>
        </span>
      </button>
      {target && (
        <Link to={target.to} className={`${c.b} ${c.primary} ${c.small}`}>
          {target.label}
        </Link>
      )}
    </div>
  );
}

/** 待处理记录跳到哪里：项目页并选中那一项（深链 ?asset=）。涉及几个项目的去第一个。 */
function usePendingTarget(record: ActivityRecord): { to: string; label: string } | null {
  const projects = useProjects();
  if (record.status !== "pending" || !record.subject) return null;
  const first =
    record.project ??
    projects.data?.find((p) => p.registered && record.projectLabel.startsWith(p.name))?.id;
  if (!first) return null;
  return {
    to: `${projectPath(first)}?asset=${encodeURIComponent(record.subject.asset)}`,
    label: record.kind === "drift" ? "去处理" : "查看差异",
  };
}

function RecordInspector({ record, onClose }: { record: ActivityRecord; onClose: () => void }) {
  const parts = localParts(record.at.toString());
  const hasSnapshot = record.snapshot !== null;
  const target = usePendingTarget(record);
  return (
    <Inspector
      kind={`${KIND_LABEL[record.kind]}${hasSnapshot ? ` · 快照 #${record.snapshot}` : ""}`}
      name={record.title}
      status={statusOf(record)}
      intro={record.summary}
      decision={
        <>
          <RollbackCard record={record} />
          {target && (
            <DecisionCard
              title={record.kind === "drift" ? "要你决定怎么处理" : "上游有新版本"}
              foot={
                <>
                  <span className={styles.footNote}>在项目页处理，先出变更计划</span>
                  <Link to={target.to} className={`${c.b} ${c.primary}`}>
                    {target.label}
                  </Link>
                </>
              }
            />
          )}
          {record.kind === "checkup" && (
            <DecisionCard
              title="只读扫描"
              foot={
                <>
                  <span className={styles.footNote}>没有写入任何文件</span>
                  <Link to="/checkup" className={c.b}>
                    打开体检报告
                  </Link>
                </>
              }
            />
          )}
        </>
      }
      fields={[
        { k: "项目", v: record.projectLabel },
        { k: "时间", v: `${parts.day} ${parts.time}`, mono: true },
        { k: "触发", v: TRIGGER_LABEL[record.trigger] },
        {
          k: "快照",
          v:
            record.kind === "secret"
              ? "无，钥匙串条目不留快照"
              : hasSnapshot
                ? `#${record.snapshot}`
                : "无，没有写入文件",
        },
      ]}
      onClose={onClose}
    >
      <Section title={hasSnapshot ? "写入了哪些文件" : "涉及的文件"}>
        <div className={page.box}>
          {record.files.map((f) => (
            <div key={`${f.op}:${f.path}`} className={styles.file}>
              <span className={styles.op} data-op={f.op}>
                {OP_MARK[f.op]}
              </span>
              <span className={styles.fileText}>
                <span className={styles.filePath}>{f.path}</span>
                {f.note && <span className={styles.fileNote}>{f.note}</span>}
              </span>
            </div>
          ))}
        </div>
      </Section>
      {record.command && (
        <Section title="命令行">
          <pre className={styles.cli}>
            $ {record.command}
            {record.output ? `\n${record.output}` : ""}
          </pre>
        </Section>
      )}
    </Inspector>
  );
}

/**
 * 「回滚到这一步之前」（6.7）：有快照、已应用的写入才有。卡片里列出逆向的变更计划；
 * 有文件被后来的变更改过时内核把计划标成不能应用（回滚规则第 6 条），提示先回滚后面那条，按钮置灰。
 * 已回滚的写一句说明；密钥记录不能回滚。
 */
function RollbackCard({ record }: { record: ActivityRecord }) {
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const can =
    record.kind !== "secret" && record.status === "applied" && record.snapshots.length > 0;
  const plan = useQuery({
    queryKey: ["rollback-record-plan", record.id],
    queryFn: () => kernel.planRollbackRecord(record.id),
    enabled: can,
  });
  if (record.kind === "secret")
    return (
      <p className={styles.rollbackNote}>
        密钥记录不能回滚，钥匙串条目保持现状。要撤销，用「替换」或「删除」再记一条。
      </p>
    );
  if (record.status === "rolled-back")
    return <p className={styles.rollbackNote}>这次变更已被回滚，磁盘状态与上一快照一致。</p>;
  if (!can) return null;
  const blocked = plan.data?.blocked ?? false;
  const rollback = async () => {
    if (!plan.data || blocked) return;
    setBusy(true);
    setError(null);
    try {
      await applyInline(
        plan.data,
        () => `${plan.data?.title ?? "已回滚"} · 磁盘状态逐字节一致`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <DecisionCard
      title="回滚到这一步之前"
      metric={plan.data ? `${plan.data.changes.length} 处` : null}
      foot={
        <>
          <span className={styles.footNote}>{blocked ? "先回滚后面那条" : "回滚后可再撤销"}</span>
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            disabled={!plan.data || blocked || busy}
            title={blocked ? "先回滚后面那条" : undefined}
            onClick={() => void rollback()}
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
        blockedNote={blocked ? plan.data?.notes.map((n) => n.text).join(" ") : null}
      />
    </DecisionCard>
  );
}
