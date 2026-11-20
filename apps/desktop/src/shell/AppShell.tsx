import { useEffect } from "react";
import { Outlet, useNavigate } from "react-router";
import { PlanHost } from "@/components/PlanHost";
import { RegisterHost } from "@/components/RegisterHost";
import { insideTauri } from "@/ipc/client";
import styles from "./AppShell.module.css";
import { SETTINGS_PATH } from "./nav";
import { Sidebar } from "./Sidebar";
import { WindowToolbar } from "./WindowToolbar";

/**
 * 窗口骨架（6.1），所有页面共用：整窗工具栏在上，下面是侧栏和主区。
 *
 * 主区里的「列表 + 检查器」由各页面自己摆，因为套装页的左栏和别的页不一样。
 * 窗口固定为一屏高，各栏各自滚动；窄于 900px 时改为上下堆叠，整页滚动。
 */
export function AppShell() {
  useOpenSettings();
  return (
    <div className={styles.shell}>
      <WindowToolbar />
      <div className={styles.split}>
        <Sidebar />
        <main className={styles.main}>
          <Outlet />
        </main>
      </div>
      <PlanHost />
      <RegisterHost />
    </div>
  );
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
