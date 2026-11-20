import { useMatch } from "react-router";
import { useUi } from "@/state/ui";
import { SETTINGS_PATH } from "./nav";

/**
 * 侧栏现在折没折、开关切的是哪一个。设置页里另算：默认折成图标栏（6.1），
 * 在那里展开不影响别的页。
 */
export function useSidebar(): { collapsed: boolean; toggle: () => void } {
  const onSettings = useMatch(SETTINGS_PATH) !== null;
  const ui = useUi();
  return onSettings
    ? { collapsed: !ui.settingsNavOpen, toggle: ui.toggleSettingsNav }
    : { collapsed: ui.sidebarCollapsed, toggle: ui.toggleSidebar };
}
