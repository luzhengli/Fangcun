import { useState } from "react";
import { Link } from "react-router";
import { Inspector, Section } from "@/components/Inspector";
import { StatusDot } from "@/components/StatusDot";
import type { Tone } from "@/components/tone";
import type { FileOp } from "@/ipc/bindings/FileOp";
import type { Record as ActivityRecord } from "@/ipc/bindings/Record";
import type { RecordKind } from "@/ipc/bindings/RecordKind";
import type { RecordStatus } from "@/ipc/bindings/RecordStatus";
import type { Trigger } from "@/ipc/bindings/Trigger";
import { useActivity, useProjects } from "@/ipc/queries";
import page from "../page.module.css";
import styles from "./ActivityPage.module.css";

/** 筛选芯片，顺序固定（6.7）。「可更新」没有芯片，算在「升级」里。 */
const CHIPS: [RecordKind | null, string][] = [
  [null, "全部"],
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

/** 活动：时间线与回滚（6.7）。M0 只有体检与「已记录」两类记录。 */
export function ActivityPage() {
  const [kind, setKind] = useState<RecordKind | null>(null);
  const [project, setProject] = useState<string>("all");
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);
  const projects = useProjects();
  const filter = {
    kind,
    project: project === "all" || project === "global" ? null : project,
  };
  const activity = useActivity(filter);

  const records = (activity.data ?? []).filter((r) => project !== "global" || r.project === null);
  const pending = records.filter((r) => r.status === "pending");
  const timeline = records.filter((r) => r.status !== "pending");
  const days: { label: string; items: ActivityRecord[] }[] = [];
  for (const r of timeline) {
    const label = dayLabel(localParts(r.at.toString()).date);
    const last = days[days.length - 1];
    if (last && last.label === label) last.items.push(r);
    else days.push({ label, items: [r] });
  }
  const current = records.find((r) => r.id === selected) ?? records[0];

  const row = (r: ActivityRecord, timeText: string) => {
    const status = statusOf(r);
    return (
      <button
        key={r.id}
        type="button"
        className={styles.row}
        aria-pressed={r.id === current?.id}
        onClick={() => {
          setSelected(r.id);
          setInspectorOpen(true);
        }}
      >
        <span className={styles.colTime}>{timeText}</span>
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
      <header className={page.header}>
        <h1 className={page.title}>活动</h1>
        <span className={styles.subtitle}>所有项目的落盘记录 · 每次写入都留快照，可一键回滚</span>
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
      </header>
      <div className={page.split}>
        <section aria-label="活动时间线" className={page.list}>
          <div className={styles.toolbar}>
            <fieldset aria-label="按类型筛选" className={page.chips}>
              {CHIPS.map(([k, label]) => (
                <button
                  key={label}
                  type="button"
                  className={page.chip}
                  aria-pressed={kind === k}
                  onClick={() => setKind(k)}
                >
                  {label}
                </button>
              ))}
            </fieldset>
          </div>
          <div className={styles.colHead}>
            <span className={styles.colTime}>时间</span>
            <span className={styles.colKind}>类型</span>
            <span className={styles.colEvent}>事件 · 项目</span>
            <span className={styles.colSnap}>快照</span>
            <span className={styles.colStatus}>状态</span>
          </div>
          {activity.isError && <p className={page.error}>{activity.error.message}</p>}
          {pending.length > 0 && (
            <div>
              <h3 className={styles.pendingHead}>
                <span aria-hidden="true" className={styles.pendingDot} />
                待处理 · {pending.length}
              </h3>
              {pending.map((r) => row(r, localParts(r.at.toString()).time))}
            </div>
          )}
          {days.map((g) => (
            <div key={g.label}>
              <h3 className={styles.dayHead}>{g.label}</h3>
              {g.items.map((r) => row(r, localParts(r.at.toString()).time))}
            </div>
          ))}
          {activity.data && records.length === 0 && (
            <div className={styles.emptyFilter}>
              没有符合筛选的活动。
              <button
                type="button"
                className={styles.link}
                onClick={() => {
                  setKind(null);
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
          <RecordInspector record={current} onClose={() => setInspectorOpen(false)} />
        )}
      </div>
    </div>
  );
}

function RecordInspector({ record, onClose }: { record: ActivityRecord; onClose: () => void }) {
  const parts = localParts(record.at.toString());
  const hasSnapshot = record.snapshot !== null;
  return (
    <Inspector
      kind={`${KIND_LABEL[record.kind]}${hasSnapshot ? ` · 快照 #${record.snapshot}` : ""}`}
      name={record.title}
      status={statusOf(record)}
      intro={record.summary}
      fields={[
        { k: "项目", v: record.projectLabel },
        { k: "时间", v: `${parts.day} ${parts.time}`, mono: true },
        { k: "触发", v: TRIGGER_LABEL[record.trigger] },
        { k: "快照", v: hasSnapshot ? `#${record.snapshot}` : "无，未写入文件" },
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
      {record.kind === "checkup" && (
        <div>
          <Link
            to="/checkup"
            className={page.button}
            style={{ display: "inline-flex", alignItems: "center", textDecoration: "none" }}
          >
            打开体检报告
          </Link>
        </div>
      )}
    </Inspector>
  );
}
