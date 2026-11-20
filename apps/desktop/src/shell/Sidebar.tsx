import { NavLink, useNavigate } from "react-router";
import { Icon } from "@/components/Icon";
import { TONE_COLORS, toneOfState } from "@/components/tone";
import type { AssetState } from "@/ipc/bindings/AssetState";
import type { ProjectSummary } from "@/ipc/bindings/ProjectSummary";
import { kernel } from "@/ipc/commands";
import { useOverview, useProjects, useRescan } from "@/ipc/queries";
import { useRegister } from "@/state/register";
import styles from "./AppShell.module.css";
import { NAV_ITEMS, projectPath, SETTINGS_PATH } from "./nav";
import { useSidebar } from "./useSidebar";

/**
 * 侧栏（6.1）：「方寸」标识、五个导航项、项目列表、底部的「设置」「重新体检」。
 * 可折叠为 56px 宽的图标栏，此时只保留导航图标。
 *
 * 项目分两组（D-28）：「已登记项目」，和「近 30 天的项目」里还没登记的目录；
 * 两组下面是「添加项目…」，选一个目录登记（F3）。
 */
/** 项目旁的短标签：只在需要处理时出现。 */
const PROJECT_NOTE: Partial<Record<AssetState, string>> = {
  drifted: "漂移",
  updatable: "可更新",
  "script-changed": "脚本变更",
};

export function Sidebar() {
  const { collapsed } = useSidebar();
  const overview = useOverview();
  const projects = useProjects();
  const rescan = useRescan();
  const navigate = useNavigate();
  const openRegister = useRegister((s) => s.open);
  const registered = projects.data?.filter((p) => p.registered) ?? [];
  const recent = projects.data?.filter((p) => !p.registered) ?? [];
  const addProject = async () => {
    const root = await kernel.pickDirectory();
    if (root) openRegister(root);
  };

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

      {!collapsed && projects.data && (
        <div className={styles.navGroup}>
          <div className={styles.groupTitle}>已登记项目</div>
          {registered.map((p) => (
            <ProjectLink key={p.id} project={p} />
          ))}
          <div className={styles.groupTitle} data-gap="true">
            近 30 天的项目
          </div>
          {recent.map((p) => (
            <ProjectLink key={p.id} project={p} />
          ))}
          {recent.length === 0 && <div className={styles.groupEmpty}>没有未登记的目录</div>}
          <button
            type="button"
            className={styles.addProject}
            title="选一个目录登记：先问物化方式，再出变更计划"
            onClick={() => void addProject()}
          >
            <span className={styles.addProjectIcon}>
              <Icon name="plus" size={12} />
            </span>
            <span>添加项目…</span>
          </button>
        </div>
      )}

      <div className={styles.sidebarFooter}>
        <NavLink to={SETTINGS_PATH} className={styles.navItem} title="设置（⌘,）">
          <Icon name="settings" />
          <span className={styles.navLabel}>设置</span>
          <span className={styles.navKbd}>⌘,</span>
        </NavLink>
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

function ProjectLink({ project: p }: { project: ProjectSummary }) {
  return (
    <NavLink to={projectPath(p.id)} className={styles.projectItem} title={p.path}>
      <span
        aria-hidden="true"
        className={styles.projectDot}
        data-hollow={!p.registered}
        style={
          p.registered
            ? { background: TONE_COLORS[p.state ? toneOfState(p.state) : "neutral"][1] }
            : undefined
        }
      />
      <span className={styles.projectName}>{p.name}</span>
      {!p.registered ? (
        <span className={styles.projectNote}>未登记</span>
      ) : (
        p.state &&
        PROJECT_NOTE[p.state] && <span className={styles.projectNote}>{PROJECT_NOTE[p.state]}</span>
      )}
    </NavLink>
  );
}
