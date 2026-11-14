import type { ToolId } from "@/ipc/bindings/ToolId";
import { useTools } from "@/ipc/queries";
import styles from "./components.module.css";

/** 目标工具分段控件。只列完整支持的工具。 */
export function ToolSwitch({
  value,
  onChange,
}: {
  value: ToolId;
  onChange: (tool: ToolId) => void;
}) {
  const tools = useTools();
  const full = (tools.data ?? []).filter((t) => t.support === "full");
  return (
    <fieldset aria-label="目标工具" className={styles.segmented}>
      {full.map((tool) => (
        <button
          key={tool.id}
          type="button"
          aria-pressed={tool.id === value}
          onClick={() => onChange(tool.id)}
        >
          {tool.name}
        </button>
      ))}
    </fieldset>
  );
}
