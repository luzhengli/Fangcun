import { Outlet } from "react-router";
import styles from "./AppShell.module.css";
import { Sidebar } from "./Sidebar";
import { WindowToolbar } from "./WindowToolbar";

/**
 * 窗口骨架（6.1），所有页面共用：整窗工具栏在上，下面是侧栏和主区。
 *
 * 主区里的「列表 + 检查器」由各页面自己摆，因为套装页的左栏和别的页不一样。
 * 窗口固定为一屏高，各栏各自滚动；窄于 900px 时改为上下堆叠，整页滚动。
 */
export function AppShell() {
  return (
    <div className={styles.shell}>
      <WindowToolbar />
      <div className={styles.split}>
        <Sidebar />
        <main className={styles.main}>
          <Outlet />
        </main>
      </div>
    </div>
  );
}
