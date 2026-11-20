/**
 * 变更计划的流程（6.8）：预览 → 应用 → 提示条，全应用共用一份。
 *
 * 页面只说「要预览哪份计划」和「应用后提示什么」，弹窗、应用、提示条、撤销都由
 * `components/PlanHost` 统一处理。计划本身是内核的数据，这里只存当前正在看的那一份。
 */
import { create } from "zustand";
import type { Applied } from "@/ipc/bindings/Applied";
import type { ChangePlan } from "@/ipc/bindings/ChangePlan";

export interface PlanRequest {
  /** 向内核要计划。 */
  load: () => Promise<ChangePlan>;
  /** 应用后提示条上的话。 */
  done: (applied: Applied, plan: ChangePlan) => string;
  /** 能不能撤销。撤销本身应用后就不再给撤销。 */
  undoable?: boolean;
  /** 应用成功后页面自己要做的事，例如登记后切到这个项目。 */
  after?: (applied: Applied) => void;
  /** 有上一步时（登记先选物化方式），弹窗里的「取消」换成「上一步」。 */
  back?: () => void;
}

interface PlanState {
  request: PlanRequest | null;
  plan: ChangePlan | null;
  loading: boolean;
  applying: boolean;
  error: string | null;
  toast: { text: string; record: string | null } | null;
  preview: (request: PlanRequest) => Promise<void>;
  close: () => void;
  setApplying: (applying: boolean) => void;
  setError: (error: string | null) => void;
  showToast: (toast: { text: string; record: string | null } | null) => void;
}

export const usePlan = create<PlanState>((set, get) => ({
  request: null,
  plan: null,
  loading: false,
  applying: false,
  error: null,
  toast: null,
  preview: async (request) => {
    set({ request, plan: null, loading: true, error: null, toast: null });
    try {
      const plan = await request.load();
      if (get().request === request) set({ plan, loading: false });
    } catch (error) {
      if (get().request === request)
        set({ loading: false, error: error instanceof Error ? error.message : String(error) });
    }
  },
  close: () => set({ request: null, plan: null, loading: false, applying: false, error: null }),
  setApplying: (applying) => set({ applying }),
  setError: (error) => set({ error, applying: false }),
  showToast: (toast) => set({ toast }),
}));
