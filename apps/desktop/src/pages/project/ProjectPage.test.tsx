import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { routes } from "@/app/router";
import type { EffectiveContext } from "@/ipc/bindings/EffectiveContext";
import type { EffectiveItem } from "@/ipc/bindings/EffectiveItem";
import type { ProjectSummary } from "@/ipc/bindings/ProjectSummary";
import { kernel } from "@/ipc/commands";
import { useUi } from "@/state/ui";

const PROJECT_ID = "/Users/me/code/media";

const mediaSummary: ProjectSummary = {
  id: PROJECT_ID,
  name: "media",
  path: "~/Project/media",
  state: "synced",
  lastSeen: null,
  sessions: 0,
  tools: ["codex"],
  registered: true,
};

function makeItem(
  over: Partial<EffectiveItem> & Pick<EffectiveItem, "id" | "name">,
): EffectiveItem {
  return {
    kind: "skill",
    version: null,
    scope: "global",
    bundle: null,
    state: null,
    label: "本项目从未触发",
    resident: 200,
    counted: true,
    location: "~/.agents/skills",
    linkTarget: null,
    source: null,
    hasScripts: false,
    members: [],
    command: null,
    declared: [],
    reason: "",
    triggers: 0,
    files: [],
    secrets: [],
    recent: [],
    drift: null,
    update: null,
    overriddenBy: null,
    disable: null,
    ...over,
  };
}

const mediaContext: EffectiveContext = {
  tool: "codex",
  projectName: "media",
  projectPath: "~/Project/media",
  hasManifest: true,
  mode: "link",
  targets: ["codex"],
  budget: { limit: 20000, instructions: 0, skills: 1000, mcp: 0, mcpUnestimated: 2 },
  items: [
    makeItem({ id: "s1", name: "create-statusline 等 6 项", resident: 500 }),
    makeItem({ id: "s2", name: "html 等 3 项", resident: 300 }),
    makeItem({ id: "s3", name: "extra-skill", resident: 200 }),
    makeItem({
      id: "mcp-1",
      name: "filesystem",
      kind: "mcp",
      scope: "project",
      resident: null,
      counted: true,
      label: "本项目从未调用",
    }),
    makeItem({
      id: "mcp-2",
      name: "github",
      kind: "mcp",
      scope: "project",
      resident: null,
      counted: true,
      label: "本项目从未调用",
    }),
  ],
  observed: [],
  observedMissing: [],
};

function setupMocks() {
  useUi.setState({ tool: "codex", guideHidden: true, sidebarCollapsed: false });
  vi.spyOn(kernel, "overview").mockResolvedValue({
    projects: 1,
    library: 0,
    bundles: 0,
    global: 0,
    pending: 0,
  });
  vi.spyOn(kernel, "settings").mockResolvedValue({
    readOnly: false,
    snapshotDays: 30,
    snapshotKeep: 20,
  });
  vi.spyOn(kernel, "tools").mockResolvedValue([{ id: "codex", name: "Codex", support: "full" }]);
  vi.spyOn(kernel, "projects").mockResolvedValue([mediaSummary]);
  vi.spyOn(kernel, "effectiveContext").mockResolvedValue(mediaContext);
  vi.spyOn(kernel, "planSync").mockResolvedValue({
    id: 1,
    title: "同步 media",
    changes: [],
    notes: [],
    blocked: false,
  });
  vi.spyOn(kernel, "suggestMode").mockResolvedValue({ mode: "link", committers: 1 });
  vi.spyOn(kernel, "manifestView").mockResolvedValue({ files: [] });
  vi.spyOn(kernel, "history").mockResolvedValue([]);
}

function renderProject() {
  const router = createMemoryRouter(routes, {
    initialEntries: [`/projects/${encodeURIComponent(PROJECT_ID)}`],
  });
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

describe("项目页骨架与双层地图联动（6.3）", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("页签栏与上下文地图位于左栏内，检查器与左栏并排贯通；切到清单/历史隐藏地图且页签栏不动", async () => {
    setupMocks();
    const { container } = (() => {
      renderProject();
      return { container: document.body };
    })();

    const map = await screen.findByRole("region", { name: "常驻上下文" });
    const tablist = screen.getByRole("tablist", { name: "项目视图" });
    const inspector = screen.getByRole("complementary", { name: "检查器" });
    const listSection = container.querySelector("section.list");
    const split = container.querySelector(".split");

    expect(listSection).not.toBeNull();
    expect(split).not.toBeNull();
    expect(listSection?.contains(tablist)).toBe(true);
    expect(listSection?.contains(map)).toBe(true);
    expect(split?.contains(inspector)).toBe(true);
    expect(listSection?.contains(inspector)).toBe(false);

    // 切到「清单」页签：隐藏上下文地图，页签栏仍在左栏顶部
    fireEvent.click(within(tablist).getByRole("tab", { name: "清单" }));
    await waitFor(() => {
      expect(screen.queryByRole("region", { name: "常驻上下文" })).not.toBeInTheDocument();
    });
    expect(listSection?.contains(screen.getByRole("tablist", { name: "项目视图" }))).toBe(true);

    // 切到「历史」页签：同样不渲染上下文地图，页签栏仍在
    fireEvent.click(within(tablist).getByRole("tab", { name: "历史" }));
    expect(screen.queryByRole("region", { name: "常驻上下文" })).not.toBeInTheDocument();
    expect(listSection?.contains(screen.getByRole("tablist", { name: "项目视图" }))).toBe(true);

    // 切回「生效上下文」：上下文地图恢复
    fireEvent.click(within(tablist).getByRole("tab", { name: "生效上下文" }));
    expect(await screen.findByRole("region", { name: "常驻上下文" })).toBeInTheDocument();
  });

  it("点击「MCP 2 项未估」虚点块：自动切回全部筛选、展开 MCP 分组并选中首个未估算 MCP", async () => {
    setupMocks();
    renderProject();

    await screen.findByRole("region", { name: "常驻上下文" });

    // 先折叠 MCP 组，再切换到「待处理」筛选
    const mcpGroupToggle = screen.getByRole("button", { name: /^MCP\s*2$/ });
    fireEvent.click(mcpGroupToggle);
    expect(mcpGroupToggle).toHaveAttribute("aria-expanded", "false");

    const listSection = document.querySelector("section.list");
    expect(listSection).not.toBeNull();
    const filterGroup = screen.getByRole("group", { name: "筛选" });
    const pendingBtn = within(filterGroup).getByRole("button", { name: /待处理/ });
    fireEvent.click(pendingBtn);
    expect(pendingBtn).toHaveAttribute("aria-pressed", "true");
    expect(within(listSection as HTMLElement).getByText("没有待处理的项")).toBeInTheDocument();

    // 点击地图中的「MCP 2 项未估」虚点块
    fireEvent.click(screen.getByRole("button", { name: /MCP 2 项未估/ }));

    // 筛选回到「全部」，MCP 分组自动展开，检查器选中首个未估算 MCP
    const allBtn = within(filterGroup).getByRole("button", { name: /^全部/ });
    expect(allBtn).toHaveAttribute("aria-pressed", "true");
    const expandedMcpToggle = screen.getByRole("button", { name: /^MCP\s*2$/ });
    expect(expandedMcpToggle).toHaveAttribute("aria-expanded", "true");

    const inspector = screen.getByRole("complementary", { name: "检查器" });
    expect(
      within(inspector).getByRole("heading", { level: 2, name: "filesystem" }),
    ).toBeInTheDocument();
  });
});
