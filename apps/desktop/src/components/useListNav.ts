import { type RefObject, useEffect } from "react";

/**
 * 列表键盘 ↑↓ 漫游：有弹窗开启或焦点在输入框时自动让位；切到新项后温和滚入可视区域。
 */
export function useListNav(
  ids: readonly string[],
  selectedId: string | null | undefined,
  onSelect: (id: string) => void,
  containerRef?: RefObject<HTMLElement | null>,
) {
  useEffect(() => {
    if (ids.length === 0) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
      if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return;

      const target = e.target as HTMLElement | null;
      if (target) {
        const tag = target.tagName;
        if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT" || target.isContentEditable) {
          return;
        }
        // 菜单或对话框内部有自己的焦点导航，不抢按键。
        if (target.closest('[role="menu"], [role="dialog"]')) return;
      }
      if (document.querySelector('[role="dialog"]')) return;

      e.preventDefault();
      const currentIdx = selectedId ? ids.indexOf(selectedId) : -1;
      const step = e.key === "ArrowDown" ? 1 : -1;
      const nextIdx =
        currentIdx === -1
          ? e.key === "ArrowDown"
            ? 0
            : ids.length - 1
          : (currentIdx + step + ids.length) % ids.length;
      const nextId = ids[nextIdx];
      if (!nextId) return;
      onSelect(nextId);

      const root = containerRef?.current ?? document;
      const el = root.querySelector<HTMLElement>(`[data-nav-id="${CSS.escape(nextId)}"]`);
      if (el && typeof el.scrollIntoView === "function") {
        el.scrollIntoView({ block: "nearest" });
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [ids, selectedId, onSelect, containerRef]);
}
