import { NavLink, useNavigate } from "react-router";
import { Icon } from "@/components/Icon";
import { TONE_COLORS, toneOfState } from "@/components/tone";
import type { AssetState } from "@/ipc/bindings/AssetState";
import { useOverview, useProjects, useRescan } from "@/ipc/queries";
import { useUi } from "@/state/ui";
import styles from "./AppShell.module.css";
import { NAV_ITEMS, projectPath } from "./nav";

/**
 * 侧栏（6.1）：「方寸」标识、五个导航项、项目列表、底部的「重新体检」。
 * 可折叠为 56px 宽的图标栏，此时只保留导航图标。
 *
 * M0 没有登记：列出的是近 30 天会话记录里发现的项目。状态点在 M1 有受管资产后才有颜色。
 */
/** 项目旁的短标签：只在需要处理时出现。 */
const PROJECT_NOTE: Partial<Record<AssetState, string>> = {
  drifted: "漂移",
  updatable: "可更新",
  "script-changed": "脚本变更",
};

export function Sidebar() {
  const collapsed = useUi((state) => state.sidebarCollapsed);
  const overview = useOverview();
  const projects = useProjects();
  const rescan = useRescan();
  const navigate = useNavigate();

  return (
    <nav aria-label="主导航" className={styles.sidebar} data-collapsed={collapsed}>
      {!collapsed && (
        <div className={styles.brand}>
          <span className={styles.brandMark}>
            <Icon name="logo" size={18} />
          </span>
          <span>方寸</span>
        </div>
      )}

      <div className={styles.navGroup}>
        {NAV_ITEMS.map((item) => (
          <NavLink key={item.to} to={item.to} className={styles.navItem} title={item.label}>
            <Icon name={item.icon} />
            <span className={styles.navLabel}>{item.label}</span>
            {overview.data && (
              <span className={styles.navCount} data-tone={item.countTone}>
                {overview.data[item.count]}
              </span>
            )}
          </NavLink>
        ))}
      </div>

      {!collapsed && projects.data && projects.data.length > 0 && (
        <div className={styles.navGroup}>
          <div className={styles.groupTitle}>近 30 天的项目</div>
          {projects.data.map((p) => (
            <NavLink
              key={p.id}
              to={projectPath(p.id)}
              className={styles.projectItem}
              title={p.path}
            >
              <span
                aria-hidden="true"
                className={styles.projectDot}
                style={{ background: TONE_COLORS[p.state ? toneOfState(p.state) : "neutral"][1] }}
              />
              <span className={styles.projectName}>{p.name}</span>
              {p.state && PROJECT_NOTE[p.state] && (
                <span className={styles.projectNote}>{PROJECT_NOTE[p.state]}</span>
              )}
            </NavLink>
          ))}
        </div>
      )}

      <div className={styles.sidebarFooter}>
        <button
          type="button"
          className={styles.navItem}
          title="重新体检"
          disabled={rescan.isPending}
          onClick={() => {
            navigate("/checkup");
            rescan.mutate();
          }}
        >
          <Icon name="checkup" />
          <span className={styles.navLabel}>{rescan.isPending ? "正在体检…" : "重新体检"}</span>
        </button>
        {!collapsed && <div className={styles.footnote}>本地优先 · 零遥测</div>}
      </div>
    </nav>
  );
}
