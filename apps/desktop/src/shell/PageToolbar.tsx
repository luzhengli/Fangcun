import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import c from "@/components/controls.module.css";
import styles from "./AppShell.module.css";
import { useToolbarSlots } from "./toolbarSlots";

/**
 * 页面往统一工具栏里放的东西（6.1）：标题、副标题，右边的控件。
 *
 * `crumb` 给了就把标题写成面包屑「上一级 › 当前」，点上一级回去（6.1、6.3，2026-11-22 人定）。
 */
export function PageToolbar({
  title,
  subtitle,
  mono = false,
  tag,
  crumb,
  children,
}: {
  title: string;
  subtitle?: ReactNode;
  /** 副标题是路径时用等宽字体。 */
  mono?: boolean;
  /** 标题后面的小标签，例如「未登记」。 */
  tag?: string;
  crumb?: { leaf: string; back: () => void; backTitle?: string };
  children?: ReactNode;
}) {
  const titleSlot = useToolbarSlots((s) => s.title);
  const actionsSlot = useToolbarSlots((s) => s.actions);
  return (
    <>
      {titleSlot &&
        createPortal(
          <>
            <div className={styles.titleRow}>
              <h1 className={styles.title}>
                {crumb ? (
                  <>
                    <button
                      type="button"
                      className={styles.crumb}
                      title={crumb.backTitle ?? `回到${title}`}
                      onClick={crumb.back}
                    >
                      {title}
                    </button>
                    <span aria-hidden="true" className={styles.crumbSep}>
                      ›
                    </span>
                    <span className={styles.crumbLeaf}>{crumb.leaf}</span>
                  </>
                ) : (
                  title
                )}
              </h1>
              {tag && <span className={c.tag}>{tag}</span>}
            </div>
            {subtitle && (
              <span
                className={`${styles.subtitle} ${mono ? styles.subtitleMono : ""}`}
                title={typeof subtitle === "string" ? subtitle : undefined}
              >
                {subtitle}
              </span>
            )}
          </>,
          titleSlot,
        )}
      {actionsSlot && children && createPortal(children, actionsSlot)}
    </>
  );
}
