import type { ReactNode } from "react";
import type { Change } from "@/ipc/bindings/Change";
import c from "./controls.module.css";
import styles from "./DecisionCard.module.css";

const OP = { add: ["+", "新增"], modify: ["~", "修改"], remove: ["−", "移除"] } as const;

/**
 * 决定卡片（6.1）：要人决定的事都用它。状态一句话、量化、证据、选项、「会写入 · N 处」、
 * 快照与撤销、写动作的按钮，依次往下。点了按钮就写入，完成后卡片原地显示结果和「撤销」。
 */
export function DecisionCard({
  title,
  metric,
  tone = "neutral",
  children,
  foot,
  label,
}: {
  title: string;
  /** 量化，例如「+3 −1」「−6.4k 停用后」。 */
  metric?: ReactNode;
  tone?: "neutral" | "warn" | "info";
  children?: ReactNode;
  /** 卡片底部一行：左边快照说明，右边按钮。 */
  foot?: ReactNode;
  /** 读屏念的名字，默认用标题。 */
  label?: string;
}) {
  return (
    <section aria-label={label ?? title} className={styles.card} data-tone={tone}>
      <div className={styles.head}>
        <span className={styles.title}>{title}</span>
        {metric && <span className={styles.metric}>{metric}</span>}
      </div>
      {children && <div className={styles.body}>{children}</div>}
      {foot && <div className={styles.foot}>{foot}</div>}
    </section>
  );
}

/** 「会写入 · N 处」：逐条列出文件和改法，写法与变更计划相同（6.8）。 */
export function WriteList({
  changes,
  loading,
  error,
  blockedNote,
}: {
  changes: Change[] | undefined;
  loading?: boolean;
  error?: string | null;
  blockedNote?: string | null;
}) {
  return (
    <div className={styles.writes}>
      <span className={styles.writesHead}>
        {loading ? "正在算会写入哪些文件…" : `会写入 · ${changes?.length ?? 0} 处`}
      </span>
      {error && <span className={styles.error}>{error}</span>}
      {blockedNote && <span className={styles.blocked}>{blockedNote}</span>}
      {changes && changes.length > 0 && (
        <ul className={styles.writeList}>
          {changes.map((ch) => (
            <li key={`${ch.op}:${ch.path}`}>
              <span className={styles.op} data-op={ch.op} title={OP[ch.op][1]}>
                {OP[ch.op][1]}
              </span>
              <span className={styles.writeText}>
                <span className={styles.path}>{ch.path}</span>
                <span className={styles.detail}>{ch.detail}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** 单选的处理方式（6.1 决定卡片第 4 条）：推荐项排第一、默认选中。 */
export function Options<T extends string>({
  name,
  legend,
  options,
  value,
  onChange,
}: {
  name: string;
  legend: string;
  options: { id: T; title: string; desc: string; off: string | null; recommended?: boolean }[];
  value: T | undefined;
  onChange: (id: T) => void;
}) {
  return (
    <fieldset className={styles.options}>
      <legend>{legend}</legend>
      {options.map((o) => (
        <label key={o.id} className={styles.opt} data-on={value === o.id} data-off={o.off !== null}>
          <input
            type="radio"
            name={name}
            checked={value === o.id}
            disabled={o.off !== null}
            onChange={() => onChange(o.id)}
          />
          <span className={styles.optText}>
            <span className={styles.optTitle}>
              {o.title}
              {o.recommended && <span className={c.tag}>推荐</span>}
            </span>
            <span className={styles.optDesc}>{o.off ?? o.desc}</span>
          </span>
        </label>
      ))}
    </fieldset>
  );
}

/** 完成状态：结果一句话、说明和「撤销」。 */
export function Done({
  title,
  desc,
  extra,
  onUndo,
}: {
  title: string;
  desc?: string;
  extra?: ReactNode;
  onUndo?: () => void;
}) {
  return (
    <section aria-label={title} className={styles.card} data-tone="ok">
      <div className={styles.head}>
        <span className={styles.doneDot} aria-hidden="true" />
        <span className={styles.title}>{title}</span>
      </div>
      {(desc || extra) && (
        <div className={styles.body}>
          {desc && <p className={styles.text}>{desc}</p>}
          {extra}
        </div>
      )}
      {onUndo && (
        <div className={styles.foot}>
          <span className={styles.footNote}>已留快照</span>
          <button type="button" className={`${c.b} ${c.small}`} onClick={onUndo}>
            撤销
          </button>
        </div>
      )}
    </section>
  );
}

export { styles as decisionStyles };
