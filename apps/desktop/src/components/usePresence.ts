import { useEffect, useState } from "react";

/**
 * 在 SSR、jsdom 单元测试环境，或用户开启「减少动态效果」时，跳过进退场延时，
 * 确保浮层与抽屉在测试中 0ms 同步卸载且严格尊重无障碍偏好。
 */
export function shouldSkipMotion(): boolean {
  if (typeof window === "undefined") return true;
  if (typeof navigator !== "undefined" && /jsdom/i.test(navigator.userAgent)) return true;
  if (typeof window.matchMedia !== "function") return true;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * 极薄存在态钩子：控制浮层、抽屉、底部浮条等的受控退场与主动切换入场。
 * - 首屏挂载时 `entering` 为 `false`，避免静态截图或初进页面触发多余切换动画；
 * - 仅在 `open` 发生切换时进入 `entering` 或 `exiting`；
 * - 在 jsdom 或 `prefers-reduced-motion: reduce` 下于渲染期同步返回卸载态（0ms）。
 */
export function usePresence(
  open: boolean,
  exitMs = 100,
): { mounted: boolean; exiting: boolean; entering: boolean } {
  const skip = shouldSkipMotion();
  const [prevOpen, setPrevOpen] = useState(open);
  const [exiting, setExiting] = useState(false);
  const [entering, setEntering] = useState(false);

  if (open !== prevOpen) {
    setPrevOpen(open);
    if (skip) {
      setExiting(false);
      setEntering(false);
    } else if (open) {
      setExiting(false);
      setEntering(true);
    } else {
      setEntering(false);
      setExiting(true);
    }
  }

  useEffect(() => {
    if (!open && exiting) {
      if (skip) {
        setExiting(false);
        return;
      }
      const timer = window.setTimeout(() => setExiting(false), exitMs);
      return () => window.clearTimeout(timer);
    }
    if (open && entering) {
      if (skip) {
        setEntering(false);
        return;
      }
      const timer = window.setTimeout(() => setEntering(false), exitMs);
      return () => window.clearTimeout(timer);
    }
    return undefined;
  }, [open, exiting, entering, exitMs, skip]);

  if (open) {
    return { mounted: true, exiting: false, entering: skip ? false : entering };
  }
  if (skip) {
    return { mounted: false, exiting: false, entering: false };
  }
  return { mounted: exiting, exiting, entering: false };
}

/**
 * 保留最近一次非空快照，供浮层/提示条/浮条在受控退场期间保持关闭前内容而不闪空。
 * 调用方在传入对象时应保持引用稳定（如 useMemo），避免无意义触发渲染期状态同步。
 */
export function useRetainedValue<T>(value: T | null): T | null {
  const [retained, setRetained] = useState<T | null>(value);
  if (value !== null && !Object.is(value, retained)) {
    setRetained(value);
  }
  return value ?? retained;
}
