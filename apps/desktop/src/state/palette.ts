/**
 * 命令面板 `⌘K`（6.1、6.9）开没开。挂在窗口骨架上，任何一页都能打开。
 */
import { create } from "zustand";

interface PaletteState {
  open: boolean;
  /** 打开前焦点所在的元素，关闭后还给它（6.1「键盘」）。 */
  returnTo: HTMLElement | null;
  show: () => void;
  hide: () => void;
}

export const usePalette = create<PaletteState>((set, get) => ({
  open: false,
  returnTo: null,
  show: () =>
    set({
      open: true,
      returnTo: document.activeElement instanceof HTMLElement ? document.activeElement : null,
    }),
  hide: () => {
    const back = get().returnTo;
    set({ open: false, returnTo: null });
    back?.focus();
  },
}));
