import type { Scope } from "@/ipc/bindings/Scope";
import styles from "./components.module.css";

/** 四层作用域的顺序，也是层级刻度四个方格的顺序（6.10）。 */
export const SCOPES: readonly Scope[] = ["global", "bundle", "project", "personal"];

export const SCOPE_NAMES: Record<Scope, string> = {
  global: "全局",
  bundle: "套装",
  project: "项目",
  personal: "个人",
};

/** 签名元素「层级刻度」：四条递增阶梯刻度，实心蓝色条代表当前生效层，悬停可读文字说明。 */
export function ScopeTicks({ scope }: { scope: Scope }) {
  return (
    <span
      aria-hidden="true"
      title={`生效层：${SCOPE_NAMES[scope]}层（全局 → 套装 → 项目 → 个人）`}
      className={styles.ticks}
    >
      {SCOPES.map((s) => (
        <span key={s} className={styles.tick} data-on={s === scope} />
      ))}
    </span>
  );
}
