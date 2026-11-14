/**
 * 界面自己的状态：只存在于窗口里、不属于内核的东西。
 *
 * 内核的数据（项目、资产、活动）不放这里，走 `ipc/queries.ts`。
 * 以后「同步 · N」里待应用的操作也放在这个目录，单独一个 store。
 */
import { create } from "zustand";
import type { ToolId } from "@/ipc/bindings/ToolId";

interface UiState {
  /** 侧栏是否折叠成 56px 的图标栏。 */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  /** 项目页与全局页当前看的目标工具。两页共用，切过去不用再选。 */
  tool: ToolId;
  setTool: (tool: ToolId) => void;
}

export const useUi = create<UiState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  tool: "claude-code",
  setTool: (tool) => set({ tool }),
}));
