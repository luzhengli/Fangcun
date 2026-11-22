import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import type { Change } from "@/ipc/bindings/Change";
import type { ChangeOp } from "@/ipc/bindings/ChangeOp";
import type { NoteTone } from "@/ipc/bindings/NoteTone";
import { kernel } from "@/ipc/commands";
import { useReadOnly } from "@/ipc/queries";
import { usePlan } from "@/state/plan";
import c from "./controls.module.css";
import styles from "./PlanHost.module.css";
import { useDialogFocus } from "./useDialogFocus";

const OP: Record<ChangeOp, string> = { add: "+", modify: "~", remove: "−" };
const NOTE: Record<NoteTone, string> = {
  info: styles.noteInfo ?? "",
  warning: styles.noteWarning ?? "",
  danger: styles.noteDanger ?? "",
};

/**
 * 变更计划弹窗与应用后的提示条（6.8），挂在窗口骨架上，全应用一份。
 *
 * 页面调 `usePlan().preview(...)` 打开；应用后让全部查询重新取数，出提示条。
 * 提示条上的「撤销」先出回滚计划给人看，确认后才回滚（预览先于写入）。
 */
export function PlanHost() {
  const { request, plan, loading, applying, error, toast } = usePlan();
  const { close, setApplying, setError, showToast, preview } = usePlan.getState();
  const client = useQueryClient();
  const readOnly = useReadOnly();
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(dialog, request !== null);

  const apply = async () => {
    if (!plan || !request || plan.blocked || applying || readOnly) return;
    setApplying(true);
    try {
      const applied = await kernel.applyPlan(plan);
      close();
      await client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" });
      showToast({
        text: request.done(applied, plan),
        record: request.undoable === false ? null : applied.record,
      });
      request.after?.(applied);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const undo = (record: string) =>
    preview({
      load: () => kernel.planRollbackRecord(record),
      done: (_, plan) => `已撤销：${plan.title}`,
      undoable: false,
    });

  // ⌘↩ 应用，esc 取消。只在弹窗开着时生效。
  useEffect(() => {
    if (!request) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void apply();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  return (
    <>
      {request && (
        <div className={styles.veil}>
          <div
            ref={dialog}
            role="dialog"
            aria-modal="true"
            aria-labelledby="plan-title"
            className={styles.dialog}
          >
            <div className={styles.head}>
              <h2 id="plan-title" className={styles.title}>
                {plan
                  ? (request.title?.(plan) ?? plan.title)
                  : loading
                    ? "正在生成变更计划…"
                    : "变更计划"}
              </h2>
              <p className={styles.sub}>
                {plan && request.sub
                  ? request.sub(plan)
                  : "预览先于写入。应用前自动快照，可在「历史」一键回滚。"}
              </p>
            </div>
            {plan && plan.notes.length > 0 && (
              <ul className={styles.notes}>
                {plan.notes.map((n) => (
                  <li key={n.text} className={NOTE[n.tone]}>
                    {n.text}
                  </li>
                ))}
              </ul>
            )}
            {plan && plan.changes.length > 0 && (
              <div className={styles.groups}>
                {groupChanges(plan.changes, request.group?.of).map((g) => (
                  <section key={g.cause ?? ""} aria-label={g.cause ?? "变更"}>
                    {g.cause && (
                      <div className={styles.groupHead}>
                        <span className={styles.groupCause}>{g.cause}</span>
                        <span className={styles.groupCount}>{g.changes.length} 处</span>
                        {request.group?.remove && (
                          <button
                            type="button"
                            className={`${c.b} ${c.quiet} ${c.small}`}
                            onClick={() => request.group?.remove?.(g.cause ?? "")}
                          >
                            移出计划
                          </button>
                        )}
                      </div>
                    )}
                    <ul className={styles.changes}>
                      {g.changes.map((ch) => (
                        <li key={`${ch.op}:${ch.path}`}>
                          <span className={styles.op} data-op={ch.op}>
                            {OP[ch.op]}
                          </span>
                          <span className={styles.path}>{ch.path}</span>
                          <span className={styles.detail}>{ch.detail}</span>
                        </li>
                      ))}
                    </ul>
                  </section>
                ))}
              </div>
            )}
            {plan && plan.changes.length === 0 && (
              <p className={styles.empty}>磁盘已经和清单一致，没有要写的。</p>
            )}
            {error && <p className={styles.error}>{error}</p>}
            <div className={styles.foot}>
              <span className={styles.footNote}>
                {plan?.blocked ? "有冲突，不能应用。原因见上方提示。" : "不碰其他文件"}
              </span>
              <button
                type="button"
                className={c.b}
                onClick={() => {
                  close();
                  request.back?.();
                }}
              >
                {request.back ? "上一步" : "取消"}
              </button>
              <button
                type="button"
                className={`${c.b} ${c.primary}`}
                disabled={
                  readOnly || !plan || plan.blocked || plan.changes.length === 0 || applying
                }
                title={readOnly ? "只读模式：在设置里关掉后才能应用" : undefined}
                onClick={() => void apply()}
              >
                {applying ? "正在应用…" : "应用"}
                <span className={c.kbd}>⌘↩</span>
              </button>
            </div>
          </div>
        </div>
      )}
      {toast && (
        <div role="status" className={styles.toast}>
          <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
            <path
              d="m3.5 8.5 3 3 6-7"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.75"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <span>{toast.text}</span>
          {toast.record && (
            <button
              type="button"
              className={styles.toastButton}
              onClick={() => toast.record && void undo(toast.record)}
            >
              撤销
            </button>
          )}
          <button type="button" className={styles.toastButton} onClick={() => showToast(null)}>
            好
          </button>
        </div>
      )}
    </>
  );
}

/** 按原因分组，保持原因第一次出现的顺序。没给分组函数时整份一组、不写标题。 */
function groupChanges(
  changes: Change[],
  of: ((change: Change) => string) | undefined,
): { cause: string | null; changes: Change[] }[] {
  if (!of) return [{ cause: null, changes }];
  const groups = new Map<string, Change[]>();
  for (const ch of changes) {
    const cause = of(ch);
    groups.set(cause, [...(groups.get(cause) ?? []), ch]);
  }
  return [...groups].map(([cause, list]) => ({ cause, changes: list }));
}
