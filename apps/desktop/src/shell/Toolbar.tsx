import { useRef } from "react";
import { useLocation, useNavigate, useNavigationType } from "react-router";
import c from "@/components/controls.module.css";
import { Icon } from "@/components/Icon";
import styles from "./AppShell.module.css";
import { useToolbarSlots } from "./toolbarSlots";
import { useSidebar } from "./useSidebar";

/**
 * 统一工具栏（6.1）：在侧栏右边，高 52px。左起是侧栏开关（侧栏收起时才在这里，连同窗口按钮的位置）、
 * 后退与前进、页面标题加副标题，右边是这一页的控件和主按钮。标题和控件由各页的 `PageToolbar` 填。
 * 整条可以拖动窗口。
 */
export function Toolbar() {
  const navigate = useNavigate();
  const sidebar = useSidebar();
  const { canBack, canForward } = useHistoryPosition();
  const setTitle = useToolbarSlots((s) => s.setTitle);
  const setActions = useToolbarSlots((s) => s.setActions);
  const showLead = !sidebar.shown || sidebar.peek;

  return (
    <header
      role="toolbar"
      aria-label="窗口工具栏"
      className={styles.toolbar}
      data-tauri-drag-region
    >
      {showLead && (
        <span className={styles.lead}>
          <span className={styles.trafficLights} aria-hidden="true" />
          <button
            type="button"
            className={c.icon}
            aria-label="显示侧栏"
            aria-expanded={sidebar.shown}
            title="显示侧栏 ⌃⌘S"
            onClick={sidebar.toggle}
          >
            <Icon name="sidebar" strokeWidth={1.4} />
          </button>
        </span>
      )}
      <div className={styles.history}>
        <button
          type="button"
          className={c.icon}
          disabled={!canBack}
          aria-label="后退"
          onClick={() => navigate(-1)}
        >
          <Icon name="back" strokeWidth={1.6} />
        </button>
        <button
          type="button"
          className={c.icon}
          disabled={!canForward}
          aria-label="前进"
          onClick={() => navigate(1)}
        >
          <Icon name="forward" strokeWidth={1.6} />
        </button>
      </div>
      <div ref={setTitle} className={styles.titleSlot} />
      <span className={styles.spacer} data-tauri-drag-region />
      <div ref={setActions} className={styles.actions} />
    </header>
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
