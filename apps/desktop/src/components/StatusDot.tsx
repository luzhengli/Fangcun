import styles from "./components.module.css";
import { TONE_COLORS, type Tone } from "./tone";

/** 圆点加短标签，用在列表的状态列。要处理的状态文字加粗（6.10）。 */
export function StatusDot({
  tone,
  label,
  strong = false,
}: {
  tone: Tone;
  label: string;
  strong?: boolean;
}) {
  const [text, dot] = TONE_COLORS[tone];
  return (
    <span className={styles.status} style={{ color: text, fontWeight: strong ? 600 : undefined }}>
      <span aria-hidden="true" className={styles.dot} style={{ background: dot }} />
      {label}
    </span>
  );
}

/** 检查器里名称下面的状态徽标。 */
export function StatusBadge({ tone, label }: { tone: Tone; label: string }) {
  const [text, dot] = TONE_COLORS[tone];
  return (
    <span className={styles.badge} style={{ color: text }}>
      <span aria-hidden="true" className={styles.dot} style={{ background: dot }} />
      {label}
    </span>
  );
}
