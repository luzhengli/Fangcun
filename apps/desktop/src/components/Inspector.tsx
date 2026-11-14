import type { ReactNode } from "react";
import styles from "./components.module.css";
import { StatusBadge } from "./StatusDot";
import { TONE_COLORS, type Tone } from "./tone";

export interface Field {
  k: string;
  v: ReactNode;
  /** 路径、哈希、命令用等宽字体。 */
  mono?: boolean;
  tone?: Tone;
}

interface Props {
  /** 类型小标签，例如「Skill」「同步 · 快照 #127」。 */
  kind: string;
  name: string;
  status?: { tone: Tone; label: string };
  intro?: ReactNode;
  fields?: Field[];
  onClose: () => void;
  children?: ReactNode;
}

/**
 * 检查器统一结构（6.1）：类型小标签、名称、状态徽标、一句说明、字段表、若干分区、操作。
 * 右上角 × 关闭。项目页、体检报告、全局、活动共用。
 */
export function Inspector({ kind, name, status, intro, fields, onClose, children }: Props) {
  return (
    <aside aria-label="检查器" className={styles.inspector}>
      <div className={styles.inspectorHead}>
        <button
          type="button"
          className={styles.close}
          onClick={onClose}
          aria-label="关闭检查器"
          title="关闭检查器"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            aria-hidden="true"
          >
            <path d="M4 4l8 8M12 4l-8 8" />
          </svg>
        </button>
        <span className={styles.kind}>{kind}</span>
        <h2 className={styles.inspectorName}>{name}</h2>
        {status && <StatusBadge tone={status.tone} label={status.label} />}
        {intro && <p className={styles.intro}>{intro}</p>}
      </div>
      {fields && fields.length > 0 && (
        <dl className={styles.fields}>
          {fields.map((f) => (
            <FieldRow key={f.k} field={f} />
          ))}
        </dl>
      )}
      {children}
    </aside>
  );
}

function FieldRow({ field }: { field: Field }) {
  const color = field.tone ? TONE_COLORS[field.tone][0] : undefined;
  return (
    <>
      <dt>{field.k}</dt>
      <dd className={field.mono ? styles.mono : undefined} style={color ? { color } : undefined}>
        {field.v}
      </dd>
    </>
  );
}

/** 检查器里的一个分区。标题统一 11px 灰色小字。 */
export function Section({
  title,
  aside,
  children,
}: {
  title: string;
  aside?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className={styles.section}>
      <div className={styles.sectionHead}>
        <h3>{title}</h3>
        {aside}
      </div>
      {children}
    </section>
  );
}

/** 「命令行同款」那一行。 */
export function CommandHint({ command }: { command: string }) {
  return (
    <span className={styles.hint}>
      命令行同款：<span className={styles.mono}>{command}</span>
    </span>
  );
}

/** M0 只读：写入类的动作按钮置灰，旁边说明。 */
export function ReadOnlyAction({
  label,
  primary = true,
  note,
}: {
  label: string;
  primary?: boolean;
  note?: string;
}) {
  return (
    <div className={styles.actionRow}>
      <button
        type="button"
        className={primary ? styles.primary : styles.secondary}
        disabled
        title="M0 只读，这个动作在 M1 开放"
      >
        {label}
      </button>
      <span className={styles.hint}>{note ?? "M0 只读，这个动作在 M1 开放"}</span>
    </div>
  );
}
