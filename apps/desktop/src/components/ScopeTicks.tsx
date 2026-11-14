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

/** 签名元素「层级刻度」：四个 6px 小方格，实心格是所在的层。 */
export function ScopeTicks({ scope }: { scope: Scope }) {
  return (
    <span aria-hidden="true" className={styles.ticks}>
      {SCOPES.map((s) => (
        <span key={s} className={styles.tick} data-on={s === scope} />
      ))}
    </span>
  );
}
