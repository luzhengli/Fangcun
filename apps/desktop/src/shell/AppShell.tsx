import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";
import { CommandPalette } from "@/components/CommandPalette";
import { PlanHost } from "@/components/PlanHost";
import { RegisterHost } from "@/components/RegisterHost";
import { insideTauri } from "@/ipc/client";
import styles from "./AppShell.module.css";
import { SETTINGS_PATH } from "./nav";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { useSidebar } from "./useSidebar";

/**
 * 窗口骨架（6.1，方寸 2），所有页面共用：左边是贯通到顶的侧栏，右边是统一工具栏和主区。
 *
 * 主区里的「列表 + 检查器」由各页面自己摆，因为套装页的左栏和别的页不一样。
 * 页面的标题和工具栏控件用 `PageToolbar` 放进工具栏。
 * 窗口固定为一屏高，各栏各自滚动；窄于 900px 时改为上下堆叠，整页滚动。
 */
export function AppShell() {
  useOpenSettings();
  useSidebarShortcut();
  useKernelEvents();
  return (
    <div className={styles.shell}>
      <Sidebar />
      <div className={styles.column}>
        <Toolbar />
        <main className={styles.main}>
          <Outlet />
        </main>
      </div>
      <PlanHost />
      <RegisterHost />
      <CommandPalette />
    </div>
  );
}

/**
 * 内核事件（7.2 监听器、M1-15）：外壳发来 `kernel-event`（发现漂移、活动记录新增），
 * 让除了工具、设置以外的查询失效，界面自动重取：页头徽标、检查器、活动页、侧栏待处理数。
 */
function useKernelEvents() {
  const client = useQueryClient();
  useEffect(() => {
    if (!insideTauri()) return;
    let stop: (() => void) | undefined;
    let gone = false;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen("kernel-event", () => {
        void client.invalidateQueries({
          predicate: (q) => q.queryKey[0] !== "settings" && q.queryKey[0] !== "tools",
        });
      }).then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      }),
    );
    return () => {
      gone = true;
      stop?.();
    };
  }, [client]);
}

/**
 * 「设置…」：应用菜单发来 `open-settings` 事件（快捷键 ⌘, 在菜单上）；
 * 浏览器里没有应用菜单，直接听 ⌘,。
 */
function useOpenSettings() {
  const navigate = useNavigate();
  useEffect(() => {
    if (insideTauri()) {
      let stop: (() => void) | undefined;
      let gone = false;
      void import("@tauri-apps/api/event").then(({ listen }) =>
        listen("open-settings", () => navigate(SETTINGS_PATH)).then((unlisten) => {
          if (gone) unlisten();
          else stop = unlisten;
        }),
      );
      return () => {
        gone = true;
        stop?.();
      };
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === ",") {
        e.preventDefault();
        navigate(SETTINGS_PATH);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);
}

/** ⌃⌘S：收起或展开侧栏（与按钮 title 声明对齐），弹窗开着时不抢按键。 */
function useSidebarShortcut() {
  const { toggle } = useSidebar();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "s") {
        if (document.querySelector('[role="dialog"]')) return;
        e.preventDefault();
        toggle();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);
}
