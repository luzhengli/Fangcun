import { Link } from "react-router";
import type { ItemActivity } from "@/ipc/bindings/ItemActivity";
import { Icon } from "./Icon";
import styles from "./WhoBar.module.css";

/**
 * 「谁在用」：一条连续的比例条加「N/M」，点了在行下方展开项目名（6.4、6.6）。
 * 展开和选中无关，由页面记着哪些行展开了。
 */
export function WhoButton({
  id,
  used,
  total,
  full,
  open,
  onToggle,
}: {
  id: string;
  used: number;
  total: number;
  /** 指令文件全文加载，不画条。 */
  full?: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const verb = open ? "收起" : "展开";
  const what = full ? "谁在用 全文加载" : `谁在用 ${used} / ${total}`;
  return (
    <button
      type="button"
      className={styles.who}
      aria-expanded={open}
      aria-controls={`who-${id}`}
      aria-label={`${what}，${verb}项目名`}
      title={`${verb}项目名`}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
    >
      {full ? (
        <span className={styles.full}>全文加载</span>
      ) : (
        <>
          <span aria-hidden="true" className={styles.track}>
            <span
              className={styles.fill}
              style={{ width: `${total > 0 ? Math.min(100, (used / total) * 100) : 0}%` }}
            />
          </span>
          <span className={styles.used}>
            {used}/{total}
          </span>
        </>
      )}
      <Icon name="chevron" size={11} strokeWidth={1.8} className={styles.chev} />
    </button>
  );
}

export interface WhoTag {
  name: string;
  /** warn：本地已改、同名冲突，标橙；all：全部项目。 */
  tone?: "warn" | "all";
  /** 多版本并存时的版本号，或「同名冲突」「本地已改」。 */
  ver?: string;
}

/** 行下方展开的一排项目名，左边和名称对齐。 */
export function WhoList({
  id,
  label,
  lead,
  tags,
  idle,
  indent,
  open = true,
}: {
  id: string;
  label: string;
  lead?: string;
  tags: WhoTag[];
  idle?: string;
  /** 左边距，和名称对齐。 */
  indent: number;
  /** 控制展开折叠，支持 CSS Grid 过渡动画。 */
  open?: boolean;
}) {
  return (
    <div className={styles.listWrap} data-open={open} aria-hidden={!open || undefined}>
      <div className={styles.listInner}>
        <fieldset
          id={`who-${id}`}
          aria-label={label}
          className={styles.list}
          style={{ "--who-x": `${indent}px` } as React.CSSProperties}
        >
          {lead && <span className={styles.lead}>{lead}</span>}
          {tags.map((t) => (
            <span key={`${t.name}:${t.ver ?? ""}`} className={styles.tag} data-t={t.tone ?? "n"}>
              {t.name}
              {t.ver && <span className={styles.ver}>{t.ver}</span>}
            </span>
          ))}
          {idle && <span className={styles.idle}>{idle}</span>}
        </fieldset>
      </div>
    </div>
  );
}

/** 「MM-DD」，一分钟内写「刚刚」，一小时内写「N 分钟前」。 */
export function shortDay(iso: string): string {
  const d = new Date(iso);
  const minutes = Math.floor((Date.now() - d.getTime()) / 60_000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const two = (n: number) => String(n).padStart(2, "0");
  return `${two(d.getMonth() + 1)}-${two(d.getDate())}`;
}

/** 检查器「最近活动」，点一行到活动页那条记录。 */
export function RecentList({ items }: { items: ItemActivity[] }) {
  if (items.length === 0) return <p className={styles.note}>还没有活动。</p>;
  return (
    <ol className={styles.acts}>
      {items.map((a) => (
        <li key={`${a.record}:${a.text}`}>
          <Link to={`/activity?record=${encodeURIComponent(a.record)}`} title="在活动里查看">
            <span className={styles.actWhen}>{shortDay(a.at.toString())}</span>
            <span className={styles.actText}>{a.text}</span>
          </Link>
        </li>
      ))}
    </ol>
  );
}
