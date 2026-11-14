/**
 * 状态的语气 → 「墨格」的颜色。状态一律「圆点加短标签」（6.10），颜色只用这几种语义，不新增。
 */
import type { AssetState } from "@/ipc/bindings/AssetState";

export type Tone = "neutral" | "muted" | "ok" | "warn" | "info" | "unknown" | "danger";

/** 文字色与圆点色。照抄原型脚本段的 TONE 表。 */
export const TONE_COLORS: Record<Tone, [text: string, dot: string]> = {
  neutral: ["var(--tx-2)", "var(--muted-dot)"],
  muted: ["var(--tx-3)", "var(--muted-dot)"],
  ok: ["var(--ok)", "var(--ok-dot)"],
  warn: ["var(--warn)", "var(--warn-dot)"],
  info: ["var(--info)", "var(--info-dot)"],
  unknown: ["var(--unknown)", "var(--unknown-dot)"],
  danger: ["var(--danger)", "var(--danger-dot)"],
};

/** 六种资产状态的语气（6.10）：漂移橙、可更新蓝，避开红绿对立。 */
export function toneOfState(state: AssetState | null): Tone {
  switch (state) {
    case "synced":
      return "ok";
    case "drifted":
      return "warn";
    case "updatable":
      return "info";
    case "script-changed":
      return "danger";
    case "unknown-source":
      return "unknown";
    case "overridden":
      return "muted";
    default:
      return "neutral";
  }
}

/** 「6.4k」的写法，与内核 `format_k` 一致：四舍五入到一位小数。 */
export function formatK(tokens: number): string {
  if (tokens > 0 && tokens < 50) return "<0.1k";
  const tenths = Math.floor((tokens + 50) / 100);
  return `${Math.floor(tenths / 10)}.${tenths % 10}k`;
}

/** 预算上限的写法：整千写成「20k」。 */
export function formatLimit(tokens: number): string {
  return tokens % 1000 === 0 ? `${tokens / 1000}k` : formatK(tokens);
}
