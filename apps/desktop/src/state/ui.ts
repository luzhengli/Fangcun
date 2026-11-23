/**
 * 界面自己的状态：只存在于窗口里、不属于内核的东西。
 *
 * 内核的数据（项目、资产、活动）不放这里，走 `ipc/queries.ts`。
 * 以后「同步 · N」里待应用的操作也放在这个目录，单独一个 store。
 */
import { create } from "zustand";
import type { ToolId } from "@/ipc/bindings/ToolId";

function readGuideHidden(): boolean {
  try {
    return (
      typeof localStorage !== "undefined" && localStorage.getItem("fangcun.guideHidden") === "1"
    );
  } catch {
    return false;
  }
}

function writeGuideHidden(hidden: boolean): void {
  try {
    if (typeof localStorage === "undefined") return;
    if (hidden) localStorage.setItem("fangcun.guideHidden", "1");
    else localStorage.removeItem("fangcun.guideHidden");
  } catch {
    // 忽略只读存储限制
  }
}

interface UiState {
  /** 侧栏是否整个藏起（6.1，macOS 原生做法，不留图标栏）。 */
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  /** 窗口窄于 960px 时侧栏默认收起；点开后浮在内容上，点外面或 Esc 收起。 */
  sidebarPeek: boolean;
  setSidebarPeek: (open: boolean) => void;
  /** 项目页与全局页当前看的目标工具。两页共用，切过去不用再选。 */
  tool: ToolId;
  setTool: (tool: ToolId) => void;
  /** 新手四步速览条是否已收起，持久化到本地存储。 */
  guideHidden: boolean;
  hideGuide: () => void;
  showGuide: () => void;
}

export const useUi = create<UiState>((set) => ({
  sidebarCollapsed: false,
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  sidebarPeek: false,
  setSidebarPeek: (sidebarPeek) => set({ sidebarPeek }),
  tool: "claude-code",
  setTool: (tool) => set({ tool }),
  guideHidden: readGuideHidden(),
  hideGuide: () => {
    writeGuideHidden(true);
    set({ guideHidden: true });
  },
  showGuide: () => {
    writeGuideHidden(false);
    set({ guideHidden: false });
  },
}));
