import { type ReactNode, useEffect, useState } from "react";
import styles from "./components.module.css";
import c from "./controls.module.css";
import { Icon } from "./Icon";
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
  /** 名称左边的类型方块（资产库，6.4）。 */
  badge?: ReactNode;
  status?: { tone: Tone; label: string };
  intro?: ReactNode;
  /** 决定卡片（6.1）：排在「为什么」之后、字段表之前，只在要人决定时给。 */
  decision?: ReactNode;
  fields?: Field[];
  onClose: () => void;
  children?: ReactNode;
}

/**
 * 检查器统一结构（6.1）：类型小标签、名称、状态徽标、一句「为什么」、决定卡片、字段表、若干分区。
 * 右上角 × 只关面板。项目页、体检报告、全局、活动共用。
 */
export function Inspector({
  kind,
  name,
  badge,
  status,
  intro,
  decision,
  fields,
  onClose,
  children,
}: Props) {
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
        {badge ? (
          <div className={styles.badgeRow}>
            {badge}
            <div className={styles.badgeText}>
              <span className={styles.kind}>{kind}</span>
              <h2 className={styles.inspectorName}>{name}</h2>
            </div>
          </div>
        ) : (
          <>
            <span className={styles.kind}>{kind}</span>
            <h2 className={styles.inspectorName}>{name}</h2>
          </>
        )}
        {status && <StatusBadge tone={status.tone} label={status.label} />}
        {intro && <div className={styles.intro}>{intro}</div>}
      </div>
      {decision}
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

/** 检查器里的一个分区。标题统一 11px 灰色小字，点标题折叠（6.1）。 */
export function Section({
  title,
  aside,
  collapsible = true,
  children,
}: {
  title: string;
  aside?: ReactNode;
  collapsible?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(true);
  return (
    <section aria-label={title} className={styles.section}>
      <div className={styles.sectionHead}>
        {collapsible ? (
          <button
            type="button"
            className={styles.sectionToggle}
            aria-expanded={open}
            onClick={() => setOpen(!open)}
          >
            <svg
              width="9"
              height="9"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
              aria-hidden="true"
              className={styles.chev}
            >
              <path d="M6 3.5 10.5 8 6 12.5" />
            </svg>
            <h3>{title}</h3>
          </button>
        ) : (
          <h3>{title}</h3>
        )}
        {aside}
      </div>
      <div className={styles.sectionBody} data-open={open} aria-hidden={!open || undefined}>
        <div className={styles.sectionInner}>{children}</div>
      </div>
    </section>
  );
}

/** 命令行的对应写法，例如「$ fangcun why tdd」（6.1 检查器第 2 条）。支持点击一键复制。 */
export function CommandHint({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 1200);
    return () => clearTimeout(t);
  }, [copied]);

  return (
    <button
      type="button"
      className={styles.command}
      data-copied={copied}
      title="点击复制命令"
      aria-label={`复制命令：${command}`}
      onClick={() => {
        void navigator.clipboard?.writeText(command).catch(() => {});
        setCopied(true);
      }}
    >
      <span>{copied ? "✓ 已复制" : `$ ${command}`}</span>
      {!copied && <Icon name="copy" size={11} className={styles.commandIcon} />}
    </button>
  );
}

/** 还没接上内核用例的写入动作：按钮置灰，旁边说明。 */
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
        className={primary ? `${c.b} ${c.primary}` : c.b}
        disabled
        title={note ?? "这个动作还没有开放"}
      >
        {label}
      </button>
      <span className={styles.hint}>{note ?? "这个动作还没有开放"}</span>
    </div>
  );
}
