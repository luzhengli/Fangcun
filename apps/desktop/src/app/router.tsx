import { createHashRouter, Navigate, type RouteObject } from "react-router";
import { ActivityPage } from "@/pages/activity/ActivityPage";
import { BundlesPage } from "@/pages/bundles/BundlesPage";
import { CheckupPage } from "@/pages/checkup/CheckupPage";
import { GlobalPage } from "@/pages/global/GlobalPage";
import { LibraryPage } from "@/pages/library/LibraryPage";
import { ProjectPage } from "@/pages/project/ProjectPage";
import { AppShell } from "@/shell/AppShell";
import { CHECKUP_PATH } from "@/shell/nav";

/**
 * 路由表。五个导航项各一页，加上体检报告。
 *
 * 首次启动先进体检报告（6.2）。M0 不落盘，记不住上次启动，所以每次启动都按首次处理。
 */
export const routes: RouteObject[] = [
  {
    path: "/",
    element: <AppShell />,
    children: [
      { index: true, element: <Navigate to={CHECKUP_PATH} replace /> },
      { path: CHECKUP_PATH, element: <CheckupPage /> },
      { path: "projects", element: <ProjectPage /> },
      { path: "projects/:projectId", element: <ProjectPage /> },
      { path: "library", element: <LibraryPage /> },
      { path: "bundles", element: <BundlesPage /> },
      { path: "global", element: <GlobalPage /> },
      { path: "activity", element: <ActivityPage /> },
    ],
  },
];

export function createAppRouter() {
  return createHashRouter(routes);
}
