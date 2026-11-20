/**
 * 登记项目的第一步：问物化方式（F3、9.2）。选完交给变更计划流程（`state/plan.ts`）。
 *
 * 三个入口都走这里：检查器「登记…」、侧栏「添加项目…」，命令行另有 `fangcun init --from-existing`。
 */
import { create } from "zustand";

interface RegisterState {
  /** 要登记的目录（绝对路径）。为空时弹窗关着。 */
  root: string | null;
  open: (root: string) => void;
  close: () => void;
}

export const useRegister = create<RegisterState>((set) => ({
  root: null,
  open: (root) => set({ root }),
  close: () => set({ root: null }),
}));
