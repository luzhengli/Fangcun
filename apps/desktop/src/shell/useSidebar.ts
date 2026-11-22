import { useSyncExternalStore } from "react";
import { useUi } from "@/state/ui";

/** 窗口窄于 960px 时侧栏收起，点开后浮在内容上（6.1「窄窗口」）。 */
const NARROW = "(max-width: 959px)";

function subscribe(onChange: () => void) {
  if (typeof window === "undefined" || !window.matchMedia) return () => {};
  const query = window.matchMedia(NARROW);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

function isNarrow() {
  return typeof window !== "undefined" && !!window.matchMedia?.(NARROW).matches;
}

/**
 * 侧栏现在显不显示、是不是浮着、开关切的是什么。
 * 宽窗口里开关把侧栏整个藏起或放回；窄窗口里开关让侧栏浮出或收起。
 */
export function useSidebar(): {
  shown: boolean;
  peek: boolean;
  toggle: () => void;
  close: () => void;
} {
  const narrow = useSyncExternalStore(subscribe, isNarrow, () => false);
  const ui = useUi();
  if (narrow)
    return {
      shown: ui.sidebarPeek,
      peek: true,
      toggle: () => ui.setSidebarPeek(!ui.sidebarPeek),
      close: () => ui.setSidebarPeek(false),
    };
  return {
    shown: !ui.sidebarCollapsed,
    peek: false,
    toggle: ui.toggleSidebar,
    close: () => {},
  };
}
