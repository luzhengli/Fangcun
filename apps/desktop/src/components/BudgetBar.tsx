import type { Budget } from "@/ipc/bindings/Budget";
import styles from "./components.module.css";
import { formatK } from "./tone";

/**
 * 上下文预算条：指令、Skill、MCP 三段，同一色相三档明度，对照项目预算，始终标「估算值」。
 * 项目页和全局页共用。`lead` 是左边的说明，例如「常驻上下文」。
 */
export function BudgetBar({ budget, lead }: { budget: Budget; lead: React.ReactNode }) {
  const limit = Math.max(budget.limit, 1);
  const total = budget.instructions + budget.skills + budget.mcp;
  const width = (n: number) => `${Math.min(100, (n / limit) * 100)}%`;
  const mcpLabel =
    budget.mcpUnestimated > 0 && budget.mcp === 0
      ? `MCP ${budget.mcpUnestimated} 个未估算`
      : `MCP ${formatK(budget.mcp)}`;
  return (
    <section aria-label="常驻上下文预算" className={styles.budget}>
      <div className={styles.budgetLine}>
        {lead}
        <span className={styles.spacer} />
        <span className={styles.legend}>
          <span
            aria-hidden="true"
            className={styles.swatch}
            style={{ background: "var(--bar-1)" }}
          />
          指令 {formatK(budget.instructions)}
        </span>
        <span className={styles.legend}>
          <span
            aria-hidden="true"
            className={styles.swatch}
            style={{ background: "var(--bar-2)" }}
          />
          Skill {formatK(budget.skills)}
        </span>
        <span className={styles.legend}>
          <span
            aria-hidden="true"
            className={styles.swatch}
            style={{ background: "var(--bar-3)" }}
          />
          {mcpLabel}
        </span>
      </div>
      <div
        role="img"
        aria-label={`常驻 ${formatK(total)}，预算 ${formatK(budget.limit)}，估算值`}
        className={styles.bar}
      >
        <span style={{ width: width(budget.instructions), background: "var(--bar-1)" }} />
        <span style={{ width: width(budget.skills), background: "var(--bar-2)" }} />
        <span style={{ width: width(budget.mcp), background: "var(--bar-3)" }} />
      </div>
    </section>
  );
}
