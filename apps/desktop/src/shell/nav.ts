import type { IconName } from "@/components/Icon";
import type { Overview } from "@/ipc/bindings/Overview";

export interface NavItem {
  label: string;
  to: string;
  icon: IconName;
  /** 旁边的数字取自内核概览的哪一项。 */
  count: keyof Overview;
  /** 「活动」旁是待处理数，用橙色；其余是数量，用灰色。 */
  countTone: "muted" | "warn";
}

/** 侧栏的五个导航项，顺序固定（6.1）。 */
export const NAV_ITEMS: readonly NavItem[] = [
  { label: "项目", to: "/projects", icon: "projects", count: "projects", countTone: "muted" },
  { label: "资产库", to: "/library", icon: "library", count: "library", countTone: "muted" },
  { label: "套装", to: "/bundles", icon: "bundles", count: "bundles", countTone: "muted" },
  { label: "全局", to: "/global", icon: "global", count: "global", countTone: "muted" },
  { label: "活动", to: "/activity", icon: "activity", count: "pending", countTone: "warn" },
];

/** 体检报告不在五个导航项里，入口是侧栏底部的「重新体检」。 */
export const CHECKUP_PATH = "/checkup";

/** 项目页的地址。项目标识是根目录的绝对路径，要编码。 */
export function projectPath(projectId: string): string {
  return `/projects/${encodeURIComponent(projectId)}`;
}
