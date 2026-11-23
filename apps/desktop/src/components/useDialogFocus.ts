import { type RefObject, useEffect } from "react";

const FOCUSABLE =
  'button:not([disabled]):not([tabindex="-1"]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * 弹层的焦点（6.1「键盘」、D-34）：打开时焦点移进弹层（第一个可聚焦的元素，没有就是弹层本身），
 * Tab 在弹层里转圈，关闭后回到打开前聚焦的那个元素（通常是触发它的按钮）。
 * 变更计划、登记面板、命令面板共用。
 */
export function useDialogFocus(ref: RefObject<HTMLElement | null>, open: boolean) {
  useEffect(() => {
    if (!open) return;
    const back = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const box = ref.current;
    if (box && !box.contains(document.activeElement)) {
      const target =
        box.querySelector<HTMLElement>('[data-autofocus="true"]:not([disabled])') ??
        box.querySelector<HTMLElement>(FOCUSABLE);
      if (target) target.focus();
      else {
        box.tabIndex = -1;
        box.focus();
      }
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab" || !ref.current) return;
      const items = [...ref.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
        (el) => el.offsetParent !== null || el === document.activeElement,
      );
      if (items.length === 0) {
        e.preventDefault();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (e.shiftKey && (active === first || !ref.current.contains(active))) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && (active === last || !ref.current.contains(active))) {
        e.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      if (back?.isConnected) back.focus();
    };
  }, [ref, open]);
}
