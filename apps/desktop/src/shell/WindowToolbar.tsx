import { useRef } from "react";
import { useLocation, useNavigate, useNavigationType } from "react-router";
import { Icon } from "@/components/Icon";
import { useUi } from "@/state/ui";
import styles from "./AppShell.module.css";

/**
 * 整窗工具栏（6.1）：高 40px，贯穿窗口顶部。从左到右是窗口按钮、后退与前进、侧栏开关。
 *
 * 窗口按钮是系统画的（tauri.conf.json 里 titleBarStyle 为 Overlay），这里只留出位置。
 * 整条工具栏可以拖动窗口。
 */
export function WindowToolbar() {
  const navigate = useNavigate();
  const collapsed = useUi((state) => state.sidebarCollapsed);
  const toggleSidebar = useUi((state) => state.toggleSidebar);
  const sidebarLabel = collapsed ? "展开侧栏" : "折叠侧栏";
  const { canBack, canForward } = useHistoryPosition();

  return (
    <div role="toolbar" aria-label="窗口工具栏" className={styles.toolbar} data-tauri-drag-region>
      <span className={styles.trafficLights} aria-hidden="true" />
      <button
        type="button"
        className={styles.toolbarButton}
        disabled={!canBack}
        aria-label="后退"
        onClick={() => navigate(-1)}
      >
        <Icon name="back" size={15} />
      </button>
      <button
        type="button"
        className={styles.toolbarButton}
        disabled={!canForward}
        aria-label="前进"
        onClick={() => navigate(1)}
      >
        <Icon name="forward" size={15} />
      </button>
      <button
        type="button"
        className={styles.toolbarButton}
        data-active={!collapsed}
        aria-label={sidebarLabel}
        aria-expanded={!collapsed}
        title={sidebarLabel}
        onClick={toggleSidebar}
      >
        <Icon name="sidebar" size={15} />
      </button>
    </div>
  );
}

/**
 * 没有可退可进的历史时，后退与前进置灰。
 * react-router 在 history.state 里记了当前条目的序号 `idx`；新跳转会截掉前面的条目。
 */
function useHistoryPosition() {
  useLocation();
  const type = useNavigationType();
  const max = useRef(0);
  const state =
    typeof window === "undefined" ? null : (window.history.state as { idx?: number } | null);
  const idx = state?.idx ?? 0;
  if (type === "PUSH") max.current = idx;
  else max.current = Math.max(max.current, idx);
  return { canBack: idx > 0, canForward: idx < max.current };
}
