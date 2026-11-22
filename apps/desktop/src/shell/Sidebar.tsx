import { useEffect } from "react";
import { NavLink, useMatch, useNavigate } from "react-router";
import c from "@/components/controls.module.css";
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

/** 项目旁的短标签：只在需要处理时出现。 */
const PROJECT_NOTE: Partial<Record<AssetState, string>> = {
  drifted: "漂移",
  updatable: "可更新",
  "script-changed": "脚本变更",
};

/**
 * 侧栏（6.1，方寸 2）：贯通到窗口顶部，顶上一行是系统的窗口按钮和侧栏开关，没有「方寸」字标。
 * 下面是五个导航项、「已登记」「近 30 天 · 未登记」两组项目和「添加项目…」；
 * 底部固定「设置 ⌘,」「重新体检」和一行「本地优先 · 零遥测」。
 * 折叠时整个藏起；窗口窄于 960px 时浮在内容上，点外面或 Esc 收起。
 */
export function Sidebar() {
  const sidebar = useSidebar();
  const overview = useOverview();
  const projects = useProjects();
  const rescan = useRescan();
  const navigate = useNavigate();
  const openRegister = useRegister((s) => s.open);
  const onProject = useMatch("/projects/:projectId") !== null;
  const registered = projects.data?.filter((p) => p.registered) ?? [];
  const recent = projects.data?.filter((p) => !p.registered) ?? [];
  const addProject = async () => {
    const root = await kernel.pickDirectory();
    if (root) openRegister(root);
  };

  useEffect(() => {
    if (!sidebar.peek || !sidebar.shown) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") sidebar.close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [sidebar]);

  if (!sidebar.shown) return null;

  return (
    <>
      <nav aria-label="主导航" className={styles.sidebar} data-peek={sidebar.peek}>
        <div className={styles.sideTop} data-tauri-drag-region>
          <span className={styles.trafficLights} aria-hidden="true" />
          <span className={styles.spacer} data-tauri-drag-region />
          <button
            type="button"
            className={c.icon}
            aria-label="隐藏侧栏"
            aria-expanded="true"
            title="隐藏侧栏 ⌃⌘S"
            onClick={sidebar.toggle}
          >
            <Icon name="sidebar" strokeWidth={1.4} />
          </button>
        </div>

        <div className={styles.sideScroll}>
          <div className={styles.navGroup}>
            {NAV_ITEMS.map((item) => {
              const parent = item.to === "/projects" && onProject;
              const count = overview.data?.[item.count];
              return (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.to === "/projects"}
                  className={styles.nav}
                  data-parent={parent}
                  onClick={sidebar.close}
                >
                  <Icon name={item.icon} strokeWidth={1.4} />
                  <span className={styles.navLabel}>{item.label}</span>
                  {count !== undefined &&
                    (item.countTone === "warn" ? (
                      count > 0 && (
                        <span className={styles.badge} title="待处理">
                          {count}
                        </span>
                      )
                    ) : (
                      <span className={styles.navCount}>{count}</span>
                    ))}
                </NavLink>
              );
            })}
          </div>

          {projects.data && (
            <>
              <div className={styles.navGroup}>
                <div className={styles.groupTitle}>已登记</div>
                {registered.map((p) => (
                  <ProjectLink key={p.id} project={p} onPick={sidebar.close} />
                ))}
                {registered.length === 0 && (
                  <div className={styles.groupEmpty}>还没有登记的项目</div>
                )}
              </div>
              <div className={styles.navGroup}>
                <div className={styles.groupTitle}>近 30 天 · 未登记</div>
                {recent.map((p) => (
                  <ProjectLink key={p.id} project={p} onPick={sidebar.close} />
                ))}
                {recent.length === 0 && <div className={styles.groupEmpty}>没有未登记的目录</div>}
                <button
                  type="button"
                  className={`${styles.nav} ${styles.addProject}`}
                  title="选一个目录登记"
                  onClick={() => void addProject()}
                >
                  <Icon name="plus" strokeWidth={1.4} />
                  <span className={styles.navLabel}>添加项目…</span>
                </button>
              </div>
            </>
          )}
        </div>

        <div className={styles.sideFoot}>
          <NavLink to={SETTINGS_PATH} className={styles.nav} onClick={sidebar.close}>
            <Icon name="settings" strokeWidth={1.4} />
            <span className={styles.navLabel}>设置</span>
            <span className={c.kbd}>⌘,</span>
          </NavLink>
          <button
            type="button"
            className={styles.nav}
            disabled={rescan.isPending}
            onClick={() => {
              sidebar.close();
              navigate("/checkup");
              rescan.mutate();
            }}
          >
            <Icon name="checkup" strokeWidth={1.4} />
            <span className={styles.navLabel}>{rescan.isPending ? "正在体检…" : "重新体检"}</span>
          </button>
          <span className={styles.footnote} title="不需要账号，不上传任何数据">
            <Icon name="shield" size={11} />
            本地优先 · 零遥测
          </span>
        </div>
      </nav>
      {sidebar.peek && (
        <button
          type="button"
          className={styles.peekVeil}
          aria-label="收起侧栏"
          onClick={sidebar.close}
        />
      )}
    </>
  );
}

function ProjectLink({ project: p, onPick }: { project: ProjectSummary; onPick: () => void }) {
  const tone = p.state ? toneOfState(p.state) : "ok";
  const note = p.registered && p.state ? PROJECT_NOTE[p.state] : undefined;
  return (
    <NavLink
      to={projectPath(p.id)}
      className={`${styles.nav} ${p.registered ? "" : styles.recent}`}
      title={p.path}
      onClick={onPick}
    >
      <span
        aria-hidden="true"
        className={styles.projectDot}
        data-hollow={!p.registered}
        style={p.registered ? { background: TONE_COLORS[tone][1] } : undefined}
      />
      <span className={styles.navLabel}>{p.name}</span>
      {note && (
        <span className={styles.projectNote} style={{ color: TONE_COLORS[tone][0] }}>
          {note}
        </span>
      )}
    </NavLink>
  );
}
