/**
 * 内核数据的查询 hook。页面从这里取数据，不自己管加载和缓存。
 *
 * 查询键集中在 `keys` 里。内核推来事件（漂移、活动记录新增）或一次写入完成后，
 * 按键让对应的查询失效，界面自动重新取数。
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ManifestEdit } from "./bindings/ManifestEdit";
import type { RecordFilter } from "./bindings/RecordFilter";
import type { ToolId } from "./bindings/ToolId";
import { kernel } from "./commands";

export const keys = {
  tools: ["tools"] as const,
  overview: ["overview"] as const,
  projects: ["projects"] as const,
  checkup: ["checkup"] as const,
  effectiveContext: (projectRoot: string, tool: ToolId) =>
    ["effective-context", projectRoot, tool] as const,
  why: (projectRoot: string | null, asset: string) => ["why", projectRoot, asset] as const,
  globalOverview: (tool: ToolId) => ["global-overview", tool] as const,
  activity: (filter: RecordFilter) => ["activity", filter] as const,
  syncPlan: (projectRoot: string) => ["sync-plan", projectRoot] as const,
  settings: ["settings"] as const,
  history: (projectRoot: string) => ["history", projectRoot] as const,
  manifestView: (projectRoot: string) => ["manifest-view", projectRoot] as const,
};

export function useTools() {
  return useQuery({
    queryKey: keys.tools,
    queryFn: kernel.tools,
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useOverview() {
  return useQuery({ queryKey: keys.overview, queryFn: kernel.overview });
}

export function useProjects() {
  return useQuery({ queryKey: keys.projects, queryFn: kernel.projects });
}

export function useCheckup() {
  return useQuery({ queryKey: keys.checkup, queryFn: kernel.checkup });
}

export function useEffectiveContext(projectRoot: string | undefined, tool: ToolId) {
  return useQuery({
    queryKey: keys.effectiveContext(projectRoot ?? "", tool),
    queryFn: () => kernel.effectiveContext(projectRoot ?? "", tool),
    enabled: projectRoot !== undefined,
  });
}

export function useGlobalOverview(tool: ToolId) {
  return useQuery({
    queryKey: keys.globalOverview(tool),
    queryFn: () => kernel.globalOverview(tool),
  });
}

export function useActivity(filter: RecordFilter) {
  return useQuery({ queryKey: keys.activity(filter), queryFn: () => kernel.activity(filter) });
}

/** 「重新体检」：重新扫描全盘，然后让所有查询重新取数。 */
export function useRescan() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: kernel.rescan,
    onSuccess: (report) => {
      client.setQueryData(keys.checkup, report);
      void client.invalidateQueries({
        predicate: (q) => q.queryKey[0] !== "checkup" && q.queryKey[0] !== "tools",
      });
    },
  });
}

/**
 * 「同步 · N」里的 N：这个项目现在的同步计划有几处变更。只对已登记的项目取。
 * 每次取都会在内核里出一份计划（内核只留最近 64 份），所以不自动重取，写入后才失效。
 */
export function useSyncPlan(
  projectRoot: string | undefined,
  registered: boolean,
  edits: ManifestEdit[] = [],
) {
  return useQuery({
    queryKey: [...keys.syncPlan(projectRoot ?? ""), edits],
    queryFn: () => kernel.planSync(projectRoot ?? "", edits),
    enabled: projectRoot !== undefined && registered,
    staleTime: Number.POSITIVE_INFINITY,
    retry: false,
  });
}

export function useSettings() {
  return useQuery({ queryKey: keys.settings, queryFn: kernel.settings });
}

/** 设置页保存。只读开关一变，内核换了，别的查询都要重新取。 */
export function useSaveSettings() {
  const client = useQueryClient();
  return useMutation({
    mutationFn: kernel.saveSettings,
    onSuccess: (saved) => {
      client.setQueryData(keys.settings, saved);
      void client.invalidateQueries({
        predicate: (q) => q.queryKey[0] !== "settings" && q.queryKey[0] !== "tools",
      });
    },
  });
}

/** 项目页「历史」页签：这个项目的快照，从新到旧。 */
export function useHistory(projectRoot: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: keys.history(projectRoot ?? ""),
    queryFn: () => kernel.history(projectRoot ?? ""),
    enabled: projectRoot !== undefined && enabled,
  });
}

/** 项目页「清单」页签：三份文件的结构化视图与源文件。 */
export function useManifestView(projectRoot: string | undefined, enabled: boolean) {
  return useQuery({
    queryKey: keys.manifestView(projectRoot ?? ""),
    queryFn: () => kernel.manifestView(projectRoot ?? ""),
    enabled: projectRoot !== undefined && enabled,
  });
}
