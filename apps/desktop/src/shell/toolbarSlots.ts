import { create } from "zustand";

/**
 * 统一工具栏（6.1）由外壳画，标题和右侧控件由各页填：外壳把两个空位登记在这里，
 * 页面里的 `PageToolbar` 用 portal 把内容放进去。
 */
export const useToolbarSlots = create<{
  title: HTMLElement | null;
  actions: HTMLElement | null;
  setTitle: (el: HTMLElement | null) => void;
  setActions: (el: HTMLElement | null) => void;
}>((set) => ({
  title: null,
  actions: null,
  setTitle: (title) => set({ title }),
  setActions: (actions) => set({ actions }),
}));
