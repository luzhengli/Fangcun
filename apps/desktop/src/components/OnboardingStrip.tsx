import { useUi } from "@/state/ui";
import { Icon } from "./Icon";
import styles from "./OnboardingStrip.module.css";

const STEPS = [
  { n: "1", title: "体检看现状", desc: "找出重复、冲突与未收录文件" },
  { n: "2", title: "登记并收录", desc: "把零散本地资产收录进资产库" },
  { n: "3", title: "按项目配清单", desc: "挑选本机软链或拷入仓库" },
  { n: "4", title: "预览变更再落盘", desc: "应用前自动快照，随时一键回滚" },
] as const;

/**
 * 情景化四步速览条：展示方寸核心工作流，可随时收起并在命令面板/空白引导恢复，零全屏打断。
 */
export function OnboardingStrip() {
  const hidden = useUi((s) => s.guideHidden);
  const hide = useUi((s) => s.hideGuide);

  if (hidden) return null;

  return (
    <aside aria-label="新手四步速览" className={styles.strip}>
      <div className={styles.steps}>
        {STEPS.map((s) => (
          <div key={s.n} className={styles.step}>
            <span className={styles.num}>{s.n}</span>
            <span className={styles.text}>
              <strong>{s.title}</strong>
              <span>{s.desc}</span>
            </span>
          </div>
        ))}
      </div>
      <button
        type="button"
        className={styles.close}
        onClick={hide}
        title="不再提示四步速览（可在命令面板恢复）"
      >
        <span>不再提示</span>
        <Icon name="close" size={11} />
      </button>
    </aside>
  );
}
