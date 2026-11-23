import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import type { SyncMode } from "@/ipc/bindings/SyncMode";
import { kernel } from "@/ipc/commands";
import { projectPath } from "@/shell/nav";
import { applyInline } from "@/state/plan";
import { useRegister } from "@/state/register";
import c from "./controls.module.css";
import { WriteList } from "./DecisionCard";
import styles from "./PlanHost.module.css";
import { useDialogFocus } from "./useDialogFocus";
import { usePresence, useRetainedValue } from "./usePresence";

const OPTIONS: { mode: SyncMode; title: string; desc: string }[] = [
  {
    mode: "link",
    title: "本机软链",
    desc: "项目内放软链指向本机资产库，不进 Git，零重复。适合个人项目。",
  },
  {
    mode: "vendor",
    title: "拷入仓库",
    desc: "实体文件拷入项目目录随 Git 提交，队友与云端 Agent 不装方寸也能直接用。适合多人协作。",
  },
];

/**
 * 登记面板（6.3「登记」，方寸 2）：选择资产在项目里的同步方式，按提交者人数标「推荐」；
 * 下面「会写入」逐条列文件，随所选方式变；点「登记」就写，提示条给撤销。
 */
export function RegisterHost() {
  const root = useRegister((s) => s.root);
  const open = root !== null;
  const presence = usePresence(open, 100);
  const activeRoot = useRetainedValue(root);
  if (!presence.mounted || !activeRoot) return null;
  return (
    <RegisterDialog key={activeRoot} root={activeRoot} open={open} exiting={presence.exiting} />
  );
}

function RegisterDialog({
  root,
  open,
  exiting,
}: {
  root: string;
  open: boolean;
  exiting: boolean;
}) {
  const close = useRegister((s) => s.close);
  const dialog = useRef<HTMLDivElement>(null);
  useDialogFocus(dialog, open);
  const navigate = useNavigate();
  const client = useQueryClient();
  const suggestion = useQuery({
    queryKey: ["suggest-mode", root],
    queryFn: () => kernel.suggestMode(root),
  });
  const [mode, setMode] = useState<SyncMode | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const picked = mode ?? suggestion.data?.mode ?? "link";
  const plan = useQuery({
    queryKey: ["init-plan", root, picked],
    queryFn: () => kernel.planInit(root, picked),
    enabled: suggestion.isSuccess || mode !== null,
    placeholderData: (prev) => prev,
  });
  const name = root.split("/").filter(Boolean).pop() ?? root;
  const committers = suggestion.data?.committers ?? 0;
  const recommended = suggestion.data?.mode;
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, close]);
  const register = async () => {
    if (!plan.data || plan.data.blocked) return;
    setBusy(true);
    setError(null);
    try {
      await applyInline(
        plan.data,
        (applied) => `已登记 ${name} · 快照 ${applied.snapshots.map((s) => `#${s}`).join("、")}`,
        () => client.invalidateQueries({ predicate: (q) => q.queryKey[0] !== "tools" }),
      );
      close();
      navigate(projectPath(root));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };
  const basis = suggestion.isPending
    ? "正在看 git 历史…"
    : committers === 0
      ? "不是 git 仓库，建议「本机软链」。"
      : `提交者 ${committers} 位，建议「${committers >= 2 ? "拷入仓库" : "本机软链"}」。`;

  return (
    <div
      className={styles.veil}
      data-exiting={exiting || undefined}
      aria-hidden={exiting || undefined}
    >
      <div
        ref={dialog}
        role="dialog"
        aria-modal="true"
        aria-labelledby="reg-title"
        className={`${styles.dialog} ${styles.narrow}`}
        data-exiting={exiting || undefined}
      >
        <div className={styles.head}>
          <h2 id="reg-title" className={styles.title}>
            登记 {name}
          </h2>
          <p className={styles.sub}>选择资产在项目里的同步方式。{basis}</p>
        </div>
        <fieldset className={styles.options}>
          <legend className={styles.srOnly}>同步方式</legend>
          {OPTIONS.map((o) => (
            <label key={o.mode} className={styles.option} data-on={picked === o.mode}>
              <input
                type="radio"
                name="reg-mode"
                checked={picked === o.mode}
                onChange={() => setMode(o.mode)}
              />
              <span className={styles.optionText}>
                <span className={styles.optionTitle}>
                  {o.title}
                  {recommended === o.mode && <span className={c.tag}>推荐</span>}
                </span>
                <span className={styles.optionDesc}>{o.desc}</span>
              </span>
            </label>
          ))}
        </fieldset>
        <div className={styles.regWrites}>
          <WriteList
            changes={plan.data?.changes}
            loading={plan.isLoading}
            updating={plan.isFetching && plan.isPlaceholderData}
            error={error ?? (plan.isError ? plan.error.message : null)}
          />
        </div>
        <div className={styles.foot}>
          <span className={styles.footNote}>不会触碰其他任何文件。</span>
          <button type="button" className={c.b} onClick={close}>
            取消
          </button>
          <button
            type="button"
            className={`${c.b} ${c.primary}`}
            data-autofocus="true"
            disabled={!plan.data || plan.data.blocked || busy}
            onClick={() => void register()}
          >
            {busy ? "正在登记…" : "登记"}
          </button>
        </div>
      </div>
    </div>
  );
}
