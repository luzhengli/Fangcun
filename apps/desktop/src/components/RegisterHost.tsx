import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import type { SyncMode } from "@/ipc/bindings/SyncMode";
import { kernel } from "@/ipc/commands";
import { projectPath } from "@/shell/nav";
import { usePlan } from "@/state/plan";
import { useRegister } from "@/state/register";
import styles from "./PlanHost.module.css";

const OPTIONS: { mode: SyncMode; title: string; desc: string }[] = [
  {
    mode: "link",
    title: "链接",
    desc: "项目目录里是指向资产库的软链，零重复。适合个人项目。",
  },
  {
    mode: "vendor",
    title: "入库",
    desc: "实体文件提交进仓库，同事与云端 Agent 不装方寸也能用。适合多人协作的仓库。",
  },
];

/**
 * 登记弹窗（Main.dc.html「登记：先问物化方式」）：按提交者人数标「建议」，
 * 选完出登记的变更计划；计划里「上一步」回到这里。
 */
export function RegisterHost() {
  const root = useRegister((s) => s.root);
  if (!root) return null;
  return <RegisterDialog key={root} root={root} />;
}

function RegisterDialog({ root }: { root: string }) {
  const close = useRegister((s) => s.close);
  const reopen = useRegister((s) => s.open);
  const preview = usePlan((s) => s.preview);
  const navigate = useNavigate();
  const suggestion = useQuery({
    queryKey: ["suggest-mode", root],
    queryFn: () => kernel.suggestMode(root),
  });
  const [mode, setMode] = useState<SyncMode | null>(null);
  const picked = mode ?? suggestion.data?.mode ?? "link";
  const name = root.split("/").filter(Boolean).pop() ?? root;
  const committers = suggestion.data?.committers ?? 0;

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  const next = () => {
    close();
    void preview({
      load: () => kernel.planInit(root, picked),
      done: (applied) =>
        `已登记 ${name}，生成清单与锁 · 快照 ${applied.snapshots.map((s) => `#${s}`).join("、")}`,
      back: () => reopen(root),
      after: () => navigate(projectPath(root)),
    });
  };

  return (
    <div className={styles.veil}>
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="reg-title"
        className={`${styles.dialog} ${styles.narrow}`}
      >
        <div className={styles.head}>
          <h2 id="reg-title" className={styles.title}>
            登记 {name}
          </h2>
          <p className={styles.sub}>
            选一种物化方式，写进清单的 mode。只问这一次，以后在清单里改。
          </p>
        </div>
        <fieldset className={styles.options}>
          <legend className={styles.srOnly}>物化方式</legend>
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
                  {suggestion.data?.mode === o.mode && <span className={styles.badge}>建议</span>}
                </span>
                <span className={styles.optionDesc}>{o.desc}</span>
              </span>
            </label>
          ))}
          <span className={styles.basis}>
            {suggestion.isPending
              ? "正在看 git 历史…"
              : committers === 0
                ? "不是 git 仓库，按个人项目建议链接。两位及以上提交者时建议入库。"
                : `建议的依据：git 历史里有 ${committers} 位提交者（不算 [bot]），${
                    committers >= 2
                      ? "按多人协作建议入库"
                      : "按个人项目建议链接。两位及以上时建议入库"
                  }。`}
          </span>
        </fieldset>
        <div className={`${styles.foot} ${styles.footEnd}`}>
          <button type="button" className={styles.cancel} onClick={close}>
            取消
          </button>
          <button type="button" className={styles.apply} onClick={next}>
            预览变更
          </button>
        </div>
      </div>
    </div>
  );
}
